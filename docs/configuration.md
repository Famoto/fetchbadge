# Configuration

All badge settings live in `firmware/src/config.rs`.

## Content and layout

Edit `BADGE` in `firmware/src/config.rs` to change the user, host, information rows,
QR URL/caption, logo, or colour scale values. `BadgeConfig<'a>` also accepts strings and
rows borrowed from runtime buffers; these buffers must live through the wake cycle.
`LAYOUT` uses the same small `ViewLayout` for each view's bounds and anchor, with named settings
for spacing, QR module size, and grayscale ink. View bounds use panel coordinates;
all element positions, including text origins and the QR bottom, are relative to their view.
Then run `cd preview && cargo run --locked` to see the result without flashing.
The preview uses the same configuration, fonts, and renderer as the firmware; its simulated battery
voltage and output path are constants at the top of `preview/src/main.rs`.

## Refresh settings

| Setting | Default | Meaning |
|---|---|---|
| `REFRESH_INTERVAL_MINUTES` | `Some(60)` | Timer wake interval; `None` = button-only. Zero fails the build. |
| `BUTTON_WAKE_ENABLED` | `true` | IO21 wake; `false` = timer-only. Both off = reset-only. |
| `FULL_REFRESH_AFTER_PARTIALS` | `Some(24)` | Full cleanup after this many partial updates; `None` = never, `Some(0)` = always. |
| `BATTERY_EMPTY_VOLTS` / `BATTERY_FULL_VOLTS` | 3.3 / 4.2 | Linear battery percentage range. |

Details on what each wake does are in [firmware.md](firmware.md#wake-and-refresh-behavior).

## Font and logo

Text is [Fira Code](https://github.com/tonsky/FiraCode) Medium (OFL), pre-rendered with 16-level antialiasing into
`firmware/src/fonts/*.bin` by `tools/fontgen.py`. Ligatures aren't applied. To change the weight, rerun it with another
Fira Code `.ttf`. `tools/moon.py` generates the shaded crescent. Tweak `INNER` (shape) or `SPARKLES`, then paste its output into `LOGO`.
