//! Shared, compile-time configuration for the firmware and desktop preview.
use embedded_graphics::{geometry::Point, geometry::Size, primitives::Rectangle};

pub const PANEL_SIZE: Size = Size::new(960, 540);
/// Single-cell 3.7 V LiPo (here 2000 mAh) resting voltage to charge, ascending by voltage.
/// The percentage interpolates linearly between points and clamps outside the table.
/// Capacity does not change the curve; swap the table for a different chemistry.
pub const BATTERY_CURVE: &[(f32, u8)] = &[
    (3.27, 0),
    (3.61, 5),
    (3.69, 10),
    (3.73, 20),
    (3.77, 30),
    (3.80, 40),
    (3.84, 50),
    (3.87, 60),
    (3.95, 70),
    (4.02, 80),
    (4.11, 90),
    (4.20, 100),
];
const _: () = {
    assert!(BATTERY_CURVE.len() >= 2, "battery curve needs at least two points");
    let mut i = 1;
    while i < BATTERY_CURVE.len() {
        assert!(BATTERY_CURVE[i].0 > BATTERY_CURVE[i - 1].0, "battery curve volts must ascend");
        assert!(
            BATTERY_CURVE[i].1 >= BATTERY_CURVE[i - 1].1,
            "battery curve percent must not drop"
        );
        i += 1;
    }
};
/// Displayed voltage resolution. Readings round to this step, so ADC noise between wakes
/// does not redraw the battery row. Raise it if an idle badge still refreshes every wake.
pub const BATTERY_STEP_VOLTS: f32 = 0.05;
const _: () = assert!(BATTERY_STEP_VOLTS > 0.0, "battery step must be positive");

/// Firmware refresh interval in minutes. None disables the timer; Some must be nonzero.
#[cfg(target_arch = "xtensa")]
pub const REFRESH_INTERVAL_MINUTES: Option<u32> = Some(360);
#[cfg(target_arch = "xtensa")]
const _: () = assert!(
    !matches!(REFRESH_INTERVAL_MINUTES, Some(0)),
    "refresh interval must be greater than zero"
);

/// Full cleanup after this many successful partial updates. None disables it.
#[cfg(target_arch = "xtensa")]
pub const FULL_REFRESH_AFTER_PARTIALS: Option<u32> = Some(24);

/// Seconds touch stays on after an IO21 button wake; each tap restarts the window.
#[cfg(target_arch = "xtensa")]
pub const TOUCH_WINDOW_SECS: u64 = 60;

/// Enable IO21 button wake. RST remains available when both wake sources are disabled.
#[cfg(target_arch = "xtensa")]
pub const BUTTON_WAKE_ENABLED: bool = true;

#[derive(Clone, Copy)]
pub struct BadgeConfig<'a> {
    pub user: &'a str,
    pub host: &'a str,
    pub info: &'a [(&'a str, &'a str)],
    pub qr_url: &'a str,
    pub qr_caption: &'a str,
    /// Replaces `qr_caption` while touch is on.
    pub qr_touch_caption: &'a str,
    pub logo: &'a str,
    pub colours: &'a [u8],
}

// Keep "Key: value" under ~32 characters and at most 9 custom rows.
pub const BADGE: BadgeConfig<'static> = BadgeConfig {
    user: "famoto",
    host: "backpack",
    info: &[
        ("Pronouns", "they/them"),
        ("GitHub", "famoto"),
        ("Blog", "famoto.digital"),
        ("OS", "Human (x86_64)"),
        ("Host", "40C3"),
        ("Shell", "zsh"),
        ("CPU", "42threads, 0 scheduler"),
        ("Memory", "volatile, no ECC"),
        ("IRQs", "unmasked, all of them"),
    ],
    qr_url: "https://xn--bck1dqb1c.dev",
    qr_caption: "<- my blog",
    qr_touch_caption: "<- tap me",
    logo: LOGO,
    colours: &[0, 2, 4, 6, 8, 10, 12, 14],
};

/// Absolute view bounds and a local content anchor (the QR uses its bottom-left).
#[derive(Clone, Copy)]
pub struct ViewLayout {
    pub area: Rectangle,
    pub origin: Point,
}

#[derive(Clone, Copy)]
pub struct LayoutConfig {
    pub logo: ViewLayout,
    pub headline: ViewLayout,
    pub data: ViewLayout,
    pub battery: ViewLayout,
    pub bonus: ViewLayout,
    pub colour_scale: ViewLayout,
    /// Full-screen QR shown on tap; covers every other view.
    pub qr_full: ViewLayout,
    pub rule_y: i32,
    pub rule_height: u32,
    pub row_spacing: i32,
    pub qr_module_size: u32,
    pub qr_caption_gap: i32,
    pub qr_full_module_size: u32,
    pub colour_step: i32,
    pub colour_block_size: Size,
    pub key_ink: u8,
    pub value_ink: u8,
}

// Gray4 ink: 0 = black, 15 = white. Element coordinates are relative to their view.
pub const LAYOUT: LayoutConfig = LayoutConfig {
    logo: ViewLayout {
        area: Rectangle::new(Point::new(0, 0), Size::new(440, 396)),
        origin: Point::new(30, 0),
    },
    headline: ViewLayout {
        area: Rectangle::new(Point::new(440, 0), Size::new(520, 116)),
        origin: Point::new(0, 30),
    },
    data: ViewLayout {
        area: Rectangle::new(Point::new(440, 116), Size::new(520, 332)),
        origin: Point::new(0, 8),
    },
    battery: ViewLayout {
        area: Rectangle::new(Point::new(440, 448), Size::new(520, 36)),
        origin: Point::new(0, 0),
    },
    bonus: ViewLayout {
        area: Rectangle::new(Point::new(0, 396), Size::new(440, 144)),
        origin: Point::new(56, 134),
    },
    colour_scale: ViewLayout {
        area: Rectangle::new(Point::new(440, 484), Size::new(520, 56)),
        origin: Point::new(0, 14),
    },
    // Centres a 25-module (version 2) code at 16 px per module; longer URLs need a new origin.
    qr_full: ViewLayout {
        area: Rectangle::new(Point::new(0, 0), PANEL_SIZE),
        origin: Point::new(280, 470),
    },
    rule_y: 104,
    rule_height: 4,
    row_spacing: 36,
    qr_module_size: 5,
    qr_caption_gap: 16,
    qr_full_module_size: 16,
    colour_step: 52,
    colour_block_size: Size::new(48, 32),
    key_ink: 0,
    value_ink: 4,
};

// Generated by tools/moon.py (38 x 18 cells of the small font).
const LOGO: &str = r"
            :;+*aa&&%%eexc~-
                 ,~=*xoea&%8ac,
  +                  ,;cxoea&%8e=
         |             -;c*oea&%8&:
        -+-              ,=cxoea&8#+
         |                -=c*oea&%8c
                           -+cxoea%8#~
                            ;+*xea&%8e
                +           :+*xoe&%8#
                            :+*xoe&%8#
     +                      ;+*xea&%8e
                           -+cxoea%8#~
             |            -=c*oea&%8c
            -+-          ,=cxoea&8#+
             |         -;c*oea&%8&:
                     ,;cxoea&%8e=
                 ,~=*xoea&%8ac,
            :;+*aa&&%%eexc~-";
