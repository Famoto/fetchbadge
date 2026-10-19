//! The neofetch scene is six independent views composed from ordinary UI elements.
use crate::{
    config::{BadgeConfig, LayoutConfig, BATTERY_EMPTY_VOLTS, BATTERY_FULL_VOLTS},
    ui::{Element, ElementSink, Font, Pipeline, View, ViewId},
};
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
    let (badge, layout) = (*badge, *layout);
    View::lazy(layout.logo.area, move |sink| {
        sink.push(Element::text(layout.logo.origin, badge.logo, Font::Small, layout.key_ink));
    })
}

fn headline<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let (badge, layout) = (*badge, *layout);
    View::lazy(layout.headline.area, move |sink| {
        let heading = layout.headline;
        let columns =
            badge.user.chars().count().saturating_add(1).saturating_add(badge.host.chars().count());
        let width = (columns as u32).saturating_mul(Font::Title.size().width);
        sink.line(
            heading.origin,
            format_args!("{}@{}", badge.user, badge.host),
            Font::Title,
            layout.key_ink,
        );
        sink.push(Element::Fill {
            area: Rectangle::new(
                Point::new(heading.origin.x, layout.rule_y),
                Size::new(width, layout.rule_height),
            ),
            ink: layout.key_ink,
        });
    })
}

fn data<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let (badge, layout) = (*badge, *layout);
    View::lazy(layout.data.area, move |sink| {
        let origin = layout.data.origin;
        for (index, &(key, value)) in badge.info.iter().enumerate() {
            let y = origin.y.saturating_add((index as i32).saturating_mul(layout.row_spacing));
            let position = row_key(sink, key, Point::new(origin.x, y), &layout);
            sink.push(Element::line(position, value, Font::Text, layout.value_ink));
        }
    })
}

fn battery<'a>(layout: &LayoutConfig, volts: f32) -> View<'a> {
    let layout = *layout;
    let percent = if volts >= BATTERY_FULL_VOLTS {
        100
    } else {
        (((volts - BATTERY_EMPTY_VOLTS) / (BATTERY_FULL_VOLTS - BATTERY_EMPTY_VOLTS)) * 100.0)
            .clamp(0.0, 100.0) as u8
    };
    View::lazy(layout.battery.area, move |sink| {
        let position = row_key(sink, "Battery", layout.battery.origin, &layout);
        sink.line(position, format_args!("{percent}% ({volts:.2}V)"), Font::Text, layout.value_ink);
    })
}

fn bonus<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let (badge, layout) = (*badge, *layout);
    View::lazy(layout.bonus.area, move |sink| {
        let bonus = layout.bonus;
        sink.push(Element::Qr {
            bottom_left: bonus.origin,
            module_size: layout.qr_module_size,
            url: badge.qr_url.into(),
            caption: badge.qr_caption.into(),
            caption_gap: layout.qr_caption_gap,
            ink: layout.key_ink,
            caption_ink: layout.value_ink,
        });
    })
}

fn colour_scale<'a>(badge: &BadgeConfig<'a>, layout: &LayoutConfig) -> View<'a> {
    let (badge, layout) = (*badge, *layout);
    View::lazy(layout.colour_scale.area, move |sink| {
        let scale = layout.colour_scale;
        for (index, &ink) in badge.colours.iter().enumerate() {
            sink.push(Element::Fill {
                area: Rectangle::new(
                    Point::new(
                        scale
                            .origin
                            .x
                            .saturating_add((index as i32).saturating_mul(layout.colour_step)),
                        scale.origin.y,
                    ),
                    layout.colour_block_size,
                ),
                ink,
            });
        }
    })
}

fn row_key(
    sink: &mut dyn ElementSink<'_>,
    key: &str,
    origin: Point,
    layout: &LayoutConfig,
) -> Point {
    let advance = (key.chars().count() as i32)
        .saturating_add(2)
        .saturating_mul(Font::Text.size().width as i32);
    sink.line(origin, format_args!("{key}:"), Font::Text, layout.key_ink);
    Point::new(origin.x.saturating_add(advance), origin.y)
}
