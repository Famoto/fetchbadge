//! Refresh decisions and a checked, pointer-free representation for RTC memory.
use crate::badge::battery_text;

pub type SavedState = [u32; 4];
pub const EMPTY: SavedState = [0; 4];
const MAGIC: u32 = 0x4241_5401;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshKind {
    Full,
    Battery,
    Skip,
}

/// The returned state must only be committed after a successful physical refresh.
pub fn plan(
    saved: SavedState,
    button_or_timer_wake: bool,
    volts: f32,
    full_after: Option<u32>,
) -> (RefreshKind, SavedState) {
    let [magic, voltage_bits, partials, check] = saved;
    let previous_volts = f32::from_bits(voltage_bits);
    let valid =
        magic == MAGIC && check == checksum(voltage_bits, partials) && previous_volts.is_finite();
    let kind =
        if !button_or_timer_wake || !valid || full_after.is_some_and(|limit| partials >= limit) {
            RefreshKind::Full
        } else if battery_text(previous_volts) == battery_text(volts) {
            RefreshKind::Skip
        } else {
            RefreshKind::Battery
        };
    if kind == RefreshKind::Skip {
        return (kind, saved);
    }
    let partials = if kind == RefreshKind::Battery {
        partials.saturating_add(1)
    } else {
        0
    };
    let bits = volts.to_bits();
    (kind, [MAGIC, bits, partials, checksum(bits, partials)])
}

fn checksum(voltage_bits: u32, partials: u32) -> u32 {
    !(MAGIC ^ voltage_bits ^ partials.rotate_left(13))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_boot_reset_and_invalid_state_require_full_refresh() {
        let (kind, saved) = plan(EMPTY, true, 3.98, Some(24));
        assert_eq!(kind, RefreshKind::Full);
        assert_eq!(plan(saved, false, 3.98, Some(24)).0, RefreshKind::Full);
        for index in 0..saved.len() {
            let mut corrupt = saved;
            corrupt[index] ^= 1;
            assert_eq!(plan(corrupt, true, 3.98, Some(24)).0, RefreshKind::Full);
        }
    }

    #[test]
    fn skips_only_when_rendered_percentage_and_voltage_match() {
        let (_, saved) = plan(EMPTY, false, 3.98, Some(24));
        assert_eq!(
            plan(saved, true, 3.981, Some(24)),
            (RefreshKind::Skip, saved)
        );
        assert_eq!(plan(saved, true, 3.97, Some(24)).0, RefreshKind::Battery);
        // Both round to 3.75V, but cross the 50% threshold.
        let (_, saved) = plan(EMPTY, false, 3.749, None);
        assert_eq!(plan(saved, true, 3.751, None).0, RefreshKind::Battery);
    }

    #[test]
    fn cleanup_counts_changes_instead_of_unchanged_wakes() {
        let (_, saved) = plan(EMPTY, false, 3.98, Some(2));
        let (kind, saved) = plan(saved, true, 3.97, Some(2));
        assert_eq!(kind, RefreshKind::Battery);
        assert_eq!(plan(saved, true, 3.97, Some(2)), (RefreshKind::Skip, saved));
        let (kind, saved) = plan(saved, true, 3.96, Some(2));
        assert_eq!(kind, RefreshKind::Battery);
        assert_eq!(plan(saved, true, 3.96, Some(2)).0, RefreshKind::Full);
        assert_eq!(plan(saved, true, 3.96, None).0, RefreshKind::Skip);
        let (_, saved) = plan(saved, true, 3.96, Some(2));
        assert_eq!(plan(saved, true, 3.95, Some(2)).0, RefreshKind::Battery);
    }
}
