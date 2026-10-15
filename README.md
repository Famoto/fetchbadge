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
Draws the badge once, then deep-sleeps with no wake source. E-paper keeps the image unpowered.
Press **RST** on the side to redraw (also refreshes the battery line).

Edit `BADGE` in `firmware/src/config.rs` to change the user, host, information rows, QR URL/caption, or logo.
`LAYOUT` in the same file controls positions, spacing, QR module size, and grayscale ink.
Then run `cd preview && cargo run --locked` to see the result without flashing.
The preview uses the same configuration, fonts, and renderer as the firmware; its simulated battery
voltage and output path are constants at the top of `preview/src/main.rs`.

The firmware flow in `src/main.rs` is: initialize hardware and PSRAM, power on the panel,
read the battery, clear and render, flush, power off, then deep-sleep until reset.
The battery must be read after the panel rail is powered on. Returned clear/render/flush errors
are logged after powering off the panel, then the device sleeps.

`src/badge.rs` draws the logo, QR code, header, information rows, and grayscale ramp in order.
`src/badge/font.rs` handles font decoding and text drawing. The renderer returns drawing,
QR encoding, and basic layout errors to its caller. Battery percentage is a clamped linear
estimate between the voltage limits in `config.rs`.

Host checks: run `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` in `preview/`.

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
| 1 | LilyGO T5 4.7" E-Paper S3 V2.3 | you have it | — |
| 1 | LiPo 3.7 V, 1000 mAh, **503450** (5 x 34 x 50 mm) | JST-PH 2.0 2-pin. **Check polarity against BAT+/BAT− on the board before plugging in**, cheap packs are often reversed | €6–10 |
| 1 | Clear acrylic or PETG sheet, 1 mm, cut to 118 x 63 mm | protects the 0.87 mm glass panel; set `lens_t=0` to skip | €3 |
| 4 | M2 x 8 self-tapping screws for plastic (pan head) | bezel to tray | €2 |
| 1 | Adhesive **hook** velcro, 50 mm wide, cut to 100 mm | backpack patch panels are usually loop; get loop if yours is hook | €4 |
| – | Double-sided foam tape | holds battery and board still | €2 |
| – | Thread + needle or 2 small zip ties | through the stitch holes | — |
| – | ~40 g PETG or ASA filament | PLA softens in a hot car | — |
| 1 | USB-C data cable | flashing and charging (charges in the case) | — |
