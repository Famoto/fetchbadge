//! Fastfetch-style con nametag for the LilyGO T5 4.7" S3 (ED047TC1, 960x540).
//! Draws once, then deep-sleeps forever. E-paper keeps the image with zero power.
//! Press the RST button (or replug) to redraw, e.g. to refresh the battery line.
#![no_std]
#![no_main]

extern crate alloc;

use esp_backtrace as _;
use esp_hal::{delay::Delay, main, rtc_cntl::Rtc};
use lilygo_epd47::{pin_config, Battery, Display, DrawMode};

esp_bootloader_esp_idf::esp_app_desc!();

mod badge;
mod config;

const PANEL_STARTUP_MS: u32 = 10;
const PANEL_SHUTDOWN_MS: u32 = 100;

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let psram = esp_hal::psram::PsramConfig {
        mode: esp_hal::psram::PsramMode::OctalSpi,
        ..Default::default()
    };
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram, psram);

    let mut display = Display::new(
        pin_config!(peripherals),
        peripherals.DMA_CH0,
        peripherals.LCD_CAM,
        peripherals.RMT,
    )
    .expect("display init");
    let mut battery = Battery::new(peripherals.GPIO14, peripherals.ADC2);
    let mut rtc = Rtc::new(peripherals.LPWR);
    let delay = Delay::new();

    // One refresh per reset. Keep the panel powered while sampling the battery.
    display.power_on();
    delay.delay_millis(PANEL_STARTUP_MS);
    let refresh: Result<(), alloc::string::String> = (|| {
        let volts = battery.read();
        display
            .clear()
            .map_err(|error| alloc::format!("display clear failed: {error:?}"))?;
        badge::draw(&mut display, &config::BADGE, &config::LAYOUT, volts)
            .map_err(|error| alloc::format!("badge rendering failed: {error}"))?;
        display
            .flush(DrawMode::BlackOnWhite)
            .map_err(|error| alloc::format!("display flush failed: {error:?}"))
    })();

    // Shut down even if clearing, drawing, or flushing returned an error.
    display.power_off();
    delay.delay_millis(PANEL_SHUTDOWN_MS);
    if let Err(error) = refresh {
        esp_println::println!("{error}");
    }

    // No wake source: the next refresh starts with RST.
    rtc.sleep_deep(&[]);
}
