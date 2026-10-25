# Firmware

Rust, `no_std`, esp-hal. Driver: [`lilygo-epd47`](https://crates.io/crates/lilygo-epd47) 1.1
(esp-hal port of LilyGO's ED047TC1 driver, V2.3/S3 only, GPL-3.0).

## Build and flash

```sh
cargo install espup espflash
espup install --targets esp32s3 --toolchain-version 1.95.0.0   # version this was built with
. ~/export-esp.sh
cd firmware && cargo run --release        # builds, flashes, opens monitor
```

Pinned: `esp-hal ~1.1` (1.2 breaks lilygo-epd47 1.1) and `Cargo.lock` from a clean build.
Newer esp toolchains (1.99) fail on `xtensa-lx-rt 0.22`, so stick to 1.95.
If flashing doesn't start: hold the IO0 (STR_IO0) button, tap RST, release.

## Wake and refresh behavior

Draws the whole badge on power-on or **RST**, then deep-sleeps until the **IO21** user button
is pressed or the configured interval elapses (currently 60 minutes). Button/timer wakes
update only changed registered views. With the default static content, only the battery changes,
and only when its reading crosses a `BATTERY_STEP_VOLTS` step (0.05 V), so ADC noise is ignored.
If all displayed content is unchanged, the firmware skips clearing/drawing/flushing.
E-paper keeps the image unpowered.

The registered views' bounds, content signatures, and partial refresh count are retained
in RTC fast RAM with a version marker and checksum; no flash writes are needed. `Board::new`
clears that record on power-on and RST (any wake that is not the button or timer), so those
redraw fully, as do invalid retained state and a failed panel update. Moving, resizing,
registering, or deregistering a view uses partial updates, including erasing its old location.
`FULL_REFRESH_AFTER_PARTIALS` in `firmware/src/config.rs` defaults to `Some(24)`: the wake after
24 successful partial updates performs a full cleanup, even if content is unchanged.
An update of several views counts once. Set it to `None` to disable
automatic cleanup, or `Some(0)` to always redraw fully. Adjust this after checking ghosting
on your panel. Unchanged content does not count toward the cleanup threshold.

## Wake sources

Set `REFRESH_INTERVAL_MINUTES` in `firmware/src/config.rs` to `Some(360)` for six hours,
`Some(30)` for thirty minutes, or `None` for button-only refresh. The minute count must be nonzero.
Set `BUTTON_WAKE_ENABLED` in `firmware/src/config.rs` to `false` for timer-only operation. Disabling both restores
reset-only operation. The interval starts when entering sleep after each refresh; it is not
a wall-clock schedule and needs neither Wi-Fi nor the external PCF8563 RTC.

Before sleeping, the firmware waits up to two seconds for a debounced button release.
If IO21 is still held, it sleeps with wake-on-release instead of wake-on-press, avoiding
repeated immediate wakes. Releasing then checks the battery again and restores normal
press-to-refresh behavior. The scheduled timer remains enabled during a long press.
IO0 remains reserved for boot/panel control.

## Board revision

The supplied pinmap picture labels its PCB **V2.4 capacitive touch**. IO21/button and
IO14/battery agree with this implementation, but the driver targets V2.3; display compatibility
with V2.4 still requires a hardware check. The external RTC is not configured.

## Touch

The GT911 (I2C SDA=18, SCL=17, INT=47) cannot wake deep sleep because GPIO47 is not an RTC pin.
Any wake except the timer (power-on, RST, IO21) therefore turns touch on for `TOUCH_WINDOW_SECS` (60 s, restarted by each
tap) and swaps the QR caption for `qr_touch_caption`. Tapping the QR view shows the code full
screen; any tap returns. When the window closes, the plain badge is restored and the GT911 sleeps.
Timer wakes put it to sleep immediately; serial logs each tap and a missing GT911. If taps land mirrored, adjust the mapping in `touch::Touch::read`.

See [configuration.md](configuration.md) for the settings above and
[architecture.md](architecture.md) for how a wake cycle is structured.
