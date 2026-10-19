//! A bounded view registry with one rendering and refresh transaction.
//!
//! A [`Pipeline`] holds up to [`MAX_VIEWS`] views in stable slots; higher IDs draw on top.
//! Each [`View`] has panel bounds and a draw list of text, filled rectangles, and QR elements
//! positioned relative to the view, so changing `view.area.top_left` moves all its content.
//! Text can borrow strings or own runtime strings. The renderer knows nothing about the badge.
//!
//! `register(id, view)` adds or replaces a view, `get_mut(id)` edits its `area` or `content`
//! in place, and `deregister(id)` removes it. No explicit invalidation is needed: every
//! [`Pipeline::refresh`] compares each view's bounds and 64-bit content signature with the
//! records retained for the panel. Text signatures use the font's `?` fallback, so text that
//! draws identically compares equal; QR URLs keep their UTF-8.
//!
//! ```ignore
//! let mut pipeline = badge::frame(&config::BADGE, &config::LAYOUT, volts)?;
//! let area = pipeline.get(badge::DATA).unwrap().area;
//! let status = Element::line(Point::new(0, 8), format!("Status: {status}"), Font::Text, 0);
//! pipeline.register(badge::DATA, View::new(area, vec![status]))?;
//! pipeline.refresh(&mut board, RefreshRequest::Changed, resume, FULL_REFRESH_AFTER_PARTIALS)?;
//! ```
//!
//! | [`RefreshRequest`] | Behavior |
//! |---|---|
//! | `Changed` | Present changed views; return early when nothing differs |
//! | `Views(mask)` | Force redraw of selected IDs, e.g. `1 << badge::BATTERY` |
//! | `Region(rect)` | Invalidate any rectangle inside the draw target, including empty background |
//! | `Full` | Force a complete framebuffer/panel clear and redraw |
//!
//! Regions expand to complete affected views, including overlaps and old locations; they are
//! not pixel clips. Zero-sized regions and empty masks do nothing unless recovery or cleanup is
//! due; out-of-bounds regions fail before any panel write. Cold boot, reset, invalid retained
//! state, and periodic cleanup override scoped requests with a full redraw. Scoped updates
//! commit only the records they presented, so unrelated changes stay pending.
//!
//! Partial updates erase the old bounds of changed or removed views and the current bounds of
//! redrawn ones, expanding the selection until every overlapping layer is included, then redraw
//! in ID order and flush once. Duplicate and contained clear rectangles are skipped.
//!
//! The whole selection is validated (bounds, ink, text size, QR encoding) before the first
//! panel write, so validation errors leave the image and retained records intact. The
//! pipeline invalidates retained state before changing the panel and commits only after a
//! successful flush; a failed update therefore forces full recovery on the next refresh.
//! [`Pipeline::draw`] renders a [`ViewSet`] mask directly, without retained state.
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

mod font;
pub mod refresh;
mod render;
mod view;
pub use font::Font;
pub use refresh::RefreshRequest;
use refresh::{FrameState, RefreshKind, SavedState, ViewState};
pub use render::RenderError;
use view::{covers, overlaps, selected};
pub use view::{Element, TextStyle, View, ViewId, ViewSet, MAX_VIEWS};

/// Display and retained storage operations. The caller manages panel power.
pub trait Backend {
    type Error;
    type Target: DrawTarget<Color = Gray4, Error = Self::Error>;

    fn target(&mut self) -> &mut Self::Target;
    fn load_saved(&mut self) -> SavedState;
    fn store_saved(&mut self, saved: SavedState);
    /// None resets the whole framebuffer and panel; Some clears only that region.
    fn clear(&mut self, area: Option<Rectangle>) -> Result<(), Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
}

#[derive(Clone)]
pub struct Pipeline<'a> {
    views: [Option<View<'a>>; MAX_VIEWS],
}
impl Default for Pipeline<'_> {
    fn default() -> Self {
        Self { views: core::array::from_fn(|_| None) }
    }
}
impl<'a> Pipeline<'a> {
    pub fn new() -> Self {
        Self::default()
    }
    /// Register or replace a view under a stable ID. Higher IDs draw on top.
    pub fn register(
        &mut self,
        id: ViewId,
        view: View<'a>,
    ) -> Result<Option<View<'a>>, &'static str> {
        Ok(self.views.get_mut(id).ok_or("view ID exceeds capacity")?.replace(view))
    }
    pub fn deregister(&mut self, id: ViewId) -> Result<Option<View<'a>>, &'static str> {
        Ok(self.views.get_mut(id).ok_or("view ID exceeds capacity")?.take())
    }
    pub fn get(&self, id: ViewId) -> Option<&View<'a>> {
        self.views.get(id)?.as_ref()
    }
    /// Edits need no invalidation: refresh compares every view's signature with the panel record.
    pub fn get_mut(&mut self, id: ViewId) -> Option<&mut View<'a>> {
        self.views.get_mut(id)?.as_mut()
    }
    pub fn state(&self) -> FrameState {
        core::array::from_fn(|id| self.views[id].as_ref().map_or(ViewState::EMPTY, View::state))
    }

    /// Present a refresh scope on a powered panel. Recovery and cleanup override the scope.
    /// The caller owns timing and shutdown. Commit only records fully presented by this call.
    pub fn refresh<B: Backend>(
        &self,
        backend: &mut B,
        request: RefreshRequest,
        resume: bool,
        full_after: Option<u32>,
    ) -> Result<(), RenderError<B::Error>> {
        if let RefreshRequest::Region(area) = request {
            if !covers(backend.target().bounding_box(), area) {
                return Err(RenderError::InvalidLayout("refresh region exceeds target bounds"));
            }
        }
        let saved = backend.load_saved();
        let Some(update) = refresh::plan_request(saved, resume, &self.state(), full_after, request)
        else {
            return Ok(());
        };
        let views = update.kind.views();
        let prepared = self.prepare(backend.target().bounding_box(), views)?;
        // From the first panel change until a successful flush, recovery needs a full redraw.
        backend.store_saved(refresh::EMPTY);
        match update.kind {
            RefreshKind::Full => {
                backend.clear(None).map_err(|error| RenderError::Target("display clear", error))?;
            }
            RefreshKind::Partial(_) => {
                for area in update.clear_areas() {
                    backend
                        .clear(Some(area))
                        .map_err(|error| RenderError::Target("view clear", error))?;
                }
            }
        }
        prepared.draw(backend.target())?;
        backend.flush().map_err(|error| RenderError::Target("display flush", error))?;
        backend.store_saved(update.next_saved);
        Ok(())
    }
}
