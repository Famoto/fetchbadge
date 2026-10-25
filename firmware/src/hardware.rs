//! ESP display, retained storage, measurement, button handling, and deep sleep.
use alloc::vec::Vec;
use core::time::Duration;
use embedded_graphics::{
    prelude::{Point, Size},
    primitives::Rectangle,
};
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, Pull, RtcPin, RtcPinWithResistors},
    peripherals::{GPIO14, GPIO21},
    rtc_cntl::{
        sleep::{Ext1WakeupSource, RtcSleepConfig, TimerWakeupSource, WakeSource, WakeupLevel},
        wakeup_cause, Rtc,
    },
    system::SleepSource,
    time::{Duration as TouchDuration, Instant},
};
use lilygo_epd47::{Battery, Display, DrawMode, Error as DisplayError};

use crate::{
    app, config,
    touch::Touch,
    ui::{
        self,
        refresh::{self, SavedState},
    },
};

// The wake cycle is the sole accessor, on one core. Every integer bit pattern is
// valid; refresh::plan checks the marker/checksum before trusting RTC memory.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut DISPLAY_STATE: RtcSavedState = RtcSavedState::EMPTY;

/// RTC storage must comprise only ESP-HAL persistable fields. Keep graphics
/// types out of this representation because they cannot implement that trait.
#[repr(C)]
#[derive(Clone, Copy)]
struct RtcViewState {
    active: u32,
    top_left: [i32; 2],
    size: [u32; 2],
    content: u64,
}

impl RtcViewState {
    const EMPTY: Self = Self { active: 0, top_left: [0; 2], size: [0; 2], content: 0 };
}

unsafe impl esp_hal::Persistable for RtcViewState {}

#[repr(C)]
#[derive(Clone, Copy)]
struct RtcSavedState {
    version: u32,
    partials: u32,
    frame: [RtcViewState; ui::MAX_VIEWS],
    checksum: u64,
}

impl RtcSavedState {
    const EMPTY: Self = Self {
        version: 0,
        partials: 0,
        frame: [RtcViewState::EMPTY; ui::MAX_VIEWS],
        checksum: 0,
    };
}

unsafe impl esp_hal::Persistable for RtcSavedState {}

impl From<RtcSavedState> for SavedState {
    fn from(saved: RtcSavedState) -> Self {
        Self {
            version: saved.version,
            partials: saved.partials,
            frame: saved.frame.map(|view| refresh::ViewState {
                active: view.active,
                area: Rectangle::new(
                    Point::new(view.top_left[0], view.top_left[1]),
                    Size::new(view.size[0], view.size[1]),
                ),
                content: view.content,
            }),
            checksum: saved.checksum,
        }
    }
}

impl From<SavedState> for RtcSavedState {
    fn from(saved: SavedState) -> Self {
        Self {
            version: saved.version,
            partials: saved.partials,
            frame: saved.frame.map(|view| RtcViewState {
                active: view.active,
                top_left: [view.area.top_left.x, view.area.top_left.y],
                size: [view.area.size.width, view.area.size.height],
                content: view.content,
            }),
            checksum: saved.checksum,
        }
    }
}

const PANEL_STARTUP_MS: u32 = 10;
const PANEL_SHUTDOWN_MS: u32 = 100;
const BUTTON_POLL_MS: u32 = 10;
const BUTTON_RELEASE_SAMPLES: u32 = 3;
const BUTTON_RELEASE_TIMEOUT_MS: u32 = 2_000;
const TOUCH_POLL_MS: u32 = 30;

pub struct Board<'d> {
    display: Display<'d>,
    battery: Battery<'d, GPIO14<'d>>,
    button_pin: GPIO21<'d>,
    delay: Delay,
    touch: Option<Touch<'d>>,
    touch_deadline: Option<Instant>,
}

impl<'d> Board<'d> {
    pub fn new(
        display: Display<'d>,
        battery: Battery<'d, GPIO14<'d>>,
        button_pin: GPIO21<'d>,
        touch: Option<Touch<'d>>,
    ) -> Self {
        let delay = Delay::new();
        let wake = wakeup_cause();
        // Touch can't wake deep sleep (its INT is GPIO47, not an RTC pin), so a button
        // (RST or IO21) or power-on turns it on. Timer wakes park the GT911 right away;
        // it draws mA idle.
        let touch = touch.and_then(|touch| {
            if !matches!(wake, SleepSource::Timer) {
                Some(touch)
            } else {
                touch.sleep(&delay);
                None
            }
        });
        let mut board =
            Self { display, battery, button_pin, delay, touch, touch_deadline: None };
        // RST keeps RTC RAM, but power-on and reset must redraw everything.
        if !matches!(wake, SleepSource::Ext1 | SleepSource::Timer) {
            ui::Backend::store_saved(&mut board, refresh::EMPTY);
        }
        board
    }

