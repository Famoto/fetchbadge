//! ESP display, retained storage, measurement, button handling, and deep sleep.
use alloc::vec::Vec;
use core::time::Duration;
use embedded_graphics::primitives::Rectangle;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, Pull, RtcPin, RtcPinWithResistors},
    peripherals::{GPIO14, GPIO21},
    rtc_cntl::{
        sleep::{Ext1WakeupSource, RtcSleepConfig, TimerWakeupSource, WakeSource, WakeupLevel},
        wakeup_cause, Rtc,
    },
    system::SleepSource,
};
use lilygo_epd47::{Battery, Display, DrawMode, Error as DisplayError};

use crate::{
    app, config,
    ui::{
        self,
        refresh::{self, SavedState},
    },
};

// The wake cycle is the sole accessor, on one core. Every integer bit pattern is
// valid; refresh::plan checks the marker/checksum before trusting RTC memory.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut DISPLAY_STATE: SavedState = refresh::EMPTY;

const PANEL_STARTUP_MS: u32 = 10;
const PANEL_SHUTDOWN_MS: u32 = 100;
const BUTTON_POLL_MS: u32 = 10;
const BUTTON_RELEASE_SAMPLES: u32 = 3;
const BUTTON_RELEASE_TIMEOUT_MS: u32 = 2_000;

pub struct Board<'d> {
    display: Display<'d>,
    battery: Battery<'d, GPIO14<'d>>,
    button_pin: GPIO21<'d>,
    delay: Delay,
}

impl<'d> Board<'d> {
    pub fn new(
        display: Display<'d>,
        battery: Battery<'d, GPIO14<'d>>,
        button_pin: GPIO21<'d>,
    ) -> Self {
        let mut board = Self { display, battery, button_pin, delay: Delay::new() };
        // RST keeps RTC RAM, but power-on and reset must redraw everything.
        if !matches!(wakeup_cause(), SleepSource::Ext1 | SleepSource::Timer) {
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

    fn power_off(&mut self) {
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
        unsafe { core::ptr::read_volatile(&raw const DISPLAY_STATE) }
    }

    fn store_saved(&mut self, saved: SavedState) {
        // SAFETY: the wake cycle is the sole accessor.
        unsafe { core::ptr::write_volatile(&raw mut DISPLAY_STATE, saved) };
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
