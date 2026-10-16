//! Button release handling and RTC wake configuration for deep sleep.
use core::time::Duration;
use esp_hal::{
    delay::Delay,
    gpio::{Input, InputConfig, Pull, RtcPin, RtcPinWithResistors},
    peripherals::GPIO21,
    rtc_cntl::{
        sleep::{Ext1WakeupSource, RtcSleepConfig, TimerWakeupSource, WakeupLevel},
        Rtc,
    },
};

use crate::config;

const BUTTON_POLL_MS: u32 = 10;
const BUTTON_RELEASE_SAMPLES: u32 = 3;
const BUTTON_RELEASE_TIMEOUT_MS: u32 = 2_000;

fn prepare_button_wake(button_pin: &mut GPIO21<'_>, delay: &Delay) -> Option<WakeupLevel> {
    if !config::BUTTON_WAKE_ENABLED {
        return None;
    }
    // IO21 is the user button. IO0 is shared with the panel and boot strapping.
    let button = Input::new(button_pin.reborrow(), InputConfig::default().with_pull(Pull::Up));
    let mut released_samples = 0;
    for _ in 0..BUTTON_RELEASE_TIMEOUT_MS / BUTTON_POLL_MS {
        delay.delay_millis(BUTTON_POLL_MS);
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
    button_pin.rtcio_pullup(true);
    button_pin.rtcio_pulldown(false);
    Some(level)
}

pub fn sleep(rtc: &mut Rtc<'_>, button_pin: &mut GPIO21<'_>, delay: &Delay) -> ! {
    let button_level = prepare_button_wake(button_pin, delay);
    let timer = config::REFRESH_INTERVAL_MINUTES.map(|minutes| {
        assert!(minutes > 0, "refresh interval must be greater than zero");
        TimerWakeupSource::new(Duration::from_secs(u64::from(minutes) * 60))
    });
    // Keep borrowed wake sources local and retain the checked battery state.
    let mut pins: [&mut dyn RtcPin; 1] = [button_pin];
    let button = match button_level {
        Some(level) => Some(Ext1WakeupSource::new(&mut pins, level)),
        None => None,
    };
    let mut config = RtcSleepConfig::deep();
    config.set_rtc_fastmem_pd_en(false);
    match (button.as_ref(), timer.as_ref()) {
        (Some(button), Some(timer)) => rtc.sleep(&config, &[button, timer]),
        (Some(button), None) => rtc.sleep(&config, &[button]),
        (None, Some(timer)) => rtc.sleep(&config, &[timer]),
        (None, None) => rtc.sleep(&config, &[]),
    }
    unreachable!("deep sleep restarts the processor");
}
