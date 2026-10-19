use embedded_graphics::prelude::*;
use nametag_preview::{
    app::{run, Hardware},
    badge, config,
    ui::{
        refresh::{self, RefreshKind, SavedState, UpdatePlan},
        Element, Font, RefreshRequest, RenderError,
    },
};
mod support;
use support::{Event, Failure, TestBoard};

fn assert_failure(result: Result<(), RenderError<Failure>>, expected: Failure) {
    assert!(matches!(result, Err(RenderError::Target(_, failure)) if failure == expected));
}

fn plan(
    saved: SavedState,
    resume: bool,
    volts: f32,
    full_after: Option<u32>,
) -> Option<UpdatePlan> {
    refresh::plan(
        saved,
        resume,
        &badge::frame(&config::BADGE, &config::LAYOUT, volts).unwrap().state(),
        full_after,
    )
}

#[test]
fn prepared_frames_can_update_repeatedly_without_another_measure_or_shutdown() {
    let mut board = TestBoard::new(refresh::EMPTY, 3.98);
    let volts = board.measure();
    let frame = badge::frame(&config::BADGE, &config::LAYOUT, volts).unwrap();
    frame.refresh(&mut board, RefreshRequest::Changed, true, Some(24)).unwrap();
    assert!(board.events.contains(&Event::Clear(None)));
    for (status, changed) in [("ready", true), ("ready", false), ("busy", true)] {
        let status = String::from(status);
        let rows = [("Status", status.as_str())];
        let badge = config::BadgeConfig { info: &rows, ..config::BADGE };
        let frame = badge::frame(&badge, &config::LAYOUT, volts).unwrap();
        board.events.clear();
        frame.refresh(&mut board, RefreshRequest::Changed, true, Some(24)).unwrap();
        assert!(board.powered);
        if changed {
            assert!(board.events.contains(&Event::Clear(Some(config::LAYOUT.data.area))));
            assert_eq!(
                board.events.iter().filter(|event| matches!(event, Event::Flush)).count(),
                1
            );
        } else {
            assert!(board.events.is_empty());
        }
        assert!(!board
            .events
            .iter()
            .any(|event| matches!(event, Event::Measure | Event::PowerOn | Event::PowerOff)));
        board.assert_full_render(&frame);
    }
    board.power_off();
}

#[test]
fn failed_awake_updates_invalidate_state_and_leave_shutdown_to_the_caller() {
    for failure in [Failure::Clear, Failure::Draw, Failure::Flush] {
        let mut board = TestBoard::new(refresh::EMPTY, 3.98);
        let mut frame = badge::frame(&config::BADGE, &config::LAYOUT, board.measure()).unwrap();
        frame.refresh(&mut board, RefreshRequest::Changed, true, None).unwrap();
        // Remove a view so recovery must erase stale pixels as well as redraw.
        frame.deregister(badge::DATA).unwrap();
        frame.get_mut(badge::LOGO).unwrap().set_content(vec![Element::text(
            Point::new(30, 0),
            "+",
            Font::Small,
            0,
        )]);
        board.failure = Some(failure);
        assert_failure(frame.refresh(&mut board, RefreshRequest::Changed, true, None), failure);
        assert!(board.powered);
        assert_eq!(board.saved, refresh::EMPTY);
        board.failure = None;
        board.events.clear();
        frame.refresh(&mut board, RefreshRequest::Changed, true, None).unwrap();
        assert!(board.events.contains(&Event::Clear(None)));
        board.assert_full_render(&frame);
        board.power_off();
    }
}

