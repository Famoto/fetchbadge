//! Diff registered views, including old bounds, and retain only checked integer state.
use super::{covers, selected, ViewSet, MAX_VIEWS};
use core::hash::{Hash, Hasher};
use embedded_graphics::{prelude::*, primitives::Rectangle};

/// Integer-only records are safe to load from unchecked RTC memory.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ViewState {
    pub active: u32,
    pub area: Rectangle,
    pub content: u64,
}
impl ViewState {
    pub const EMPTY: Self =
        Self { active: 0, area: Rectangle::new(Point::zero(), Size::zero()), content: 0 };
}
pub type FrameState = [ViewState; MAX_VIEWS];

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SavedState {
    pub version: u32,
    pub partials: u32,
    pub frame: FrameState,
    pub checksum: u64,
}
pub const EMPTY: SavedState =
    SavedState { version: 0, partials: 0, frame: [ViewState::EMPTY; MAX_VIEWS], checksum: 0 };
impl SavedState {
    fn checksum(&self) -> u64 {
        signature(&(self.version, self.partials, self.frame))
    }

    fn valid(&self) -> bool {
        self.version == MAGIC && self.checksum == self.checksum()
    }
}
// Bump when retained records or render semantics change.
const MAGIC: u32 = 0x4241_5406;
const EMPTY_RECT: Rectangle = Rectangle::new(Point::zero(), Size::zero());

/// Regions are invalidations: complete intersecting views and old bounds may enlarge them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshRequest {
    /// Present queued changes, comparing only edited entries in a live registry.
    Changed,
    /// Force selected IDs to redraw, including any affected neighboring layers.
    Views(ViewSet),
    /// Clear and restore a target region, expanding to complete affected views.
    Region(Rectangle),
    /// Force full panel cleanup and redraw, resetting the partial-update count.
    Full,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RefreshKind {
    Full,
    Partial(ViewSet),
}
impl RefreshKind {
    pub fn views(self) -> ViewSet {
        match self {
            Self::Full => ViewSet::MAX,
            Self::Partial(views) => views,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct UpdatePlan {
    pub kind: RefreshKind,
    clear: [Rectangle; MAX_VIEWS * 2 + 1],
    /// Records fully presented by this update, including removed views.
    pub applied: ViewSet,
    pub next_saved: SavedState,
}
impl UpdatePlan {
    pub fn clear_areas(&self) -> impl Iterator<Item = Rectangle> + '_ {
        self.clear.iter().enumerate().filter_map(|(index, &area)| {
            let redundant = self.clear.iter().enumerate().any(|(other_index, &other)| {
                other_index != index
                    && covers(other, area)
                    && (other != area || other_index < index)
            });
            (area.size.width > 0 && area.size.height > 0 && !redundant).then_some(area)
        })
    }
}

pub fn plan(
    saved: SavedState,
    resume: bool,
    current: &FrameState,
    full_after: Option<u32>,
) -> Option<UpdatePlan> {
    plan_request(saved, resume, current, full_after, RefreshRequest::Changed, ViewSet::MAX)
}

pub(super) fn plan_request(
    saved: SavedState,
    resume: bool,
    current: &FrameState,
    full_after: Option<u32>,
    request: RefreshRequest,
    candidates: ViewSet,
) -> Option<UpdatePlan> {
    let full = request == RefreshRequest::Full
        || !resume
        || !saved.valid()
        || full_after.is_some_and(|limit| saved.partials >= limit);
    let requested = match request {
        RefreshRequest::Changed => selected(candidates)
            .fold(0, |mask, id| mask | if saved.frame[id] != current[id] { 1 << id } else { 0 }),
        RefreshRequest::Views(views) => views,
        RefreshRequest::Region(_) | RefreshRequest::Full => 0,
    };
    if !full
        && requested == 0
        && !matches!(request, RefreshRequest::Region(area) if area.size.width > 0 && area.size.height > 0)
    {
        return None;
    }
    let mut clear = [EMPTY_RECT; MAX_VIEWS * 2 + 1];
    let mut frame = *current;
    let applied;
    let kind = if full {
        applied = ViewSet::MAX;
        RefreshKind::Full
    } else {
        let mut affected = requested;
        if let RefreshRequest::Region(area) = request {
            clear[MAX_VIEWS * 2] = area;
        }
        // Old bounds matter too: an unselected view may have moved or been removed.
        loop {
            for id in selected(affected) {
                if saved.frame[id].active != 0 {
                    clear[id] = saved.frame[id].area;
                }
                if current[id].active != 0 {
                    clear[MAX_VIEWS + id] = current[id].area;
                }
            }
            let before = affected;
            for (id, (old, new)) in saved.frame.iter().zip(current).enumerate() {
                if [old, new].iter().any(|view| {
                    view.active != 0 && clear.iter().any(|&area| overlaps(area, view.area))
                }) {
                    affected |= 1 << id;
                }
            }
            if affected == before {
                break;
            }
        }
        if !clear.iter().any(|area| area.size.width > 0 && area.size.height > 0) {
            return None;
        }
        applied = affected;
        frame = saved.frame;
        let mut draw = 0;
        for id in selected(affected) {
            frame[id] = current[id];
            if current[id].active != 0 {
                draw |= 1 << id;
            }
        }
        RefreshKind::Partial(draw)
    };
    let mut next_saved = SavedState {
        version: MAGIC,
        partials: if matches!(kind, RefreshKind::Partial(_)) {
            saved.partials.saturating_add(1)
        } else {
            0
        },
        frame,
        checksum: 0,
    };
    next_saved.checksum = next_saved.checksum();
    Some(UpdatePlan { kind, clear, applied, next_saved })
}

fn overlaps(first: Rectangle, second: Rectangle) -> bool {
    let (x, y) = (i64::from(first.top_left.x), i64::from(first.top_left.y));
    let (left, top) = (i64::from(second.top_left.x), i64::from(second.top_left.y));
    first.size.width > 0
        && first.size.height > 0
        && second.size.width > 0
        && second.size.height > 0
        && x < left + i64::from(second.size.width)
        && left < x + i64::from(first.size.width)
        && y < top + i64::from(second.size.height)
        && top < y + i64::from(first.size.height)
}

pub(super) struct Fingerprint(u64);
impl Default for Fingerprint {
    fn default() -> Self {
        Self(0xcbf2_9ce4_8422_2325)
    }
}
impl Hasher for Fingerprint {
    fn finish(&self) -> u64 {
        self.0
    }
    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0 ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
}

pub(super) fn signature(value: &impl Hash) -> u64 {
    let mut hash = Fingerprint::default();
    value.hash(&mut hash);
    hash.finish()
}
