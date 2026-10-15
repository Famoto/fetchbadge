# Fastfetch con nametag (LilyGO T5 4.7" E-Paper S3 V2.3)

```
nametag/
  firmware/            Rust (no_std, esp-hal) — badge settings live in src/config.rs
  preview/             `cargo run` here renders badge.rs to preview.png on your computer
  tools/               fontgen.py (Fira Code -> e-paper font files), moon.py (the ASCII moon)
  case/                nametag_case.scad + tray.stl + bezel.stl
```

## Firmware

Driver: [`lilygo-epd47`](https://crates.io/crates/lilygo-epd47) 1.1 (esp-hal port of LilyGO's ED047TC1 driver, V2.3/S3 only, GPL-3.0).
Draws the whole badge on power-on or **RST**, then deep-sleeps until the **IO21** user button
is pressed or the configured interval elapses (currently 60 minutes). Button/timer wakes
update only the battery row. If the displayed percentage and two-decimal voltage are unchanged,
the firmware skips clearing/drawing/flushing. E-paper keeps the image unpowered.

The previous displayed reading and partial refresh count are retained in RTC fast RAM, with
a validity marker and checksum; no flash writes are needed. Cold boot, RST, invalid retained
state, or a failed refresh forces a full redraw. `FULL_REFRESH_AFTER_PARTIALS` in
`firmware/src/config.rs` defaults to `Some(24)`: the wake after 24 successful battery-only
redraws performs a full cleanup, even if the reading is unchanged. Set it to `None` to disable
automatic cleanup, or `Some(0)` to always redraw fully. Adjust this after checking ghosting
on your panel. Unchanged readings do not count toward the cleanup threshold.

Set `REFRESH_INTERVAL_MINUTES` in `firmware/src/config.rs` to `Some(360)` for six hours,
`Some(30)` for thirty minutes, or `None` for button-only refresh. The minute count must be nonzero.
Set `BUTTON_WAKE_ENABLED` in `firmware/src/main.rs` to `false` for timer-only operation. Disabling both restores
reset-only operation. The interval starts when entering sleep after each refresh; it is not
a wall-clock schedule and needs neither Wi-Fi nor the external PCF8563 RTC.

Before sleeping, the firmware waits up to two seconds for a debounced button release.
If IO21 is still held, it sleeps with wake-on-release instead of wake-on-press, avoiding
repeated immediate wakes. Releasing then checks the battery again and restores normal
press-to-refresh behavior. The scheduled timer remains enabled during a long press.
IO0 remains reserved for boot/panel control.

The supplied pinmap picture labels its PCB **V2.4 capacitive touch**. IO21/button and
IO14/battery agree with this implementation, but the driver targets V2.3; display compatibility
with V2.4 still requires a hardware check. Touch wake and the external RTC are not configured.

Edit `BADGE` in `firmware/src/config.rs` to change the user, host, information rows, QR URL/caption, or logo.
`LAYOUT` in the same file controls positions, spacing, QR module size, and grayscale ink.
Then run `cd preview && cargo run --locked` to see the result without flashing.
The preview uses the same configuration, fonts, and renderer as the firmware; its simulated battery
voltage and output path are constants at the top of `preview/src/main.rs`.

The firmware flow in `src/main.rs` is: initialize hardware and PSRAM, power on the panel,
read the battery, choose full/battery-only/no redraw, power off, then arm button/timer deep sleep.
Partial redraw physically clears the entire battery rectangle before drawing the new text,
so shorter values erase previous digits. The rest of the badge is left on the panel.
Even unchanged readings require a short panel power-on to sample the battery; boot, PSRAM,
and framebuffer allocation overhead also remain. Actual energy savings require measurement.
The battery must be read after the panel rail is powered on. Returned clear/render/flush errors
are logged after powering off the panel, then the device sleeps.

`src/badge.rs` draws the logo, QR code, header, information rows, and grayscale ramp in order,
and exposes the same battery renderer and bounds for partial updates. `src/refresh.rs` decides
whether to redraw, skip, or perform a full cleanup using the retained reading and wake type.
`src/badge/font.rs` handles font decoding and text drawing. The renderer returns drawing,
QR encoding, and basic layout errors to its caller. Battery percentage is a clamped linear
estimate between the voltage limits in `config.rs`.

Host checks: run `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` in `preview/`.
Firmware check: run `cargo build --release --locked` in `firmware/` with the ESP toolchain.
Host tests compare battery-only updates against full renders, check bounds and preservation
of surrounding pixels, and exercise retained-state validation, skip decisions, and cleanup.
On hardware, test IO21 wake, a temporarily short timer interval (e.g. `Some(1)` for one minute), a press
held through the refresh and released after sleep, and repeated sleep/wake cycles on battery.
Also verify button-only/timer-only modes, sleep current, and displayed voltage against a
multimeter. Confirm unchanged readings cause no visible refresh, changed readings affect only
the battery row, RST redraws everything, and `FULL_REFRESH_AFTER_PARTIALS = Some(2)` produces
a full cleanup after two partial updates. Check ghosting over repeated partial updates.
Restore the normal interval and cleanup threshold after testing; percentage remains an approximate
voltage-based estimate and does not detect charging or battery presence.

Text is [Fira Code](https://github.com/tonsky/FiraCode) Medium (OFL), pre-rendered with 16-level antialiasing into
`firmware/src/fonts/*.bin` by `tools/fontgen.py`. Ligatures aren't applied. To change the weight, rerun it with another
Fira Code `.ttf`. `tools/moon.py` generates the shaded crescent. Tweak `INNER` (shape) or `SPARKLES`, then paste its output into `LOGO`.

```sh
cargo install espup espflash
espup install --targets esp32s3 --toolchain-version 1.95.0.0   # version this was built with
. ~/export-esp.sh
cd firmware && cargo run --release        # builds, flashes, opens monitor
```

Pinned: `esp-hal ~1.1` (1.2 breaks lilygo-epd47 1.1) and `Cargo.lock` from a clean build.
Newer esp toolchains (1.99) fail on `xtensa-lx-rt 0.22`, so stick to 1.95.
If flashing doesn't start: hold the IO0 (STR_IO0) button, tap RST, release.

## Case

Print `tray.stl` floor-down and `bezel.stl` face-down, PETG/ASA, 0.2 mm, 3 walls, ~40 g total. No supports.
Outer size ~127 x 72 x 17 mm. Stack inside, front to back: clear lens, board, LiPo (foam tape).

Board numbers are from LilyGO's official `shell/EPD47-S3.dxf`, but **measure your board** and adjust
`fit`, `back_parts`, `bat_t`, and the port cutouts in the `.scad`. If USB-C/button cutouts land on the
wrong side, set `mirror_ports = true`. Re-export:

```sh
openscad -D 'part="tray"' -o tray.stl nametag_case.scad
openscad -D 'part="bezel"' -o bezel.stl nametag_case.scad
```

The back has a 100 x 50 x 0.8 mm recess for adhesive hook velcro, with 4 holes to stitch or zip-tie it down
(adhesive alone lets go in a warm con hall).

## Parts list

| # | Part | Notes | ~Price |
|---|------|-------|--------|
| 1 | LilyGO T5 4.7" E-Paper S3 V2.3 |  | 40€ |
| 1 | LiPo 3.7 V, 1000 mAh, **503450** (5 x 34 x 50 mm) | JST-PH 2.0 2-pin. **Check polarity against BAT+/BAT− on the board before plugging in**, cheap packs are often reversed | €6–10 |
| 1 | Clear acrylic or PETG sheet, 1 mm, cut to 118 x 63 mm | protects the 0.87 mm glass panel; set `lens_t=0` to skip | €3 |
| 4 | M2 x 8 self-tapping screws for plastic (pan head) | bezel to tray | €2 |
| 1 | Adhesive **hook** velcro, 50 mm wide, cut to 100 mm | backpack patch panels are usually loop; get loop if yours is hook | €4 |
| – | Double-sided foam tape | holds battery and board still | €2 |
| – | Thread + needle or 2 small zip ties | through the stitch holes | — |
| – | ~40 g PETG or ASA filament | PLA softens in a hot car | — |
| 1 | USB-C data cable | flashing and charging (charges in the case) | — |
