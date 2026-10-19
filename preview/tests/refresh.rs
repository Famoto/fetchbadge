use nametag_preview::{
    badge, config,
    ui::{
        refresh::{plan, FrameState, RefreshKind, SavedState, EMPTY},
        MAX_VIEWS,
    },
};

fn state(volts: f32) -> FrameState {
    badge::frame(&config::BADGE, &config::LAYOUT, volts).unwrap().state()
}

#[test]
fn missing_or_corrupt_state_requires_full_refresh() {
    let current = state(3.98);
    let update = plan(EMPTY, &current, Some(24)).unwrap();
    assert_eq!(update.kind, RefreshKind::Full);
    let saved = update.next_saved;
    let check =
        |corrupt| assert_eq!(plan(corrupt, &current, Some(24)).unwrap().kind, RefreshKind::Full);
    check(SavedState { version: saved.version ^ 1, ..saved });
    check(SavedState { partials: saved.partials ^ 1, ..saved });
    for mask in [1, 1u64 << 32] {
        check(SavedState { checksum: saved.checksum ^ mask, ..saved });
        for id in 0..MAX_VIEWS {
            let mut corrupt = saved;
            corrupt.frame[id].content ^= mask;
            check(corrupt);
        }
    }
    let mut corrupt = saved;
    corrupt.frame[badge::LOGO].area.top_left.x ^= 1;
    check(corrupt);
    corrupt = saved;
    corrupt.frame[badge::LOGO].active ^= 1;
    check(corrupt);
}

#[test]
fn unchanged_displayed_battery_skips_updates() {
    let battery = RefreshKind::Partial(1 << badge::BATTERY);
    let saved = plan(EMPTY, &state(3.98), Some(24)).unwrap().next_saved;
    assert!(plan(saved, &state(3.981), Some(24)).is_none());
    let update = plan(saved, &state(3.97), Some(24)).unwrap();
    assert_eq!(update.kind, battery);
    assert_eq!(update.clear_areas().count(), 1, "unchanged bounds must be cleared only once");
    // Readings round to BATTERY_STEP_VOLTS: 3.775 V is the boundary between 3.75 and 3.80.
    let saved = plan(EMPTY, &state(3.751), None).unwrap().next_saved;
    assert!(plan(saved, &state(3.774), None).is_none());
    assert_eq!(plan(saved, &state(3.776), None).unwrap().kind, battery);
    let saved = plan(EMPTY, &state(0.0), None).unwrap().next_saved;
    assert!(plan(saved, &state(0.0), None).is_none());
    for invalid in [-0.0, -1.0, f32::NAN] {
        assert!(plan(saved, &state(invalid), None).is_none(), "{invalid} displays as 0.00V");
    }
}

#[test]
fn each_view_is_compared_independently() {
    let original = state(3.98);
    let saved = plan(EMPTY, &original, None).unwrap().next_saved;
    for id in 0..6 {
        let mut current = original;
        current[id].content ^= 1;
        let update = plan(saved, &current, None).unwrap();
        assert_eq!(update.kind, RefreshKind::Partial(1 << id));
        assert!(plan(update.next_saved, &current, None).is_none());
    }
}

#[test]
fn moved_or_resized_views_clear_old_and_new_bounds_without_full_refresh() {
    let original = state(3.98);
    let saved = plan(EMPTY, &original, None).unwrap().next_saved;
    let mut current = original;
    current[badge::LOGO].area.size.width -= 1;
    let update = plan(saved, &current, None).unwrap();
    assert_eq!(update.kind, RefreshKind::Partial(1 << badge::LOGO));
    let areas: Vec<_> = update.clear_areas().collect();
    assert!(areas.contains(&original[badge::LOGO].area));
    assert_eq!(areas.len(), 1, "old bounds already contain the smaller view");
}

#[test]
fn cleanup_counts_flushes_instead_of_views_or_unchanged_calls() {
    let original = state(3.98);
    let saved = plan(EMPTY, &original, Some(2)).unwrap().next_saved;
    let mut current = original;
    current[badge::LOGO].content ^= 1;
    current[badge::DATA].content ^= 1;
    let update = plan(saved, &current, Some(2)).unwrap();
    assert_eq!(update.next_saved.partials, 1);
    assert!(plan(update.next_saved, &current, Some(2)).is_none());
    let update = plan(update.next_saved, &original, Some(2)).unwrap();
    assert_eq!(update.next_saved.partials, 2);
    assert!(plan(update.next_saved, &original, None).is_none());
    let update = plan(update.next_saved, &original, Some(2)).unwrap();
    assert_eq!(update.kind, RefreshKind::Full);
    assert_eq!(update.next_saved.partials, 0);
    assert!(plan(update.next_saved, &original, Some(2)).is_none());
    assert_eq!(plan(update.next_saved, &original, Some(0)).unwrap().kind, RefreshKind::Full);
}
