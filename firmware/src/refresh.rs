//! Refresh decisions and a checked, pointer-free representation for RTC memory.
use crate::badge::battery_text;

pub type SavedState = [u32; 4];
pub const EMPTY: SavedState = [0; 4];
const MAGIC: u32 = 0x4241_5401;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshKind {
    Full,
    Battery,
}

/// A complete update decision. Commit next_saved only after a successful physical refresh.
#[derive(Debug, PartialEq)]
pub struct UpdatePlan {
    pub kind: RefreshKind,
    pub volts: f32,
    pub next_saved: SavedState,
}

/// None leaves the panel and retained state unchanged.
pub fn plan(
    saved: SavedState,
    button_or_timer_wake: bool,
    volts: f32,
    full_after: Option<u32>,
) -> Option<UpdatePlan> {
    let [magic, voltage_bits, partials, check] = saved;
    let previous_volts = f32::from_bits(voltage_bits);
    let valid =
        magic == MAGIC && check == checksum(voltage_bits, partials) && previous_volts.is_finite();
    // Exact ADC repeats need no formatting; changed readings compare displayed text.
    let kind =
        if !button_or_timer_wake || !valid || full_after.is_some_and(|limit| partials >= limit) {
            RefreshKind::Full
        } else if voltage_bits == volts.to_bits()
            || battery_text(previous_volts) == battery_text(volts)
        {
            return None;
        } else {
            RefreshKind::Battery
        };
    let partials = if kind == RefreshKind::Battery { partials.saturating_add(1) } else { 0 };
    let bits = volts.to_bits();
    Some(UpdatePlan { kind, volts, next_saved: [MAGIC, bits, partials, checksum(bits, partials)] })
}

fn checksum(voltage_bits: u32, partials: u32) -> u32 {
    !(MAGIC ^ voltage_bits ^ partials.rotate_left(13))
}
