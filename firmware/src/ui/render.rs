//! Prepare and validate the complete selection before drawing any pixels.
use super::{covers, selected, Element, Font, Pipeline, TextStyle, ViewSet};
use alloc::vec::Vec;
use core::fmt;
use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};
use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};

#[derive(Debug)]
pub enum RenderError<E> {
    Target(&'static str, E),
    QrEncoding,
    InvalidLayout(&'static str),
}
impl<E> From<E> for RenderError<E> {
    fn from(error: E) -> Self {
        Self::Target("drawing", error)
    }
}
impl<E: fmt::Debug> fmt::Display for RenderError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(operation, error) => write!(f, "{operation} failed: {error:?}"),
            Self::QrEncoding => f.write_str("QR URL cannot be encoded"),
            Self::InvalidLayout(reason) => write!(f, "invalid view: {reason}"),
        }
    }
}
impl<E: fmt::Debug> core::error::Error for RenderError<E> {}
type RenderResult<E> = Result<(), RenderError<E>>;

/// Validated draw commands in layer order, with QR modules encoded once per update.
pub(super) struct PreparedFrame<'a> {
    elements: Vec<PreparedElement<'a>>,
}

enum PreparedElement<'a> {
    Text { position: Point, text: &'a str, style: TextStyle },
    Fill { area: Rectangle, ink: u8 },
    Qr(PreparedQr),
}

impl<'a> PreparedFrame<'a> {
    fn prepare_text<E>(
        &mut self,
        area: Rectangle,
        position: Point,
        text: &'a str,
        style: TextStyle,
    ) -> RenderResult<E> {
        validate_text(area.size, position, text, style)?;
        if !text.is_empty() {
            self.elements.push(PreparedElement::Text {
                position: area.top_left + position,
                text,
                style,
            });
        }
        Ok(())
    }

    /// Execute only prepared commands; all layout checks and encoding have already succeeded.
    pub(super) fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
    ) -> RenderResult<D::Error> {
        for element in &self.elements {
            match element {
                PreparedElement::Text { position, text, style } => {
                    style.font.draw(target, text, *position, style.ink)?;
                }
                PreparedElement::Fill { area, ink } => {
                    target.fill_solid(area, Gray4::new(*ink))?;
                }
                PreparedElement::Qr(qr) => qr.draw(target)?,
            }
        }
        Ok(())
    }
}

impl Pipeline<'_> {
    pub fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        views: ViewSet,
    ) -> RenderResult<D::Error> {
        self.prepare(target.bounding_box(), views)?.draw(target)
    }

    pub(super) fn prepare<E>(
        &self,
        target_bounds: Rectangle,
        views: ViewSet,
    ) -> Result<PreparedFrame<'_>, RenderError<E>> {
        let mut prepared = PreparedFrame { elements: Vec::new() };
        for id in selected(views) {
            let Some(view) = self.get(id) else {
                continue;
            };
            if view.area.is_zero_sized() {
                return Err(RenderError::InvalidLayout("view bounds must be nonempty"));
            }
            check_rect(view.area, target_bounds)?;
            let bounds = Rectangle::new(Point::zero(), view.area.size);
            for element in &view.content {
                match element {
                    Element::Text { position, text, style } => {
                        prepared.prepare_text(view.area, *position, text, *style)?;
                    }
                    Element::Fill { area, ink } => {
                        check_ink(*ink)?;
                        check_rect(*area, bounds)?;
                        prepared.elements.push(PreparedElement::Fill {
                            area: area.translate(view.area.top_left),
                            ink: *ink,
                        });
                    }
                    Element::Qr {
                        bottom_left,
                        module_size,
                        url,
                        caption,
                        caption_font,
                        caption_gap,
                        ink,
                        caption_ink,
                    } => {
                        let mut qr =
                            PreparedQr::new(url, *bottom_left, *module_size, *ink, bounds)?;
                        prepared.prepare_text(
                            view.area,
                            qr.caption_position(*caption_gap, *caption_font)?,
                            caption,
                            TextStyle { font: *caption_font, ink: *caption_ink, single_line: true },
                        )?;
                        qr.origin += view.area.top_left;
                        prepared.elements.push(PreparedElement::Qr(qr));
                    }
                }
            }
        }
        Ok(prepared)
    }
}

