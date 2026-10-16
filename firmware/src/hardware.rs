//! ESP-specific operations used by the wake-cycle controller in app.rs.
use esp_hal::{
    delay::Delay,
    peripherals::{GPIO14, GPIO21},
    rtc_cntl::Rtc,
};
use lilygo_epd47::{Battery, Display, DrawMode, Error as DisplayError};

use crate::{
    app, badge,
    config::{BADGE, LAYOUT},
    power,
    refresh::{self, RefreshKind, SavedState, UpdatePlan},
};

// The wake cycle is the sole accessor, on one core. Every integer bit pattern is
// valid; refresh::plan checks the marker/checksum before trusting RTC memory.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut BATTERY_STATE: SavedState = refresh::EMPTY;

const PANEL_STARTUP_MS: u32 = 10;
const PANEL_SHUTDOWN_MS: u32 = 100;

pub type Error = badge::RenderError<DisplayError>;

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
        Self { display, battery, button_pin, delay: Delay::new() }
    }

    pub fn sleep(&mut self, rtc: &mut Rtc<'_>) -> ! {
        power::sleep(rtc, &mut self.button_pin, &self.delay)
    }
}

impl app::Hardware for Board<'_> {
    type Error = Error;

    fn measure(&mut self) -> f32 {
        self.display.power_on();
        self.delay.delay_millis(PANEL_STARTUP_MS);
        self.battery.read()
    }

    fn load_saved(&mut self) -> SavedState {
        // SAFETY: no interrupt or other core reads/writes this static.
        unsafe { core::ptr::read_volatile(&raw const BATTERY_STATE) }
    }

    fn store_saved(&mut self, saved: SavedState) {
        // SAFETY: the wake cycle is the sole accessor.
        unsafe { core::ptr::write_volatile(&raw mut BATTERY_STATE, saved) };
    }

    fn render(&mut self, update: &UpdatePlan) -> Result<(), Error> {
        match update.kind {
            RefreshKind::Full => {
                self.display.clear().map_err(|error| Error::Target("display clear", error))?;
                badge::Renderer::new(&mut self.display, &BADGE, &LAYOUT).draw(update.volts)
            }
            RefreshKind::Battery => {
                let area = badge::battery_area(&BADGE, &LAYOUT).map_err(Error::InvalidLayout)?;
                self.display
                    .clear_area(area.into())
                    .map_err(|error| Error::Target("battery clear", error))?;
                badge::Renderer::new(&mut self.display, &BADGE, &LAYOUT).draw_battery(update.volts)
            }
        }
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.display
            .flush(DrawMode::BlackOnWhite)
            .map_err(|error| Error::Target("display flush", error))
    }

    fn power_off(&mut self) {
        self.display.power_off();
        self.delay.delay_millis(PANEL_SHUTDOWN_MS);
    }
}
