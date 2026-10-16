use nametag_preview::refresh::{plan, RefreshKind, EMPTY};

#[test]
fn cold_boot_reset_and_invalid_state_require_full_refresh() {
    let update = plan(EMPTY, true, 3.98, Some(24)).unwrap();
    assert_eq!(update.kind, RefreshKind::Full);
    assert_eq!(update.volts, 3.98);
    let saved = update.next_saved;
    assert_eq!(saved[1], update.volts.to_bits());
    assert_eq!(plan(saved, false, 3.98, Some(24)).unwrap().kind, RefreshKind::Full);
    for index in 0..saved.len() {
        let mut corrupt = saved;
        corrupt[index] ^= 1;
        assert_eq!(plan(corrupt, true, 3.98, Some(24)).unwrap().kind, RefreshKind::Full);
    }
}

#[test]
fn skips_only_when_rendered_percentage_and_voltage_match() {
    let saved = plan(EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    assert!(plan(saved, true, 3.981, Some(24)).is_none());
    assert_eq!(plan(saved, true, 3.97, Some(24)).unwrap().kind, RefreshKind::Battery);
    // Both round to 3.75V, but cross the 50% threshold.
    let saved = plan(EMPTY, false, 3.749, None).unwrap().next_saved;
    assert_eq!(plan(saved, true, 3.751, None).unwrap().kind, RefreshKind::Battery);
    // Numeric equality alone would hide the rendered minus sign on -0.00V.
    let saved = plan(EMPTY, false, 0.0, None).unwrap().next_saved;
    assert!(plan(saved, true, 0.0, None).is_none());
    assert_eq!(plan(saved, true, -0.0, None).unwrap().kind, RefreshKind::Battery);
}

#[test]
fn cleanup_counts_changes_instead_of_unchanged_wakes() {
    let saved = plan(EMPTY, false, 3.98, Some(2)).unwrap().next_saved;
    let update = plan(saved, true, 3.97, Some(2)).unwrap();
    assert_eq!(update.kind, RefreshKind::Battery);
    assert_eq!(update.next_saved[2], 1);
    let saved = update.next_saved;
    assert!(plan(saved, true, 3.97, Some(2)).is_none());
    let update = plan(saved, true, 3.96, Some(2)).unwrap();
    assert_eq!(update.kind, RefreshKind::Battery);
    assert_eq!(update.next_saved[2], 2);
    let saved = update.next_saved;
    assert_eq!(plan(saved, true, 3.96, Some(2)).unwrap().kind, RefreshKind::Full);
    assert!(plan(saved, true, 3.96, None).is_none());
    let update = plan(saved, true, 3.96, Some(2)).unwrap();
    assert_eq!(update.next_saved[2], 0);
    assert_eq!(plan(update.next_saved, true, 3.95, Some(2)).unwrap().kind, RefreshKind::Battery);
}
