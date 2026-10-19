use embedded_graphics::{pixelcolor::Gray4, prelude::*, primitives::Rectangle};
use nametag_preview::{
    badge, config,
    ui::{
        refresh::{self, RefreshKind},
        Element, Font, Pipeline, RefreshRequest, View, ViewSet, MAX_VIEWS,
    },
    Framebuffer,
};

mod support;
use support::TestBoard;

fn render(pipeline: &Pipeline<'_>) -> Framebuffer {
    let mut target = Framebuffer::default();
    pipeline.draw(&mut target, ViewSet::MAX).unwrap();
    target
}

fn assert_partial(old: &Pipeline<'_>, new: &Pipeline<'_>, expected: &[usize]) {
    let saved = refresh::plan(refresh::EMPTY, false, &old.state(), None).unwrap().next_saved;
    let update = refresh::plan(saved, true, &new.state(), None).unwrap();
    assert!(matches!(update.kind, RefreshKind::Partial(_)));
    assert_eq!(update.kind.views(), expected.iter().fold(0, |mask, id| mask | (1 << id)));
    let mut display = TestBoard::new(saved, 3.98);
    display.powered = true;
    display.pixels = render(old);
    let before = display.pixels.pixels().to_vec();
    let areas: Vec<_> = update.clear_areas().collect();
    new.refresh(&mut display, RefreshRequest::Changed, true, None).unwrap();
    assert_eq!(display.saved, update.next_saved);
    display.assert_full_render(new);
    let target = display.pixels;
    for point in Rectangle::new(Point::zero(), config::PANEL_SIZE).points() {
        if !areas.iter().any(|area| area.contains(point)) {
            let index = point.y as usize * config::PANEL_SIZE.width as usize + point.x as usize;
            assert_eq!(
                target.pixels()[index],
                before[index],
                "unselected pixel changed at {point:?}"
            );
        }
    }
    assert!(refresh::plan(update.next_saved, true, &new.state(), None).is_none());
}

fn solid(x: i32, y: i32, width: u32, height: u32, ink: u8) -> View<'static> {
    let size = Size::new(width, height);
    View::new(
        Rectangle::new(Point::new(x, y), size),
        vec![Element::Fill { area: Rectangle::new(Point::zero(), size), ink }],
    )
}

#[test]
fn default_render_matches_existing_preview_pixels() {
    let mut reader = png::Decoder::new(std::io::Cursor::new(include_bytes!("../preview.png")))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!(info.color_type, png::ColorType::Grayscale);
    assert_eq!((info.width, info.height), (config::PANEL_SIZE.width, config::PANEL_SIZE.height));
    assert_eq!(
        render(&badge::frame(&config::BADGE, &config::LAYOUT, 3.98).unwrap()).pixels(),
        &pixels[..info.buffer_size()]
    );
}

#[test]
fn every_badge_view_changes_shrinks_and_grows_independently() {
    let original = badge::frame(&config::BADGE, &config::LAYOUT, 3.98).unwrap();
    for id in 0..6 {
        let mut badge = config::BADGE;
        let user = String::from("moon");
        let status = String::from("available");
        let rows = [("Status", status.as_str())];
        let mut volts = 3.98;
        match id {
            badge::LOGO => badge.logo = "+ moon +",
            badge::HEADLINE => badge.user = &user,
            badge::DATA => badge.info = &rows,
            badge::BATTERY => volts = 4.2,
            badge::BONUS => {
                badge.qr_url = "a";
                badge.qr_caption = "QR";
            }
            badge::COLOUR_SCALE => badge.colours = &[0, 4, 8, 12],
            _ => unreachable!(),
        }
        let changed = badge::frame(&badge, &config::LAYOUT, volts).unwrap();
        assert_partial(&original, &changed, &[id]);
        assert_partial(&changed, &original, &[id]);
    }
}

#[test]
fn several_changed_views_match_one_full_render() {
    let original = badge::frame(&config::BADGE, &config::LAYOUT, 3.98).unwrap();
    let badge = config::BadgeConfig {
        logo: "moon",
        user: "hi",
        info: &[("Status", "ready")],
        qr_caption: "blog",
        colours: &[0, 7, 14],
        ..config::BADGE
    };
    let changed = badge::frame(&badge, &config::LAYOUT, 3.3).unwrap();
    assert_partial(&original, &changed, &[0, 1, 2, 3, 4, 5]);
}

#[test]
fn empty_content_erases_only_its_views() {
    let original = badge::frame(&config::BADGE, &config::LAYOUT, 3.98).unwrap();
    let badge = config::BadgeConfig { logo: "", info: &[], colours: &[], ..config::BADGE };
    let changed = badge::frame(&badge, &config::LAYOUT, 3.98).unwrap();
    assert_partial(&original, &changed, &[badge::LOGO, badge::DATA, badge::COLOUR_SCALE]);
    assert_eq!(
        original.get(badge::BATTERY).unwrap().area,
        changed.get(badge::BATTERY).unwrap().area
    );
}