#[test]
fn successful_updates_commit_after_flush_then_shut_down() {
    let saved = plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    for (previous, resume, full_after, kind) in [
        (refresh::EMPTY, true, Some(24), RefreshKind::Full),
        (saved, false, Some(24), RefreshKind::Full),
        (saved, true, Some(24), RefreshKind::Partial(1 << badge::BATTERY)),
        (saved, true, Some(0), RefreshKind::Full),
    ] {
        let mut board = TestBoard::new(previous, 3.97);
        let update = plan(previous, resume, board.volts, full_after).unwrap();
        assert_eq!(update.kind, kind);
        run(&mut board, &config::BADGE, &config::LAYOUT, resume, full_after).unwrap();
        assert_eq!(board.saved, update.next_saved);
        let mut expected = vec![Event::PowerOn, Event::Measure, Event::Invalidate];
        match kind {
            RefreshKind::Full => expected.push(Event::Clear(None)),
            RefreshKind::Partial(_) => {
                expected.extend(update.clear_areas().map(|area| Event::Clear(Some(area))));
            }
        }
        expected.extend([Event::Draw, Event::Flush, Event::Commit, Event::PowerOff]);
        assert_eq!(board.events, expected);
    }
}

#[test]
fn unchanged_reading_preserves_retained_state_and_skips_panel_writes() {
    let saved = plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    let saved = plan(saved, true, 3.97, Some(24)).unwrap().next_saved;
    for volts in [3.97, 3.971] {
        let mut board = TestBoard::new(saved, volts);
        run(&mut board, &config::BADGE, &config::LAYOUT, true, Some(24)).unwrap();
        assert_eq!(board.saved, saved);
        assert_eq!(board.events, vec![Event::PowerOn, Event::Measure, Event::PowerOff]);
    }
}

#[test]
fn failed_updates_shut_down_without_committing_and_recover_fully_next_wake() {
    let saved = plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    let value = String::from("ready");
    let rows = [("Status", value.as_str())];
    let badge = config::BadgeConfig { info: &rows, ..config::BADGE };
    for previous in [refresh::EMPTY, saved] {
        for failure in [Failure::Clear, Failure::Draw, Failure::Flush] {
            let mut board = TestBoard::new(previous, 3.97);
            board.failure = Some(failure);
            assert_failure(run(&mut board, &badge, &config::LAYOUT, true, Some(24)), failure);
            assert!(!board.powered, "return errors only after panel shutdown");
            assert_eq!(board.saved, refresh::EMPTY);
            assert_eq!(board.events.last(), Some(&Event::PowerOff));
            assert!(!board.events.contains(&Event::Commit));
            board.failure = None;
            board.events.clear();
            run(&mut board, &badge, &config::LAYOUT, true, Some(24)).unwrap();
            assert!(board.events.contains(&Event::Clear(None)));
            assert_ne!(board.saved, refresh::EMPTY);
            board.assert_full_render(&badge::frame(&badge, &config::LAYOUT, 3.97).unwrap());
        }
    }
}

#[test]
fn runtime_data_and_battery_update_together_with_one_commit() {
    let saved = plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    let value = String::from("ready");
    let rows = [("Status", value.as_str())];
    let badge = config::BadgeConfig { info: &rows, ..config::BADGE };
    let mut board = TestBoard::new(saved, 3.97);
    run(&mut board, &badge, &config::LAYOUT, true, Some(24)).unwrap();
    assert_eq!(
        board.events,
        [
            Event::PowerOn,
            Event::Measure,
            Event::Invalidate,
            Event::Clear(Some(config::LAYOUT.data.area)),
            Event::Clear(Some(config::LAYOUT.battery.area)),
            Event::Draw,
            Event::Flush,
            Event::Commit,
            Event::PowerOff,
        ]
    );
    assert_ne!(board.saved, saved);
    board.events.clear();
    run(&mut board, &badge, &config::LAYOUT, true, Some(24)).unwrap();
    assert_eq!(board.events, [Event::PowerOn, Event::Measure, Event::PowerOff]);
}

