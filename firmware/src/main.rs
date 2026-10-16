//! Fastfetch-style con nametag for the LilyGO T5 4.7" S3 (ED047TC1, 960x540).
//! Every boot initializes hardware, runs one wake cycle, then enters deep sleep.
#![no_std]
#![no_main]

extern crate alloc;

use esp_backtrace as _;
use esp_hal::{
    main,
    rtc_cntl::{wakeup_cause, Rtc},
    system::SleepSource,
};
use lilygo_epd47::{pin_config, Battery, Display};

esp_bootloader_esp_idf::esp_app_desc!();

mod app;
mod badge;
mod config;
mod hardware;
mod power;
mod refresh;

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

    let resume = matches!(wakeup_cause(), SleepSource::Ext1 | SleepSource::Timer);
    if let Err(error) = app::run(&mut board, resume, config::FULL_REFRESH_AFTER_PARTIALS) {
        esp_println::println!("{error}");
    }
    // Deep sleep restarts main; only the checked battery state survives in RTC RAM.
    board.sleep(&mut rtc)
}
