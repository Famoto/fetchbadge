use embedded_graphics::{prelude::*, primitives::Rectangle};
use nametag_preview::{
    badge, config,
    ui::{refresh, Element, Font, Pipeline, RefreshRequest, RenderError, View, MAX_VIEWS},
};

mod support;
use support::{Event, Failure, TestBoard};

fn area(x: i32, y: i32, width: u32, height: u32) -> Rectangle {
    Rectangle::new(Point::new(x, y), Size::new(width, height))
}

fn solid(bounds: Rectangle, ink: u8) -> View<'static> {
    View::new(bounds, vec![Element::Fill { area: Rectangle::new(Point::zero(), bounds.size), ink }])
}

fn scene(inks: [u8; 3]) -> Pipeline<'static> {
    let mut pipeline = Pipeline::new();
    for (id, ink) in inks.into_iter().enumerate() {
        pipeline.register(id, solid(area(10 + id as i32 * 100, 10, 40, 40), ink)).unwrap();
    }
    pipeline
}

fn displayed(pipeline: &Pipeline<'_>) -> TestBoard {
    let mut board = TestBoard::new(refresh::EMPTY, 3.98);
    board.powered = true;
    pipeline.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    board.events.clear();
    board
}

fn flushes(board: &TestBoard) -> usize {
    board.events.iter().filter(|event| matches!(event, Event::Flush)).count()
}

#[test]
fn unchanged_wakes_skip_panel_writes_and_changed_battery_flushes_once() {
    let original = badge::frame(&config::BADGE, &config::LAYOUT, 3.98).unwrap();
    let mut board = displayed(&original);
    let saved = board.saved;
    let started = std::time::Instant::now();
    for _ in 0..256 {
        let wake = badge::frame(&config::BADGE, &config::LAYOUT, 3.981).unwrap();
        wake.refresh(&mut board, RefreshRequest::Changed, Some(24)).unwrap();
    }
    eprintln!("256 unchanged host wake checks: {:?}", started.elapsed());
    assert_eq!(board.saved, saved);
    assert!(board.events.is_empty());

    let changed = badge::frame(&config::BADGE, &config::LAYOUT, 3.97).unwrap();
    changed.refresh(&mut board, RefreshRequest::Changed, Some(24)).unwrap();
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&changed);
}

#[test]
fn selected_views_commit_only_displayed_records_and_keep_other_changes_pending() {
    let original = scene([0, 4, 8]);
    let mut board = displayed(&original);
    let saved = board.saved;
    let changed = scene([2, 6, 8]);
    changed.refresh(&mut board, RefreshRequest::Views(1), None).unwrap();
    assert_eq!(board.saved.frame[0], changed.state()[0]);
    assert_eq!(board.saved.frame[1], saved.frame[1]);
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&scene([2, 4, 8]));

    board.events.clear();
    changed.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(flushes(&board), 1);
    assert_eq!(board.saved.frame, changed.state());
    board.assert_full_render(&changed);
    board.events.clear();
    changed.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert!(board.events.is_empty());
}

#[test]
fn sparse_view_ids_use_the_same_scoped_refresh_path() {
    let mut original = Pipeline::new();
    let mut changed = Pipeline::new();
    for (index, id) in [0, 7, MAX_VIEWS - 1].into_iter().enumerate() {
        let bounds = area(index as i32 * 100, 10, 40, 40);
        original.register(id, solid(bounds, 0)).unwrap();
        changed.register(id, solid(bounds, 4)).unwrap();
    }
    let mut board = displayed(&original);
    for id in [MAX_VIEWS - 1, 7, 0] {
        let previous = board.saved.frame;
        board.events.clear();
        changed.refresh(&mut board, RefreshRequest::Views(1 << id), None).unwrap();
        assert_eq!(flushes(&board), 1);
        for (other, record) in board.saved.frame.iter().enumerate() {
            assert_eq!(*record, if other == id { changed.state()[id] } else { previous[other] });
        }
    }
    board.assert_full_render(&changed);
    board.events.clear();
    changed.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert!(board.events.is_empty());
}