#[test]
fn battery_updates_erase_shorter_and_longer_values() {
    for (old, new) in [(4.2, 3.3), (3.3, 4.2), (3.98, 3.749)] {
        assert_partial(
            &badge::frame(&config::BADGE, &config::LAYOUT, old).unwrap(),
            &badge::frame(&config::BADGE, &config::LAYOUT, new).unwrap(),
            &[badge::BATTERY],
        );
    }
}

#[test]
fn register_replace_deregister_and_reuse_any_slot() {
    let empty = Pipeline::new();
    let mut current = empty.clone();
    current.register(9, solid(10, 10, 80, 40, 0)).unwrap();
    assert_partial(&empty, &current, &[9]);
    let before = current.clone();
    assert!(current.register(9, solid(10, 10, 80, 40, 4)).unwrap().is_some());
    assert_partial(&before, &current, &[9]);
    let before = current.clone();
    assert!(current.deregister(9).unwrap().is_some());
    assert_partial(&before, &current, &[]);
    assert!(current.deregister(9).unwrap().is_none());
    current.register(9, solid(200, 100, 20, 30, 8)).unwrap();
    assert_partial(&empty, &current, &[9]);
    let before = current.state();
    assert!(current.register(MAX_VIEWS, solid(0, 0, 1, 1, 0)).is_err());
    assert!(current.deregister(MAX_VIEWS).is_err());
    assert_eq!(before, current.state());
}

#[test]
fn content_and_position_can_change_in_place() {
    let mut original = Pipeline::new();
    original
        .register(
            8,
            View::new(
                Rectangle::new(Point::new(10, 10), Size::new(220, 40)),
                vec![Element::line(Point::zero(), String::from("ready"), Font::Text, 0)],
            ),
        )
        .unwrap();
    let mut changed = original.clone();
    changed.get_mut(8).unwrap().content_mut()[0] =
        Element::line(Point::zero(), format!("{} tasks", 3), Font::Text, 0);
    assert_partial(&original, &changed, &[8]);
    let moved_from = changed.clone();
    changed.get_mut(8).unwrap().area.top_left = Point::new(300, 120);
    assert_partial(&moved_from, &changed, &[8]);
    let old = changed.clone();
    changed.get_mut(8).unwrap().area.size.width = 120;
    assert_partial(&old, &changed, &[8]);
}

#[test]
fn overlapping_views_are_redrawn_transitively_in_id_order() {
    let mut old = Pipeline::new();
    for (id, x, ink) in [(0, 10, 0), (1, 60, 4), (2, 110, 8), (3, 250, 12)] {
        old.register(id, solid(x, 10, 80, 40, ink)).unwrap();
    }
    let mut changed = old.clone();
    changed.register(0, solid(10, 10, 80, 40, 2)).unwrap();
    assert_partial(&old, &changed, &[0, 1, 2]);
    let mut removed = old.clone();
    removed.deregister(1).unwrap();
    assert_partial(&old, &removed, &[0, 2]);
    let mut moved = old.clone();
    moved.get_mut(0).unwrap().area.top_left.x = 180;
    assert_partial(&old, &moved, &[0, 1, 2, 3]);
}

#[test]
fn each_badge_view_writes_only_inside_its_bounds() {
    struct Target {
        area: Rectangle,
        count: usize,
    }
    impl OriginDimensions for Target {
        fn size(&self) -> Size {
            config::PANEL_SIZE
        }
    }
    impl DrawTarget for Target {
        type Color = Gray4;
        type Error = core::convert::Infallible;
        fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
            &mut self,
            pixels: I,
        ) -> Result<(), Self::Error> {
            for Pixel(point, _) in pixels {
                assert!(self.area.contains(point));
                self.count += 1;
            }
            Ok(())
        }
    }
    let pipeline = badge::frame(&config::BADGE, &config::LAYOUT, 3.98).unwrap();
    for id in 0..6 {
        let mut target = Target { area: pipeline.get(id).unwrap().area, count: 0 };
        pipeline.draw(&mut target, 1 << id).unwrap();
        assert!(target.count > 0);
    }
}

#[test]
fn frame_factories_borrow_content_without_retaining_the_config_wrapper() {
    fn frame_from_rows<'a>(rows: &'a [(&'a str, &'a str)]) -> Pipeline<'a> {
        let badge = config::BadgeConfig { info: rows, ..config::BADGE };
        badge::frame(&badge, &config::LAYOUT, 3.98).unwrap()
    }

    let status = String::from("ready");
    let rows = [("Status", status.as_str())];
    let pipeline = frame_from_rows(&rows);
    assert!(matches!(
        &pipeline.get(badge::DATA).unwrap().content()[1],
        Element::Text { text, .. } if text == "ready"
    ));
    render(&pipeline);
}

