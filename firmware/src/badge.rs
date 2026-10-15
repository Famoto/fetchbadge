//! Hardware-independent badge rendering, shared with the desktop preview.
use alloc::format;
use core::fmt;
use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
};
use qrcodegen_no_heap::{QrCode, QrCodeEcc, Version};

use crate::config::{
    BadgeConfig, LayoutConfig, BATTERY_EMPTY_VOLTS, BATTERY_FULL_VOLTS, PANEL_SIZE,
};

#[path = "badge/font.rs"]
mod font;
use font::{SMALL, TEXT, TITLE};

#[derive(Debug)]
pub enum RenderError<E> {
    Target(E),
    QrEncoding,
    InvalidLayout(&'static str),
}

impl<E: fmt::Debug> fmt::Display for RenderError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Target(error) => write!(f, "drawing failed: {error:?}"),
            Self::QrEncoding => f.write_str("QR URL cannot be encoded"),
            Self::InvalidLayout(reason) => write!(f, "invalid badge layout: {reason}"),
        }
    }
}

impl<E: fmt::Debug> core::error::Error for RenderError<E> {}

/// Render onto a white target. The caller owns clearing, flushing, and power management.
pub fn draw<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    badge: &BadgeConfig,
    layout: &LayoutConfig,
    volts: f32,
) -> Result<(), RenderError<D::Error>> {
    validate_layout(badge, layout).map_err(RenderError::InvalidLayout)?;
    draw_logo(target, badge, layout).map_err(RenderError::Target)?;
    draw_qr(target, badge, layout)?;
    draw_header(target, badge, layout).map_err(RenderError::Target)?;
    let battery_y = draw_info_rows(target, badge, layout).map_err(RenderError::Target)?;
    draw_battery(target, badge, layout, volts)?;
    let next_y = battery_y + layout.row_spacing;
    draw_color_ramp(target, layout, next_y).map_err(RenderError::Target)
}

/// Use the exact rendered string when deciding whether a battery refresh is needed.
pub fn battery_text(volts: f32) -> alloc::string::String {
    format!("{}% ({volts:.2}V)", battery_percent(volts))
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
    Ok(Rectangle::new(
        Point::new(layout.column_x, y as i32),
        Size::new(
            PANEL_SIZE.width - layout.column_x as u32,
            TEXT.height as u32,
        ),
    ))
}

/// Draw only battery text. The caller must physically clear `battery_area` first.
pub fn draw_battery<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    badge: &BadgeConfig,
    layout: &LayoutConfig,
    volts: f32,
) -> Result<(), RenderError<D::Error>> {
    validate_layout(badge, layout).map_err(RenderError::InvalidLayout)?;
    let area = battery_area(badge, layout).map_err(RenderError::InvalidLayout)?;
    draw_info_row(
        target,
        layout,
        "Battery",
        &battery_text(volts),
        area.top_left.y,
    )
    .map_err(RenderError::Target)
}

/// Approximate linear LiPo charge estimate, clamped at the configured voltage limits.
fn battery_percent(volts: f32) -> u8 {
    if volts >= BATTERY_FULL_VOLTS {
        return 100;
    }
    (((volts - BATTERY_EMPTY_VOLTS) / (BATTERY_FULL_VOLTS - BATTERY_EMPTY_VOLTS)) * 100.0)
        .clamp(0.0, 100.0) as u8
}

fn validate_layout(badge: &BadgeConfig, layout: &LayoutConfig) -> Result<(), &'static str> {
    battery_area(badge, layout)?;
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
    let ramp_bottom = i64::from(layout.info_y)
        + (badge.info.len() as i64 + 1) * i64::from(layout.row_spacing)
        + i64::from(layout.ramp_gap)
        + i64::from(layout.ramp_block_size.height);
    if layout.info_y < 0 || ramp_bottom > i64::from(PANEL_SIZE.height) {
        return Err("information rows and color ramp exceed the panel height");
    }
    Ok(())
}

fn draw_logo<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    badge: &BadgeConfig,
    layout: &LayoutConfig,
) -> Result<(), D::Error> {
    SMALL.draw(target, badge.logo, layout.logo_origin, layout.key_ink)
}

fn draw_qr<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    badge: &BadgeConfig,
    layout: &LayoutConfig,
) -> Result<(), RenderError<D::Error>> {
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
    let module_size = layout.qr_module_size as i32;
    let side = qr.size() * module_size;
    let origin = Point::new(layout.qr_x, layout.qr_bottom - side);
    if origin.x < 0
        || origin.y < 0
        || i64::from(origin.x) + i64::from(side) > i64::from(PANEL_SIZE.width)
        || layout.qr_bottom > PANEL_SIZE.height as i32
    {
        return Err(RenderError::InvalidLayout(
            "QR code exceeds the panel bounds",
        ));
    }
    for y in 0..qr.size() {
        for x in 0..qr.size() {
            if qr.get_module(x, y) {
                fill_rectangle(
                    target,
                    origin + Point::new(x * module_size, y * module_size),
                    Size::new(layout.qr_module_size, layout.qr_module_size),
                    layout.key_ink,
                )
                .map_err(RenderError::Target)?;
            }
        }
    }
    TEXT.draw(
        target,
        badge.qr_caption,
        origin + Point::new(side + layout.qr_caption_gap, side / 2 - TEXT.height / 2),
        layout.value_ink,
    )
    .map_err(RenderError::Target)
}

fn draw_header<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    badge: &BadgeConfig,
    layout: &LayoutConfig,
) -> Result<(), D::Error> {
    let title = format!("{}@{}", badge.user, badge.host);
    TITLE.draw(
        target,
        &title,
        Point::new(layout.column_x, layout.title_y),
        layout.key_ink,
    )?;
    fill_rectangle(
        target,
        Point::new(layout.column_x, layout.rule_y),
        Size::new(
            title.chars().count() as u32 * TITLE.width as u32,
            layout.rule_height,
        ),
        layout.key_ink,
    )
}

fn draw_info_rows<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    badge: &BadgeConfig,
    layout: &LayoutConfig,
) -> Result<i32, D::Error> {
    let mut y = layout.info_y;
    for &(key, value) in badge.info {
        draw_info_row(target, layout, key, value, y)?;
        y += layout.row_spacing;
    }
    Ok(y)
}

fn draw_info_row<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    layout: &LayoutConfig,
    key: &str,
    value: &str,
    y: i32,
) -> Result<(), D::Error> {
    TEXT.draw(
        target,
        &format!("{key}:"),
        Point::new(layout.column_x, y),
        layout.key_ink,
    )?;
    let value_x = layout.column_x + (key.chars().count() as i32 + 2) * TEXT.width;
    TEXT.draw(target, value, Point::new(value_x, y), layout.value_ink)
}

fn draw_color_ramp<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    layout: &LayoutConfig,
    next_y: i32,
) -> Result<(), D::Error> {
    for index in 0..8 {
        fill_rectangle(
            target,
            Point::new(
                layout.column_x + index * layout.ramp_step,
                next_y + layout.ramp_gap,
            ),
            layout.ramp_block_size,
            index as u8 * 2,
        )?;
    }
    Ok(())
}

fn fill_rectangle<D: DrawTarget<Color = Gray4>>(
    target: &mut D,
    origin: Point,
    size: Size,
    ink: u8,
) -> Result<(), D::Error> {
    Rectangle::new(origin, size)
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(ink)))
        .draw(target)
}