#[test]
fn scoped_updates_against_a_different_backend_keep_unselected_changes_pending() {
    let desired = scene([2, 6, 8]);
    let _first_board = displayed(&desired); // This registry now has no dirty bits.
    let mut other_board = displayed(&scene([0, 4, 8]));
    desired.refresh(&mut other_board, RefreshRequest::Views(1), None).unwrap();
    other_board.assert_full_render(&scene([2, 4, 8]));
    other_board.events.clear();
    desired.refresh(&mut other_board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(flushes(&other_board), 1);
    other_board.assert_full_render(&desired);
}

#[test]
fn edits_replacements_and_reverts_are_tracked_without_explicit_invalidation() {
    let mut pipeline = scene([0, 4, 8]);
    let mut board = displayed(&pipeline);
    let original = pipeline.get(0).unwrap().clone();
    pipeline.get_mut(0).unwrap().content[0] = Element::Fill { area: area(0, 0, 40, 40), ink: 2 };
    pipeline.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&scene([2, 4, 8]));
    board.events.clear();
    pipeline.register(0, original).unwrap();
    pipeline.register(0, solid(area(10, 10, 40, 40), 2)).unwrap();
    pipeline.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert!(board.events.is_empty(), "reverting to displayed content needs no update");
    pipeline.get_mut(0).unwrap().area.top_left.x = 300;
    pipeline.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&pipeline);
}

#[test]
fn a_region_redraws_its_view_and_leaves_unrelated_changes_pending() {
    let mut board = displayed(&scene([0, 4, 8]));
    let changed = scene([2, 6, 8]);
    changed.refresh(&mut board, RefreshRequest::Region(area(20, 20, 5, 5)), None).unwrap();
    assert!(board.events.contains(&Event::Clear(Some(area(10, 10, 40, 40)))));
    board.assert_full_render(&scene([2, 4, 8]));
    board.events.clear();
    changed.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&changed);
}

#[test]
fn region_expansion_includes_old_bounds_removals_and_overlap_chains() {
    let mut original = Pipeline::new();
    for (id, x) in [10, 40, 70, 400].into_iter().enumerate() {
        original.register(id, solid(area(x, 10, 40, 40), id as u8 * 3)).unwrap();
    }
    let mut board = displayed(&original);
    let saved = board.saved;
    let mut changed = original.clone();
    changed.get_mut(0).unwrap().area.top_left.x = 200;
    changed.deregister(1).unwrap();
    changed.register(3, solid(area(400, 10, 40, 40), 12)).unwrap();
    changed.refresh(&mut board, RefreshRequest::Region(area(15, 15, 5, 5)), None).unwrap();
    assert_eq!(board.saved.frame[0], changed.state()[0]);
    assert_eq!(board.saved.frame[1].active, 0);
    assert_eq!(board.saved.frame[2], changed.state()[2]);
    assert_eq!(board.saved.frame[3], saved.frame[3]);
    let mut expected = changed.clone();
    expected.register(3, original.get(3).unwrap().clone()).unwrap();
    board.assert_full_render(&expected);
    board.events.clear();
    changed.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&changed);
}

#[test]
fn background_regions_clear_and_flush_without_drawing_views() {
    let mut board = displayed(&scene([0, 4, 8]));
    let current = scene([0, 4, 8]);
    let saved = board.saved;
    current.refresh(&mut board, RefreshRequest::Region(area(600, 300, 20, 20)), None).unwrap();
    assert_eq!(flushes(&board), 1);
    assert!(!board.events.contains(&Event::Draw));
    assert_eq!(board.saved.frame, saved.frame);
    assert_eq!(board.saved.partials, saved.partials + 1);
}