struct PreparedQr {
    origin: Point,
    size: i32,
    module_size: i32,
    modules: Vec<bool>,
    ink: u8,
}

impl PreparedQr {
    fn new<E>(
        url: &str,
        bottom_left: Point,
        module_size: u32,
        ink: u8,
        bounds: Rectangle,
    ) -> Result<Self, RenderError<E>> {
        check_ink(ink)?;
        if module_size == 0 || module_size > bounds.size.height {
            return Err(RenderError::InvalidLayout("QR module size must fit its view"));
        }
        let module_size = i32::try_from(module_size)
            .map_err(|_| RenderError::InvalidLayout("QR dimensions overflow"))?;
        let mut temporary = [0u8; Version::MAX.buffer_len()];
        let mut buffer = [0u8; Version::MAX.buffer_len()];
        let qr = QrCode::encode_text(
            url,
            &mut temporary,
            &mut buffer,
            QrCodeEcc::Medium,
            Version::MIN,
            Version::MAX,
            None,
            true,
        )
        .map_err(|_| RenderError::QrEncoding)?;
        let size = qr.size();
        let side = size
            .checked_mul(module_size)
            .ok_or(RenderError::InvalidLayout("QR dimensions overflow"))?;
        let origin = Point::new(
            bottom_left.x,
            bottom_left
                .y
                .checked_sub(side)
                .ok_or(RenderError::InvalidLayout("QR coordinates overflow"))?,
        );
        check_rect(Rectangle::new(origin, Size::new(side as u32, side as u32)), bounds)?;
        let modules =
            (0..size * size).map(|index| qr.get_module(index % size, index / size)).collect();
        Ok(Self { origin, size, module_size, modules, ink })
    }

    /// Right of the code, vertically centred on it.
    fn caption_position<E>(&self, gap: i32, font: Font) -> Result<Point, RenderError<E>> {
        let side = self.size * self.module_size;
        let x = i32::try_from(i64::from(self.origin.x) + i64::from(side) + i64::from(gap))
            .map_err(|_| RenderError::InvalidLayout("QR caption coordinates overflow"))?;
        Ok(Point::new(x, self.origin.y + side / 2 - font.size().height as i32 / 2))
    }

    fn draw<D: DrawTarget<Color = Gray4>>(&self, target: &mut D) -> Result<(), D::Error> {
        let module = Size::new_equal(self.module_size as u32);
        for (index, _) in self.modules.iter().enumerate().filter(|(_, &dark)| dark) {
            let (x, y) = (index as i32 % self.size, index as i32 / self.size);
            let top_left = self.origin + Point::new(x, y) * self.module_size;
            target.fill_solid(&Rectangle::new(top_left, module), Gray4::new(self.ink))?;
        }
        Ok(())
    }
}

fn validate_text<E>(
    view_size: Size,
    position: Point,
    text: &str,
    style: TextStyle,
) -> RenderResult<E> {
    check_ink(style.ink)?;
    if style.single_line && text.contains('\n') {
        return Err(RenderError::InvalidLayout("line contains a newline"));
    }
    if text.is_empty() {
        return Ok(());
    }
    let (mut columns, mut lines) = (0usize, 0usize);
    for line in text.split('\n') {
        columns = columns.max(line.chars().count());
        lines += 1;
    }
    let size = style.font.size();
    let width = u32::try_from(columns).ok().and_then(|n| n.checked_mul(size.width));
    let height = u32::try_from(lines).ok().and_then(|n| n.checked_mul(size.height));
    let (Some(width), Some(height)) = (width, height) else {
        return Err(RenderError::InvalidLayout("text dimensions overflow"));
    };
    check_rect(
        Rectangle::new(position, Size::new(width, height)),
        Rectangle::new(Point::zero(), view_size),
    )?;
    Ok(())
}

fn check_rect<E>(inner: Rectangle, outer: Rectangle) -> RenderResult<E> {
    if !covers(outer, inner) {
        return Err(RenderError::InvalidLayout("content exceeds its bounds"));
    }
    Ok(())
}
fn check_ink<E>(ink: u8) -> RenderResult<E> {
    if ink <= 15 {
        Ok(())
    } else {
        Err(RenderError::InvalidLayout("ink must be between 0 and 15"))
    }
}
