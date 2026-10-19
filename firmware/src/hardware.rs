//! ESP display, retained storage, measurement, and power operations.
use embedded_graphics::primitives::Rectangle;
use esp_hal::{
    delay::Delay,
    peripherals::{GPIO14, GPIO21},
    rtc_cntl::Rtc,
};
use lilygo_epd47::{Battery, Display, DrawMode, Error as DisplayError};

use crate::{
    app, power,
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
