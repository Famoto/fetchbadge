//! A bounded view registry with one rendering and refresh transaction.
use core::{
    cell::Cell,
    ops::{Deref, DerefMut},
};
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};

mod font;
pub mod refresh;
mod render;
mod view;
pub use font::Font;
pub use refresh::RefreshRequest;
use refresh::{FrameState, RefreshKind, SavedState, ViewState};
pub use render::RenderError;
use view::{covers, selected};
pub use view::{Element, ElementSink, TextStyle, View, ViewId, ViewSet, MAX_VIEWS};

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
    state: FrameState,
    dirty: Cell<ViewSet>,
    observed: Cell<Option<u64>>,
}
impl Default for Pipeline<'_> {
    fn default() -> Self {
        Self {
            views: core::array::from_fn(|_| None),
            state: [ViewState::EMPTY; MAX_VIEWS],
            // A new scene must also detect views removed since the last wake.
            dirty: Cell::new(ViewSet::MAX),
            observed: Cell::new(None),
        }
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
        let slot = self.views.get_mut(id).ok_or("view ID exceeds capacity")?;
        let state = view.state();
        if self.state[id] != state {
            self.dirty.set(self.dirty.get() | (1 << id));
        }
        self.state[id] = state;
        Ok(slot.replace(view))
    }
    pub fn deregister(&mut self, id: ViewId) -> Result<Option<View<'a>>, &'static str> {
        let old = self.views.get_mut(id).ok_or("view ID exceeds capacity")?.take();
        if old.is_some() {
            self.state[id] = ViewState::EMPTY;
            self.dirty.set(self.dirty.get() | (1 << id));
        }
        Ok(old)
    }
    pub fn get(&self, id: ViewId) -> Option<&View<'a>> {
        self.views.get(id)?.as_ref()
    }
    /// Dropping the guard updates only this view's cached signature and redraw request.
    pub fn get_mut(&mut self, id: ViewId) -> Option<ViewMut<'_, 'a>> {
        Some(ViewMut {
            view: self.views.get_mut(id)?.as_mut()?,
            state: &mut self.state[id],
            dirty: &self.dirty,
            bit: 1 << id,
        })
    }
    pub fn state(&self) -> FrameState {
        self.state
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
        // Dirty bits are trusted only for the display state last seen by this registry.
        let candidates = if self.observed.get() == Some(saved.checksum) {
            self.dirty.get()
        } else {
            ViewSet::MAX
        };
        let Some(update) =
            refresh::plan_request(saved, resume, &self.state, full_after, request, candidates)
        else {
            if request == RefreshRequest::Changed {
                self.dirty.set(0);
                self.observed.set(Some(saved.checksum));
            }
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
        self.dirty.set((self.dirty.get() | candidates) & !update.applied);
        self.observed.set(Some(update.next_saved.checksum));
        Ok(())
    }
}

pub struct ViewMut<'p, 'a> {
    view: &'p mut View<'a>,
    state: &'p mut ViewState,
    dirty: &'p Cell<ViewSet>,
    bit: ViewSet,
}
impl<'a> Deref for ViewMut<'_, 'a> {
    type Target = View<'a>;
    fn deref(&self) -> &Self::Target {
        self.view
    }
}
impl DerefMut for ViewMut<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.view
    }
}
impl Drop for ViewMut<'_, '_> {
    fn drop(&mut self) {
        let state = self.view.state();
        if *self.state != state {
            *self.state = state;
            self.dirty.set(self.dirty.get() | self.bit);
        }
    }
}