#[test]
fn badge_frames_reject_overlapping_information_rows() {
    for spacing in [-36, 0, 16, 31] {
        let layout = config::LayoutConfig { row_spacing: spacing, ..config::LAYOUT };
        assert_eq!(
            badge::frame(&config::BADGE, &layout, 3.98).err(),
            Some("information rows overlap")
        );
    }
    let layout = config::LayoutConfig { row_spacing: 32, ..config::LAYOUT };
    render(&badge::frame(&config::BADGE, &layout, 3.98).unwrap());
}

#[test]
fn validation_happens_before_any_pixels_are_written() {
    let long = "x".repeat(40);
    let too_many = [("Key", "value"); 10];
    let long_rows = [("Key", long.as_str())];
    let multiline = [("Key", "first\nsecond")];
    let mut badges = [config::BADGE; 5];
    badges[0].user = &long;
    badges[1].info = &too_many;
    badges[2].info = &long_rows;
    badges[3].info = &multiline;
    badges[4].qr_caption = &long;
    for badge in badges {
        let pipeline = badge::frame(&badge, &config::LAYOUT, 3.98).unwrap();
        let mut target = Framebuffer::default();
        assert!(pipeline.draw(&mut target, ViewSet::MAX).is_err());
        assert!(target.pixels().iter().all(|&pixel| pixel == 255));
    }
    let mut layout = config::LAYOUT;
    layout.qr_module_size = 0;
    let pipeline = badge::frame(&config::BADGE, &layout, 3.98).unwrap();
    let mut target = Framebuffer::default();
    pipeline.draw(&mut target, 1 << badge::LOGO).unwrap();
    let before = target.pixels().to_vec();
    assert!(pipeline.draw(&mut target, ViewSet::MAX).is_err());
    assert_eq!(before, target.pixels());
}

#[test]
fn invalid_view_bounds_or_local_positions_are_rejected() {
    for view in [
        solid(-1, 0, 20, 20, 0),
        solid(950, 0, 20, 20, 0),
        solid(0, 0, 0, 20, 0),
        solid(i32::MAX, 0, 20, 20, 0),
        View::new(
            Rectangle::new(Point::zero(), Size::new(20, 20)),
            vec![Element::Fill {
                area: Rectangle::new(Point::new(i32::MAX, 0), Size::new(u32::MAX, 1)),
                ink: 0,
            }],
        ),
    ] {
        let mut pipeline = Pipeline::new();
        pipeline.register(0, view).unwrap();
        let mut target = Framebuffer::default();
        assert!(pipeline.draw(&mut target, ViewSet::MAX).is_err());
        assert!(target.pixels().iter().all(|&pixel| pixel == 255));
    }
}

#[test]
fn validation_uses_the_actual_draw_target_bounds() {
    let mut pipeline = Pipeline::new();
    pipeline.register(0, solid(60, 0, 20, 20, 0)).unwrap();
    let mut target = embedded_graphics::mock_display::MockDisplay::<Gray4>::new();
    assert!(pipeline.draw(&mut target, ViewSet::MAX).is_err());
    assert_eq!(target.affected_area().size, Size::zero());
    pipeline.register(0, solid(10, 10, 20, 20, 0)).unwrap();
    pipeline.draw(&mut target, ViewSet::MAX).unwrap();
    assert_eq!(target.get_pixel(Point::new(10, 10)), Some(Gray4::BLACK));
}

#[test]
fn empty_text_leaves_pixels_untouched_even_with_unused_extreme_positions() {
    let mut pipeline = Pipeline::new();
    pipeline
        .register(
            0,
            View::new(
                Rectangle::new(Point::new(20, 20), Size::new(20, 20)),
                vec![Element::text(Point::new(i32::MAX, i32::MAX), "", Font::Text, 0)],
            ),
        )
        .unwrap();
    let target = render(&pipeline);
    assert!(target.pixels().iter().all(|&pixel| pixel == 255));
}

#[test]
fn font_fallback_compares_as_displayed_but_qr_keeps_utf8() {
    let first = config::BadgeConfig { info: &[("Status", "caf\u{00e9}")], ..config::BADGE };
    let second = config::BadgeConfig { info: &[("Status", "caf?")], ..config::BADGE };
    let first = badge::frame(&first, &config::LAYOUT, 3.98).unwrap();
    let second = badge::frame(&second, &config::LAYOUT, 3.98).unwrap();
    assert_eq!(first.state(), second.state());
    assert_eq!(render(&first).pixels(), render(&second).pixels());
    let first = config::BadgeConfig { qr_url: "caf\u{00e9}", ..config::BADGE };
    let second = config::BadgeConfig { qr_url: "caf?", ..config::BADGE };
    assert_partial(
        &badge::frame(&first, &config::LAYOUT, 3.98).unwrap(),
        &badge::frame(&second, &config::LAYOUT, 3.98).unwrap(),
        &[badge::BONUS],
    );
}
