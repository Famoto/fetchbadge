use nametag_preview::{
    app::{run, Hardware},
    refresh::{self, RefreshKind, SavedState, UpdatePlan},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Failure {
    Render,
    Flush,
}

#[derive(Debug, PartialEq, Eq)]
enum Event {
    PowerOn,
    Measure,
    Store(SavedState),
    Render(RefreshKind),
    Flush,
    PowerOff,
}

struct TestBoard {
    saved: SavedState,
    volts: f32,
    failure: Option<Failure>,
    powered: bool,
    flushed: bool,
    events: Vec<Event>,
}

impl TestBoard {
    fn new(saved: SavedState, volts: f32) -> Self {
        Self { saved, volts, failure: None, powered: false, flushed: false, events: Vec::new() }
    }
}

impl Hardware for TestBoard {
    type Error = Failure;

    fn measure(&mut self) -> f32 {
        assert!(!self.powered);
        self.powered = true;
        self.flushed = false;
        self.events.push(Event::PowerOn);
        self.events.push(Event::Measure);
        self.volts
    }

    fn load_saved(&mut self) -> SavedState {
        self.saved
    }

    fn store_saved(&mut self, saved: SavedState) {
        assert!(saved == refresh::EMPTY || self.flushed, "commit before flush");
        self.saved = saved;
        self.events.push(Event::Store(saved));
    }

    fn render(&mut self, update: &UpdatePlan) -> Result<(), Failure> {
        assert!(self.powered);
        assert_eq!(self.saved, refresh::EMPTY, "invalidate before changing the panel");
        assert_eq!(update.volts, self.volts);
        self.events.push(Event::Render(update.kind));
        if self.failure == Some(Failure::Render) {
            return Err(Failure::Render);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Failure> {
        assert!(self.powered);
        assert_eq!(self.saved, refresh::EMPTY);
        self.events.push(Event::Flush);
        if self.failure == Some(Failure::Flush) {
            return Err(Failure::Flush);
        }
        self.flushed = true;
        Ok(())
    }

    fn power_off(&mut self) {
        assert!(self.powered);
        self.powered = false;
        self.events.push(Event::PowerOff);
    }
}

#[test]
fn successful_updates_commit_after_flush_then_shut_down() {
    let saved = refresh::plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    for (previous, resume, full_after, expected_kind) in [
        (refresh::EMPTY, true, Some(24), RefreshKind::Full),
        (saved, false, Some(24), RefreshKind::Full),
        (saved, true, Some(24), RefreshKind::Battery),
        (saved, true, Some(0), RefreshKind::Full),
    ] {
        let mut board = TestBoard::new(previous, 3.97);
        let expected_saved =
            refresh::plan(previous, resume, board.volts, full_after).unwrap().next_saved;

        assert_eq!(run(&mut board, resume, full_after), Ok(()));

        assert_eq!(board.saved, expected_saved);
        assert_eq!(
            board.events,
            vec![
                Event::PowerOn,
                Event::Measure,
                Event::Store(refresh::EMPTY),
                Event::Render(expected_kind),
                Event::Flush,
                Event::Store(expected_saved),
                Event::PowerOff,
            ]
        );
    }
}

#[test]
fn unchanged_reading_preserves_retained_state_and_skips_panel_writes() {
    let saved = refresh::plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    let saved = refresh::plan(saved, true, 3.97, Some(24)).unwrap().next_saved;
    for volts in [3.97, 3.971] {
        let mut board = TestBoard::new(saved, volts);
        assert_eq!(run(&mut board, true, Some(24)), Ok(()));
        assert_eq!(board.saved, saved);
        assert_eq!(board.events, vec![Event::PowerOn, Event::Measure, Event::PowerOff]);
    }
}

#[test]
fn failed_updates_shut_down_without_committing_and_recover_fully_next_wake() {
    let saved = refresh::plan(refresh::EMPTY, false, 3.98, Some(24)).unwrap().next_saved;
    for (previous, kind) in [(refresh::EMPTY, RefreshKind::Full), (saved, RefreshKind::Battery)] {
        for failure in [Failure::Render, Failure::Flush] {
            let mut board = TestBoard::new(previous, 3.97);
            board.failure = Some(failure);

            assert_eq!(run(&mut board, true, Some(24)), Err(failure));

            assert!(!board.powered, "return errors only after panel shutdown");
            assert_eq!(board.saved, refresh::EMPTY);
            let mut expected = vec![
                Event::PowerOn,
                Event::Measure,
                Event::Store(refresh::EMPTY),
                Event::Render(kind),
            ];
            if failure == Failure::Flush {
                expected.push(Event::Flush);
            }
            expected.push(Event::PowerOff);
            assert_eq!(board.events, expected);

            board.failure = None;
            board.events.clear();
            assert_eq!(run(&mut board, true, Some(24)), Ok(()));
            assert!(board.events.contains(&Event::Render(RefreshKind::Full)));
            assert_ne!(board.saved, refresh::EMPTY);
        }
    }
}