    /// Configure the button and timer wake sources, retain RTC fast RAM, and deep-sleep.
    pub fn sleep(&mut self, rtc: &mut Rtc<'_>) -> ! {
        let button_level = self.prepare_button_wake();
        let timer = config::REFRESH_INTERVAL_MINUTES
            .map(|minutes| TimerWakeupSource::new(Duration::from_secs(u64::from(minutes) * 60)));
        let mut pins: [&mut dyn RtcPin; 1] = [&mut self.button_pin];
        let button = button_level.map(|level| Ext1WakeupSource::new(&mut pins, level));
        let sources: Vec<&dyn WakeSource> = [
            button.as_ref().map(|button| button as &dyn WakeSource),
            timer.as_ref().map(|timer| timer as &dyn WakeSource),
        ]
        .into_iter()
        .flatten()
        .collect();
        let mut sleep_config = RtcSleepConfig::deep();
        // Keep the checked display state in RTC fast RAM.
        sleep_config.set_rtc_fastmem_pd_en(false);
        rtc.sleep(&sleep_config, &sources);
        unreachable!("deep sleep restarts the processor");
    }

    fn prepare_button_wake(&mut self) -> Option<WakeupLevel> {
        if !config::BUTTON_WAKE_ENABLED {
            return None;
        }
        // IO21 is the user button. IO0 is shared with the panel and boot strapping.
        let button =
            Input::new(self.button_pin.reborrow(), InputConfig::default().with_pull(Pull::Up));
        let mut released_samples = 0;
        for _ in 0..BUTTON_RELEASE_TIMEOUT_MS / BUTTON_POLL_MS {
            self.delay.delay_millis(BUTTON_POLL_MS);
            released_samples = if button.is_high() { released_samples + 1 } else { 0 };
            if released_samples >= BUTTON_RELEASE_SAMPLES {
                break;
            }
        }
        // A held button wakes on release to avoid repeated immediate wakes. Release
        // starts another wake cycle, which restores normal press-to-refresh behavior.
        let level = if button.is_low() { WakeupLevel::High } else { WakeupLevel::Low };
        drop(button);
        // Digital GPIO pull configuration does not configure the RTC pull resistors.
        self.button_pin.rtcio_pullup(true);
        self.button_pin.rtcio_pulldown(false);
        Some(level)
    }
}

impl app::Hardware for Board<'_> {
    fn measure(&mut self) -> f32 {
        self.display.power_on();
        self.delay.delay_millis(PANEL_STARTUP_MS);
        self.battery.read()
    }

    fn touch_enabled(&self) -> bool {
        self.touch.is_some()
    }

    fn wait_tap(&mut self) -> Option<Point> {
        let touch = self.touch.as_mut()?;
        let window = TouchDuration::from_secs(config::TOUCH_WINDOW_SECS);
        let deadline = *self.touch_deadline.get_or_insert_with(|| Instant::now() + window);
        while touch.read().is_some() {} // drop taps queued during the last refresh
        while Instant::now() < deadline {
            if let Some((x, y)) = touch.read() {
                // QR tap area: x 0..440, y 396..540. Elsewhere? Fix the mapping in touch.rs.
                esp_println::println!("touch: tap at {x},{y}");
                self.touch_deadline = Some(Instant::now() + window);
                return Some(Point::new(x, y));
            }
            self.delay.delay_millis(TOUCH_POLL_MS);
        }
        None
    }

    fn power_off(&mut self) {
        if let Some(touch) = self.touch.take() {
            touch.sleep(&self.delay);
        }
        self.display.power_off();
        self.delay.delay_millis(PANEL_SHUTDOWN_MS);
    }
}

impl<'d> ui::Backend for Board<'d> {
    type Error = DisplayError;
    type Target = Display<'d>;

    fn target(&mut self) -> &mut Self::Target {
        &mut self.display
    }

    fn load_saved(&mut self) -> SavedState {
        // SAFETY: no interrupt or other core reads/writes this static.
        unsafe { core::ptr::read_volatile(&raw const DISPLAY_STATE).into() }
    }

    fn store_saved(&mut self, saved: SavedState) {
        // SAFETY: the wake cycle is the sole accessor.
        unsafe { core::ptr::write_volatile(&raw mut DISPLAY_STATE, saved.into()) };
    }

    fn clear(&mut self, area: Option<Rectangle>) -> Result<(), DisplayError> {
        match area {
            None => {
                // A failed awake update can leave pixels buffered; physical clear does not erase them.
                self.display.fill(15)?;
                self.display.clear()
            }
            Some(area) => self.display.clear_area(area.into()),
        }
    }

    fn flush(&mut self) -> Result<(), DisplayError> {
        self.display.flush(DrawMode::BlackOnWhite)
    }
}
