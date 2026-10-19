//! The neofetch scene is six independent views composed from ordinary UI elements.
use crate::{
    config::{BadgeConfig, LayoutConfig, BATTERY_EMPTY_VOLTS, BATTERY_FULL_VOLTS},
    ui::{Element, Font, Pipeline, View, ViewId},
};
use alloc::{format, vec, vec::Vec};
use embedded_graphics::{prelude::*, primitives::Rectangle};

pub const LOGO: ViewId = 0;
pub const HEADLINE: ViewId = 1;
pub const DATA: ViewId = 2;
pub const BATTERY: ViewId = 3;
pub const BONUS: ViewId = 4;
pub const COLOUR_SCALE: ViewId = 5;

/// Borrow the badge's content, independently of the configuration wrapper's lifetime.
pub fn frame<'a>(
    badge: &BadgeConfig<'a>,
    layout: &LayoutConfig,
    volts: f32,
) -> Result<Pipeline<'a>, &'static str> {
    if layout.row_spacing < Font::Text.size().height as i32 {
        return Err("information rows overlap");
    }
    let mut pipeline = Pipeline::new();
    for (id, view) in [
        (LOGO, logo(badge, layout)),
        (HEADLINE, headline(badge, layout)),
        (DATA, data(badge, layout)),
        (BATTERY, battery(layout, volts)),
        (BONUS, bonus(badge, layout)),
        (COLOUR_SCALE, colour_scale(badge, layout)),
    ] {
        pipeline.register(id, view).expect("default badge view ID");
    }
    Ok(pipeline)
}

fn logo<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let logo = Element::text(layout.logo.origin, badge.logo, Font::Small, layout.key_ink);
    View::new(layout.logo.area, vec![logo])
}

fn headline<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let heading = layout.headline;
    let columns =
        badge.user.chars().count().saturating_add(1).saturating_add(badge.host.chars().count());
    let width = (columns as u32).saturating_mul(Font::Title.size().width);
    let title = format!("{}@{}", badge.user, badge.host);
    let rule = Rectangle::new(
        Point::new(heading.origin.x, layout.rule_y),
        Size::new(width, layout.rule_height),
    );
    View::new(
        heading.area,
        vec![
            Element::line(heading.origin, title, Font::Title, layout.key_ink),
            Element::Fill { area: rule, ink: layout.key_ink },
        ],
    )
}

fn data<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let origin = layout.data.origin;
    let mut content = Vec::new();
    for (index, &(key, value)) in badge.info.iter().enumerate() {
        let y = origin.y.saturating_add((index as i32).saturating_mul(layout.row_spacing));
        let position = row_key(&mut content, key, Point::new(origin.x, y), layout);
        content.push(Element::line(position, value, Font::Text, layout.value_ink));
    }
    View::new(layout.data.area, content)
}

fn battery<'a>(layout: &LayoutConfig, volts: f32) -> View<'a> {
    let percent = ((volts - BATTERY_EMPTY_VOLTS) / (BATTERY_FULL_VOLTS - BATTERY_EMPTY_VOLTS)
        * 100.0)
        .clamp(0.0, 100.0) as u8;
    let mut content = Vec::new();
    let position = row_key(&mut content, "Battery", layout.battery.origin, layout);
    let reading = format!("{percent}% ({volts:.2}V)");
    content.push(Element::line(position, reading, Font::Text, layout.value_ink));
    View::new(layout.battery.area, content)
}

fn bonus<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let qr = Element::Qr {
        bottom_left: layout.bonus.origin,
        module_size: layout.qr_module_size,
        url: badge.qr_url.into(),
        caption: badge.qr_caption.into(),
        caption_gap: layout.qr_caption_gap,
        ink: layout.key_ink,
        caption_ink: layout.value_ink,
    };
    View::new(layout.bonus.area, vec![qr])
}

fn colour_scale<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let origin = layout.colour_scale.origin;
    let blocks = badge.colours.iter().enumerate().map(|(index, &ink)| {
        let x = origin.x.saturating_add((index as i32).saturating_mul(layout.colour_step));
        Element::Fill {
            area: Rectangle::new(Point::new(x, origin.y), layout.colour_block_size),
            ink,
        }
    });
    View::new(layout.colour_scale.area, blocks.collect())
}

/// Push `Key:` and return where its value starts.
fn row_key(
    content: &mut Vec<Element<'_>>,
    key: &str,
    origin: Point,
    layout: &LayoutConfig,
) -> Point {
    let advance = (key.chars().count() as i32)
        .saturating_add(2)
        .saturating_mul(Font::Text.size().width as i32);
    content.push(Element::line(origin, format!("{key}:"), Font::Text, layout.key_ink));
    Point::new(origin.x.saturating_add(advance), origin.y)
}
