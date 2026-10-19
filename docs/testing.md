# Testing

## Host

Host checks: run `cargo test --locked` and `cargo clippy --locked --all-targets -- -D warnings` in `preview/`.
Firmware check: run `cargo build --release --locked` in `firmware/` with the ESP toolchain.
All desktop tests live in `preview/tests/`: `controller.rs` checks sequencing and failure recovery,
`refresh.rs` checks retained state, `rendering.rs` checks pixels and bounds, and `scoped_refresh.rs`
checks edit tracking, scoped commits, regions, and recovery. Display tests share one simulator
in `support/mod.rs`. The preview library imports the actual firmware modules via `#[path]`.
Host tests preserve the checked-in preview pixels and compare partial updates against full
renders, including registration, replacement, removal, movement, resizing, overlap chains,
shrinking text, and empty content.

## Hardware

On hardware, test IO21 wake, a temporarily short timer interval (e.g. `Some(1)` for one minute), a press
held through the refresh and released after sleep, and repeated sleep/wake cycles on battery.
Also verify button-only/timer-only modes, sleep current, and displayed voltage against a
multimeter. Confirm unchanged readings cause no visible refresh, changed readings affect only
the battery row, RST redraws everything, and `FULL_REFRESH_AFTER_PARTIALS = Some(2)` produces
a full cleanup after two partial updates. Check ghosting over repeated partial updates.
Restore the normal interval and cleanup threshold after testing; percentage remains an approximate
voltage-based estimate and does not detect charging or battery presence.
