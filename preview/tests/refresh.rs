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
fn cold_boot_reset_and_corrupt_state_require_full_refresh() {
    let current = state(3.98);
    let update = plan(EMPTY, true, &current, Some(24)).unwrap();
    assert_eq!(update.kind, RefreshKind::Full);
    let saved = update.next_saved;
    assert_eq!(plan(saved, false, &current, Some(24)).unwrap().kind, RefreshKind::Full);
    let check = |corrupt| {
        assert_eq!(plan(corrupt, true, &current, Some(24)).unwrap().kind, RefreshKind::Full)
    };
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
    let saved = plan(EMPTY, false, &state(3.98), Some(24)).unwrap().next_saved;
    assert!(plan(saved, true, &state(3.981), Some(24)).is_none());
    let update = plan(saved, true, &state(3.97), Some(24)).unwrap();
    assert_eq!(update.kind, battery);
    assert_eq!(update.clear_areas().count(), 1, "unchanged bounds must be cleared only once");
    let saved = plan(EMPTY, false, &state(3.749), None).unwrap().next_saved;
    assert_eq!(plan(saved, true, &state(3.751), None).unwrap().kind, battery);
    let saved = plan(EMPTY, false, &state(0.0), None).unwrap().next_saved;
    assert!(plan(saved, true, &state(0.0), None).is_none());
    assert_eq!(plan(saved, true, &state(-0.0), None).unwrap().kind, battery);
}

#[test]
fn each_view_is_compared_independently() {
    let original = state(3.98);
    let saved = plan(EMPTY, false, &original, None).unwrap().next_saved;
    for id in 0..6 {
        let mut current = original;
        current[id].content ^= 1;
        let update = plan(saved, true, &current, None).unwrap();
        assert_eq!(update.kind, RefreshKind::Partial(1 << id));
        assert!(plan(update.next_saved, true, &current, None).is_none());
    }
}

#[test]
fn moved_or_resized_views_clear_old_and_new_bounds_without_full_refresh() {
    let original = state(3.98);
    let saved = plan(EMPTY, false, &original, None).unwrap().next_saved;
    let mut current = original;
    current[badge::LOGO].area.size.width -= 1;
    let update = plan(saved, true, &current, None).unwrap();
    assert_eq!(update.kind, RefreshKind::Partial(1 << badge::LOGO));
    let areas: Vec<_> = update.clear_areas().collect();
    assert!(areas.contains(&original[badge::LOGO].area));
    assert_eq!(areas.len(), 1, "old bounds already contain the smaller view");
}

#[test]
fn cleanup_counts_flushes_instead_of_views_or_unchanged_calls() {
    let original = state(3.98);
    let saved = plan(EMPTY, false, &original, Some(2)).unwrap().next_saved;
    let mut current = original;
    current[badge::LOGO].content ^= 1;
    current[badge::DATA].content ^= 1;
    let update = plan(saved, true, &current, Some(2)).unwrap();
    assert_eq!(update.next_saved.partials, 1);
    assert!(plan(update.next_saved, true, &current, Some(2)).is_none());
    let update = plan(update.next_saved, true, &original, Some(2)).unwrap();
    assert_eq!(update.next_saved.partials, 2);
    assert!(plan(update.next_saved, true, &original, None).is_none());
    let update = plan(update.next_saved, true, &original, Some(2)).unwrap();
    assert_eq!(update.kind, RefreshKind::Full);
    assert_eq!(update.next_saved.partials, 0);
    assert!(plan(update.next_saved, true, &original, Some(2)).is_none());
    assert_eq!(plan(update.next_saved, true, &original, Some(0)).unwrap().kind, RefreshKind::Full);
}
