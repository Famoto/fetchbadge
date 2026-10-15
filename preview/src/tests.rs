use super::*;
use embedded_graphics::primitives::Rectangle;

#[test]
fn battery_only_update_matches_full_render_and_preserves_other_pixels() {
    // Exercise both shrinking and growing battery text, including 100% -> 0%.
    for (old_volts, new_volts) in [(4.2, 3.3), (3.3, 4.2), (3.98, 3.749)] {
        let mut partial = Framebuffer::new();
        badge::draw(&mut partial, &config::BADGE, &config::LAYOUT, old_volts).unwrap();
        let before = partial.pixels.clone();
        let area = badge::battery_area(&config::BADGE, &config::LAYOUT).unwrap();
        partial.fill_solid(&area, Gray4::WHITE).unwrap();
        badge::draw_battery(&mut partial, &config::BADGE, &config::LAYOUT, new_volts).unwrap();

        let mut expected = Framebuffer::new();
        badge::draw(&mut expected, &config::BADGE, &config::LAYOUT, new_volts).unwrap();
        assert_eq!(partial.pixels, expected.pixels);
        for point in Rectangle::new(Point::zero(), config::PANEL_SIZE).points() {
            if !area.contains(point) {
                let index = point.y as usize * config::PANEL_SIZE.width as usize + point.x as usize;
                assert_eq!(partial.pixels[index], before[index]);
            }
        }
    }
}

#[test]
fn battery_draw_never_writes_outside_its_clear_rectangle() {
    struct BatteryTarget {
        area: Rectangle,
        pixels_written: usize,
    }
    impl OriginDimensions for BatteryTarget {
        fn size(&self) -> Size {
            config::PANEL_SIZE
        }
    }
    impl DrawTarget for BatteryTarget {
        type Color = Gray4;
        type Error = core::convert::Infallible;
        fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
            &mut self,
            pixels: I,
        ) -> Result<(), Self::Error> {
            for Pixel(point, _) in pixels {
                assert!(
                    self.area.contains(point),
                    "battery pixel outside clear area: {point:?}"
                );
                self.pixels_written += 1;
            }
            Ok(())
        }
    }
    let mut target = BatteryTarget {
        area: badge::battery_area(&config::BADGE, &config::LAYOUT).unwrap(),
        pixels_written: 0,
    };
    badge::draw_battery(&mut target, &config::BADGE, &config::LAYOUT, 4.2).unwrap();
    assert!(target.pixels_written > 0);
}

#[test]
fn invalid_battery_rectangle_is_rejected_before_drawing() {
    let mut layout = config::LAYOUT;
    layout.column_x = -1;
    assert!(badge::battery_area(&config::BADGE, &layout).is_err());
    layout.column_x = config::PANEL_SIZE.width as i32;
    assert!(badge::battery_area(&config::BADGE, &layout).is_err());
    layout.column_x = config::LAYOUT.column_x;
    layout.info_y = 530;
    let mut target = Framebuffer::new();
    assert!(badge::draw_battery(&mut target, &config::BADGE, &layout, 3.98).is_err());
    assert!(target.pixels.iter().all(|&pixel| pixel == 255));
}
