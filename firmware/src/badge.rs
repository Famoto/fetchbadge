//! Hardware-independent badge rendering, shared with the desktop preview.
use crate::config::{
    BadgeConfig, LayoutConfig, BATTERY_EMPTY_VOLTS, BATTERY_FULL_VOLTS, PANEL_SIZE,
};
use alloc::format;
use core::fmt;
use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};

#[path = "badge/font.rs"]
mod font;
use font::{Font, SMALL, TEXT, TITLE};

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
            Self::InvalidLayout(reason) => write!(f, "invalid badge layout: {reason}"),
        }
    }
}

impl<E: fmt::Debug> core::error::Error for RenderError<E> {}
type RenderResult<E> = Result<(), RenderError<E>>;

/// Compare the exact displayed string, including rounding and percentage thresholds.
pub fn battery_text(volts: f32) -> alloc::string::String {
    let percent = if volts >= BATTERY_FULL_VOLTS {
        100
    } else {
        (((volts - BATTERY_EMPTY_VOLTS) / (BATTERY_FULL_VOLTS - BATTERY_EMPTY_VOLTS)) * 100.0)
            .clamp(0.0, 100.0) as u8
    };
    format!("{percent}% ({volts:.2}V)")
}

/// Entire battery row, including space to erase a previously longer value.
pub fn battery_area(badge: &BadgeConfig, layout: &LayoutConfig) -> Result<Rectangle, &'static str> {
    let y = i64::from(layout.info_y) + badge.info.len() as i64 * i64::from(layout.row_spacing);
    if layout.column_x < 0
        || layout.column_x >= PANEL_SIZE.width as i32
        || y < 0
        || y + i64::from(TEXT.height) > i64::from(PANEL_SIZE.height)
    {
        return Err("battery row must fit the panel");
    }
    let origin = Point::new(layout.column_x, y as i32);
    let size = Size::new(PANEL_SIZE.width - layout.column_x as u32, TEXT.height as u32);
    Ok(Rectangle::new(origin, size))
}

fn validate_layout(badge: &BadgeConfig, layout: &LayoutConfig) -> Result<Rectangle, &'static str> {
    let area = battery_area(badge, layout)?;
    if layout.key_ink > 15 || layout.value_ink > 15 {
        return Err("ink must be between 0 and 15");
    }
    if layout.qr_module_size == 0 || layout.qr_module_size > PANEL_SIZE.height {
        return Err("QR module size must fit the panel");
    }
    if !(0..=PANEL_SIZE.width as i32).contains(&layout.qr_x)
        || !(0..=PANEL_SIZE.height as i32).contains(&layout.qr_bottom)
    {
        return Err("QR anchor must be inside the panel");
    }
    if layout.row_spacing < TEXT.height {
        return Err("information rows overlap");
    }
    let ramp_bottom = i64::from(area.top_left.y)
        + i64::from(layout.row_spacing)
        + i64::from(layout.ramp_gap)
        + i64::from(layout.ramp_block_size.height);
    if layout.info_y < 0 || ramp_bottom > i64::from(PANEL_SIZE.height) {
        return Err("information rows and color ramp exceed the panel height");
    }
    Ok(area)
}

/// Drawing context shared by full and battery-only renders. The caller clears and flushes.
pub struct Renderer<'a, D> {
    target: &'a mut D,
    badge: &'a BadgeConfig,
    layout: &'a LayoutConfig,
}

impl<'a, D: DrawTarget<Color = Gray4>> Renderer<'a, D> {
    pub fn new(target: &'a mut D, badge: &'a BadgeConfig, layout: &'a LayoutConfig) -> Self {
        Self { target, badge, layout }
    }

    pub fn draw(&mut self, volts: f32) -> RenderResult<D::Error> {
        let area = validate_layout(self.badge, self.layout).map_err(RenderError::InvalidLayout)?;
        let (badge, layout) = (self.badge, self.layout);
        self.key(&SMALL, badge.logo, layout.logo_origin)?;
        self.qr()?;
        let title = format!("{}@{}", badge.user, badge.host);
        self.key(&TITLE, &title, Point::new(layout.column_x, layout.title_y))?;
        self.fill(
            Point::new(layout.column_x, layout.rule_y),
            Size::new(title.chars().count() as u32 * TITLE.width as u32, layout.rule_height),
            layout.key_ink,
        )?;
        let mut y = layout.info_y;
        for &(key, value) in badge.info {
            self.row(key, value, y)?;
            y += layout.row_spacing;
        }
        self.row("Battery", &battery_text(volts), area.top_left.y)?;
        let ramp_y = area.top_left.y + layout.row_spacing + layout.ramp_gap;
        for index in 0..8 {
            self.fill(
                Point::new(layout.column_x + index * layout.ramp_step, ramp_y),
                layout.ramp_block_size,
                index as u8 * 2,
            )?;
        }
        Ok(())
    }

    /// Physically clear battery_area before drawing a battery-only update.
    pub fn draw_battery(&mut self, volts: f32) -> RenderResult<D::Error> {
        let area = validate_layout(self.badge, self.layout).map_err(RenderError::InvalidLayout)?;
        self.row("Battery", &battery_text(volts), area.top_left.y)
    }

    fn key(&mut self, font: &Font, text: &str, origin: Point) -> RenderResult<D::Error> {
        font.draw(self.target, text, origin, self.layout.key_ink).map_err(RenderError::from)
    }

    fn value(&mut self, text: &str, origin: Point) -> RenderResult<D::Error> {
        TEXT.draw(self.target, text, origin, self.layout.value_ink).map_err(RenderError::from)
    }

    fn row(&mut self, key: &str, value: &str, y: i32) -> RenderResult<D::Error> {
        let layout = self.layout;
        self.key(&TEXT, &format!("{key}:"), Point::new(layout.column_x, y))?;
        let value_x = layout.column_x + (key.chars().count() as i32 + 2) * TEXT.width;
        self.value(value, Point::new(value_x, y))
    }

    fn fill(&mut self, origin: Point, size: Size, ink: u8) -> RenderResult<D::Error> {
        Rectangle::new(origin, size)
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(ink)))
            .draw(self.target)
            .map_err(RenderError::from)
    }

    fn qr(&mut self) -> RenderResult<D::Error> {
        let (badge, layout) = (self.badge, self.layout);
        let mut temporary = [0u8; Version::MAX.buffer_len()];
        let mut buffer = [0u8; Version::MAX.buffer_len()];
        let qr = QrCode::encode_text(
            badge.qr_url,
            &mut temporary,
            &mut buffer,
            QrCodeEcc::Medium,
            Version::MIN,
            Version::MAX,
            None,
            true,
        )
        .map_err(|_| RenderError::QrEncoding)?;
        let module = layout.qr_module_size as i32;
        let side = qr.size() * module;
        let origin = Point::new(layout.qr_x, layout.qr_bottom - side);
        // validate_layout already checked both anchors and the module size.
        if origin.y < 0 || origin.x + side > PANEL_SIZE.width as i32 {
            return Err(RenderError::InvalidLayout("QR code exceeds the panel bounds"));
        }
        let size = Size::new(module as u32, module as u32);
        for y in 0..qr.size() {
            for x in 0..qr.size() {
                if qr.get_module(x, y) {
                    self.fill(origin + Point::new(x * module, y * module), size, layout.key_ink)?;
                }
            }
        }
        self.value(
            badge.qr_caption,
            origin + Point::new(side + layout.qr_caption_gap, side / 2 - TEXT.height / 2),
        )
    }
}
