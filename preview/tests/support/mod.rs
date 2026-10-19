use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};
use nametag_preview::{
    app::Hardware,
    config,
    ui::{
        refresh::{self, SavedState},
        Backend, Pipeline, ViewSet,
    },
    Framebuffer,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Clear,
    Draw,
    Flush,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Event {
    PowerOn,
    Measure,
    Invalidate,
    Commit,
    Clear(Option<Rectangle>),
    Draw,
    Flush,
    PowerOff,
}

pub struct TestBoard {
    pub saved: SavedState,
    pub volts: f32,
    pub failure: Option<Failure>,
    pub powered: bool,
    flushed: bool,
    drawing: bool,
    pub pixels: Framebuffer,
    pub events: Vec<Event>,
}

impl TestBoard {
    pub fn new(saved: SavedState, volts: f32) -> Self {
        Self {
            saved,
            volts,
            failure: None,
            powered: false,
            flushed: false,
            drawing: false,
            pixels: Framebuffer::default(),
            events: Vec::new(),
        }
    }

    fn check_write(&self) {
        assert!(self.powered);
        assert_eq!(self.saved, refresh::EMPTY, "invalidate before changing the panel");
    }

    pub fn assert_full_render(&self, pipeline: &Pipeline<'_>) {
        let mut expected = Framebuffer::default();
        pipeline.draw(&mut expected, ViewSet::MAX).unwrap();
        assert_eq!(self.pixels.pixels(), expected.pixels());
    }
}

impl Hardware for TestBoard {
    fn measure(&mut self) -> f32 {
        assert!(!self.powered);
        self.powered = true;
        self.flushed = false;
        self.events.extend([Event::PowerOn, Event::Measure]);
        self.volts
    }

    fn power_off(&mut self) {
        assert!(self.powered);
        self.powered = false;
        self.events.push(Event::PowerOff);
    }
}

impl Backend for TestBoard {
    type Error = Failure;
    type Target = Self;

    fn target(&mut self) -> &mut Self {
        self
    }

    fn load_saved(&mut self) -> SavedState {
        self.saved
    }

    fn store_saved(&mut self, saved: SavedState) {
        assert!(saved == refresh::EMPTY || self.flushed, "commit before flush");
        if saved == refresh::EMPTY {
            self.flushed = false;
            self.drawing = false;
        }
        self.saved = saved;
        self.events.push(if saved == refresh::EMPTY { Event::Invalidate } else { Event::Commit });
    }

    fn clear(&mut self, area: Option<Rectangle>) -> Result<(), Failure> {
        self.check_write();
        self.events.push(Event::Clear(area));
        self.pixels.fill_solid(&area.unwrap_or(self.bounding_box()), Gray4::WHITE).unwrap();
        if self.failure == Some(Failure::Clear) {
            return Err(Failure::Clear);
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), Failure> {
        self.check_write();
        self.events.push(Event::Flush);
        if self.failure == Some(Failure::Flush) {
            return Err(Failure::Flush);
        }
        self.flushed = true;
        Ok(())
    }
}

impl OriginDimensions for TestBoard {
    fn size(&self) -> Size {
        config::PANEL_SIZE
    }
}

impl DrawTarget for TestBoard {
    type Color = Gray4;
    type Error = Failure;

    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Failure> {
        self.check_write();
        if !self.drawing {
            self.events.push(Event::Draw);
            self.drawing = true;
        }
        self.pixels.draw_iter(pixels).unwrap();
        if self.failure == Some(Failure::Draw) {
            return Err(Failure::Draw);
        }
        Ok(())
    }
}