#[test]
fn deregistering_the_last_view_still_clears_flushes_and_commits() {
    let mut board = TestBoard::new(refresh::EMPTY, 3.98);
    let mut pipeline = badge::frame(&config::BADGE, &config::LAYOUT, board.measure()).unwrap();
    pipeline.refresh(&mut board, RefreshRequest::Changed, true, None).unwrap();
    for id in 0..6 {
        pipeline.deregister(id).unwrap();
    }
    board.events.clear();
    pipeline.refresh(&mut board, RefreshRequest::Changed, true, None).unwrap();
    assert_eq!(
        board.events.iter().filter(|event| matches!(event, Event::Clear(Some(_)))).count(),
        6
    );
    assert!(!board.events.contains(&Event::Draw));
    assert_eq!(board.events.iter().filter(|event| matches!(event, Event::Flush)).count(), 1);
    board.assert_full_render(&pipeline);
    board.events.clear();
    pipeline.refresh(&mut board, RefreshRequest::Changed, true, None).unwrap();
    assert!(board.events.is_empty());
    board.power_off();
}

#[test]
fn invalid_row_spacing_preserves_display_state_and_powers_off() {
    let mut board = TestBoard::new(refresh::EMPTY, 3.98);
    run(&mut board, &config::BADGE, &config::LAYOUT, false, None).unwrap();
    let saved = board.saved;
    let pixels = board.pixels.pixels().to_vec();
    for resume in [false, true] {
        for spacing in [0, 16] {
            let layout = config::LayoutConfig { row_spacing: spacing, ..config::LAYOUT };
            board.events.clear();
            assert!(matches!(
                run(&mut board, &config::BADGE, &layout, resume, None),
                Err(RenderError::InvalidLayout("information rows overlap"))
            ));
            assert_eq!(board.saved, saved);
            assert_eq!(board.pixels.pixels(), pixels);
            assert!(!board.powered);
            assert_eq!(board.events, vec![Event::PowerOn, Event::Measure, Event::PowerOff]);
        }
    }
}

#[test]
fn qr_encoding_failure_preserves_display_state_before_any_panel_writes() {
    let mut board = TestBoard::new(refresh::EMPTY, 3.98);
    let mut pipeline = badge::frame(&config::BADGE, &config::LAYOUT, board.measure()).unwrap();
    pipeline.refresh(&mut board, RefreshRequest::Changed, true, None).unwrap();
    let saved = board.saved;
    let pixels = board.pixels.pixels().to_vec();

    pipeline.get_mut(badge::LOGO).unwrap().set_content(vec![Element::text(
        Point::new(30, 0),
        "+",
        Font::Small,
        0,
    )]);
    let mut invalid_qr = pipeline.get(badge::BONUS).unwrap().clone();
    if let Element::Qr { url, .. } = &mut invalid_qr.content_mut()[0] {
        *url = "x".repeat(10_000).into();
    }
    pipeline.register(15, invalid_qr).unwrap();
    board.events.clear();
    assert!(matches!(
        pipeline.refresh(&mut board, RefreshRequest::Changed, true, None),
        Err(RenderError::QrEncoding)
    ));
    assert_eq!(board.saved, saved);
    assert_eq!(board.pixels.pixels(), pixels);
    assert!(board.events.is_empty());
    board.power_off();
}

#[test]
fn invalid_content_preserves_panel_and_saved_state_before_shutting_down() {
    let mut board = TestBoard::new(refresh::EMPTY, 3.98);
    let long = "x".repeat(40);
    let invalid = config::BadgeConfig { qr_caption: &long, ..config::BADGE };
    for resume in [false, true] {
        run(&mut board, &config::BADGE, &config::LAYOUT, false, None).unwrap();
        let pixels = board.pixels.pixels().to_vec();
        let saved = board.saved;
        board.events.clear();
        assert!(matches!(
            run(&mut board, &invalid, &config::LAYOUT, resume, None),
            Err(RenderError::InvalidLayout(_))
        ));
        assert_eq!(board.saved, saved);
        assert_eq!(board.pixels.pixels(), pixels);
        assert_eq!(board.events, vec![Event::PowerOn, Event::Measure, Event::PowerOff]);
    }
}
