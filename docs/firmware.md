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
update only changed registered views. With the default static content, only the battery changes.
If all displayed content is unchanged, the firmware skips clearing/drawing/flushing.
E-paper keeps the image unpowered.

The registered views' bounds, content signatures, and partial refresh count are retained
in RTC fast RAM with a version marker and checksum; no flash writes are needed. Cold boot,
RST, invalid retained state, or a failed panel update forces a full redraw. Moving, resizing,
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
with V2.4 still requires a hardware check. Touch wake and the external RTC are not configured.

See [configuration.md](configuration.md) for the settings above and
[architecture.md](architecture.md) for how a wake cycle is structured.
