//! Fastfetch-style con nametag for the LilyGO T5 4.7" S3 (ED047TC1, 960x540).
//! Full redraw on reset; battery-only updates on IO21 button or timer wake.
//! E-paper keeps the image while the panel is powered off.
#![no_std]
#![no_main]

extern crate alloc;

use core::time::Duration;
use esp_backtrace as _;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, Pull, RtcPin, RtcPinWithResistors},
    main,
    rtc_cntl::{
        sleep::{Ext1WakeupSource, RtcSleepConfig, TimerWakeupSource, WakeSource, WakeupLevel},
        wakeup_cause, Rtc,
    },
    system::SleepSource,
};
use lilygo_epd47::{pin_config, Battery, Display, DrawMode};

esp_bootloader_esp_idf::esp_app_desc!();

mod badge;
mod config;
mod refresh;

// Only main accesses this state, on one core. Integers are valid for every bit pattern;
// refresh::plan checks the marker/checksum before trusting retained memory.
#[esp_hal::ram(unstable(rtc_fast, persistent))]
static mut BATTERY_STATE: refresh::SavedState = refresh::EMPTY;

const PANEL_STARTUP_MS: u32 = 10;
const PANEL_SHUTDOWN_MS: u32 = 100;

// The scheduled interval is configured in config.rs; RST always remains available.
const BUTTON_WAKE_ENABLED: bool = true;
const BUTTON_POLL_MS: u32 = 10;
const BUTTON_RELEASE_SAMPLES: u32 = 3;
const BUTTON_RELEASE_TIMEOUT_MS: u32 = 2_000;

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

    // Deep-sleep wake restarts main. Keep the panel powered while sampling the battery.
    display.power_on();
    delay.delay_millis(PANEL_STARTUP_MS);
    let refresh: Result<(), alloc::string::String> = (|| {
        let volts = battery.read();
        // SAFETY: no interrupt or other core reads/writes this static.
        let saved = unsafe { core::ptr::read_volatile(&raw const BATTERY_STATE) };
        let resume = matches!(wakeup_cause(), SleepSource::Ext1 | SleepSource::Timer);
        let (kind, next_state) =
            refresh::plan(saved, resume, volts, config::FULL_REFRESH_AFTER_PARTIALS);
        if kind == refresh::RefreshKind::Skip {
            return Ok(());
        }
        // A failed/interrupted refresh leaves the panel contents unknown. Force a full
        // recovery next wake, and only commit the new reading after a successful flush.
        // SAFETY: main is the sole accessor.
        unsafe { core::ptr::write_volatile(&raw mut BATTERY_STATE, refresh::EMPTY) };
        match kind {
            refresh::RefreshKind::Full => {
                display
                    .clear()
                    .map_err(|error| alloc::format!("display clear failed: {error:?}"))?;
                badge::draw(&mut display, &config::BADGE, &config::LAYOUT, volts)
                    .map_err(|error| alloc::format!("badge rendering failed: {error}"))?;
            }
            refresh::RefreshKind::Battery => {
                let area = badge::battery_area(&config::BADGE, &config::LAYOUT)
                    .map_err(|error| alloc::format!("battery layout failed: {error}"))?;
                display
                    .clear_area(lilygo_epd47::display::Rectangle {
                        x: area.top_left.x as u16,
                        y: area.top_left.y as u16,
                        width: area.size.width as u16,
                        height: area.size.height as u16,
                    })
                    .map_err(|error| alloc::format!("battery clear failed: {error:?}"))?;
                badge::draw_battery(&mut display, &config::BADGE, &config::LAYOUT, volts)
                    .map_err(|error| alloc::format!("battery rendering failed: {error}"))?;
            }
            refresh::RefreshKind::Skip => unreachable!(),
        }
        display
            .flush(DrawMode::BlackOnWhite)
            .map_err(|error| alloc::format!("display flush failed: {error:?}"))?;
        // SAFETY: main is the sole accessor; the panel now matches this state.
        unsafe { core::ptr::write_volatile(&raw mut BATTERY_STATE, next_state) };
        Ok(())
    })();

    // Shut down even if clearing, drawing, or flushing returned an error.
    display.power_off();
    delay.delay_millis(PANEL_SHUTDOWN_MS);
    if let Err(error) = refresh {
        esp_println::println!("{error}");
    }

    let timer = config::REFRESH_INTERVAL_MINUTES.map(|minutes| {
        assert!(minutes > 0, "refresh interval must be greater than zero");
        TimerWakeupSource::new(Duration::from_secs(u64::from(minutes) * 60))
    });
    if BUTTON_WAKE_ENABLED {
        // IO21 is the user button. IO0 is shared with the panel and boot strapping.
        let mut button_pin = peripherals.GPIO21;
        let button = Input::new(
            button_pin.reborrow(),
            InputConfig::default().with_pull(Pull::Up),
        );
        let mut released_samples = 0;
        for _ in 0..BUTTON_RELEASE_TIMEOUT_MS / BUTTON_POLL_MS {
            delay.delay_millis(BUTTON_POLL_MS);
            if button.is_high() {
                released_samples += 1;
                if released_samples >= BUTTON_RELEASE_SAMPLES {
                    break;
                }
            } else {
                released_samples = 0;
            }
        }
        // A held button must not immediately wake the device again. In that case,
        // arm release instead; release checks the battery again and rearms press.
        let level = if button.is_low() {
            WakeupLevel::High
        } else {
            WakeupLevel::Low
        };
        drop(button);
        // Digital GPIO pull configuration does not configure the RTC pull resistors.
        button_pin.rtcio_pullup(true);
        button_pin.rtcio_pulldown(false);
        let mut pins: [&mut dyn RtcPin; 1] = [&mut button_pin];
        let button_wake = Ext1WakeupSource::new(&mut pins, level);
        if let Some(timer) = timer.as_ref() {
            sleep_retaining_battery(&mut rtc, &[&button_wake, timer]);
        }
        sleep_retaining_battery(&mut rtc, &[&button_wake]);
    }
    if let Some(timer) = timer.as_ref() {
        sleep_retaining_battery(&mut rtc, &[timer]);
    }
    sleep_retaining_battery(&mut rtc, &[]);
}

fn sleep_retaining_battery(rtc: &mut Rtc<'_>, sources: &[&dyn WakeSource]) -> ! {
    // The HAL's default deep-sleep config powers RTC fast RAM down. Keep the
    // persistent state alive for button-only, timer-only, and combined operation.
    let mut config = RtcSleepConfig::deep();
    config.set_rtc_fastmem_pd_en(false);
    rtc.sleep(&config, sources);
    unreachable!("deep sleep restarts the processor");
}
