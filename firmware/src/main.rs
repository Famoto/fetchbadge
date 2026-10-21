//! Fastfetch-style con nametag for the LilyGO T5 4.7" S3 (ED047TC1, 960x540).
//! Every boot initializes hardware, runs one wake cycle, then enters deep sleep.
#![no_std]
#![no_main]

extern crate alloc;

use esp_backtrace as _;
use esp_hal::{main, rtc_cntl::Rtc};
use lilygo_epd47::{pin_config, Battery, Display};

esp_bootloader_esp_idf::esp_app_desc!();

mod app;
mod badge;
// Shared with the host preview and tests, which use items the firmware does not.
#[allow(dead_code)]
mod config;
mod hardware;
#[allow(dead_code)]
mod ui;

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let psram = esp_hal::psram::PsramConfig {
        mode: esp_hal::psram::PsramMode::OctalSpi,
        ..Default::default()
    };
    esp_alloc::psram_allocator!(peripherals.PSRAM, esp_hal::psram, psram);

    let display = Display::new(
        pin_config!(peripherals),
        peripherals.DMA_CH0,
        peripherals.LCD_CAM,
        peripherals.RMT,
    )
    .expect("display init");
    let battery = Battery::new(peripherals.GPIO14, peripherals.ADC2);
    let mut rtc = Rtc::new(peripherals.LPWR);
    let mut board = hardware::Board::new(display, battery, peripherals.GPIO21);

    if let Err(error) =
        app::run(&mut board, &config::BADGE, &config::LAYOUT, config::FULL_REFRESH_AFTER_PARTIALS)
    {
        esp_println::println!("{error}");
    }
    // Deep sleep restarts main; only the checked display state survives in RTC RAM.
    board.sleep(&mut rtc)
}