#[test]
fn invalid_and_empty_regions_preserve_pixels_and_retained_records() {
    let pipeline = scene([0, 4, 8]);
    let mut board = displayed(&pipeline);
    let saved = board.saved;
    let pixels = board.pixels.pixels().to_vec();
    for bounds in [area(-1, 0, 10, 10), area(959, 0, 2, 1), area(i32::MAX, 0, u32::MAX, 1)] {
        assert!(matches!(
            pipeline.refresh(&mut board, RefreshRequest::Region(bounds), None),
            Err(RenderError::InvalidLayout(_))
        ));
    }
    for request in [RefreshRequest::Region(area(10, 10, 0, 0)), RefreshRequest::Views(0)] {
        pipeline.refresh(&mut board, request, None).unwrap();
    }
    assert_eq!(board.saved, saved);
    assert_eq!(board.pixels.pixels(), pixels);
    assert!(board.events.is_empty());
}

#[test]
fn explicit_redraws_and_cleanup_work_without_dirty_content() {
    let pipeline = scene([0, 4, 8]);
    let mut board = displayed(&pipeline);
    pipeline.refresh(&mut board, RefreshRequest::Views(2), Some(1)).unwrap();
    assert_eq!(board.saved.partials, 1);
    assert_eq!(flushes(&board), 1);
    board.events.clear();
    pipeline.refresh(&mut board, RefreshRequest::Changed, Some(1)).unwrap();
    assert!(board.events.contains(&Event::Clear(None)));
    assert_eq!(board.saved.partials, 0);
    board.events.clear();
    pipeline.refresh(&mut board, RefreshRequest::Full, None).unwrap();
    assert!(board.events.contains(&Event::Clear(None)));
    assert_eq!(flushes(&board), 1);
    board.assert_full_render(&pipeline);
}

#[test]
fn a_new_empty_registry_erases_views_retained_from_the_previous_wake() {
    let mut board = displayed(&scene([0, 4, 8]));
    let empty = Pipeline::new();
    empty.refresh(&mut board, RefreshRequest::Changed, None).unwrap();
    assert_eq!(board.saved.frame, empty.state());
    assert_eq!(flushes(&board), 1);
    assert!(!board.events.contains(&Event::Draw));
    board.assert_full_render(&empty);
}

#[test]
fn invalid_pending_views_are_validated_when_their_scope_is_selected() {
    let mut pipeline = scene([0, 4, 8]);
    let mut board = displayed(&pipeline);
    pipeline.register(0, solid(area(10, 10, 40, 40), 2)).unwrap();
    pipeline
        .register(
            2,
            View::new(
                area(210, 10, 40, 40),
                vec![Element::line(Point::zero(), "too long for this view", Font::Text, 0)],
            ),
        )
        .unwrap();
    pipeline.refresh(&mut board, RefreshRequest::Views(1), None).unwrap();
    let saved = board.saved;
    let pixels = board.pixels.pixels().to_vec();
    board.events.clear();
    assert!(matches!(
        pipeline.refresh(&mut board, RefreshRequest::Changed, None),
        Err(RenderError::InvalidLayout(_))
    ));
    assert_eq!(board.saved, saved);
    assert_eq!(board.pixels.pixels(), pixels);
    assert!(board.events.is_empty());
}

#[test]
fn scoped_failures_force_full_recovery_even_for_an_empty_followup_request() {
    for failure in [Failure::Clear, Failure::Draw, Failure::Flush] {
        let pipeline = scene([0, 4, 8]);
        let mut board = displayed(&pipeline);
        board.failure = Some(failure);
        assert!(pipeline
            .refresh(&mut board, RefreshRequest::Region(area(15, 15, 5, 5)), None)
            .is_err());
        assert_eq!(board.saved, refresh::EMPTY);
        board.failure = None;
        board.events.clear();
        pipeline.refresh(&mut board, RefreshRequest::Views(0), None).unwrap();
        assert!(board.events.contains(&Event::Clear(None)));
        assert_eq!(flushes(&board), 1);
        board.assert_full_render(&pipeline);
    }
}
