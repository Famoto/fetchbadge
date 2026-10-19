use super::{
    refresh::{Fingerprint, ViewState},
    Font,
};
use alloc::{borrow::Cow, vec::Vec};
use core::{
    fmt::{self, Write},
    hash::{Hash, Hasher},
};
use embedded_graphics::{prelude::*, primitives::Rectangle};

pub const MAX_VIEWS: usize = 16;
pub type ViewId = usize;
/// One bit per registered ID; all 16 slots fit in a native mask.
pub type ViewSet = u16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextStyle {
    pub font: Font,
    pub ink: u8,
    pub single_line: bool,
}

/// All positions are relative to the containing view. Owned strings also work across updates.
#[derive(Clone, Debug)]
pub enum Element<'a> {
    Text {
        position: Point,
        text: Cow<'a, str>,
        style: TextStyle,
    },
    Fill {
        area: Rectangle,
        ink: u8,
    },
    Qr {
        bottom_left: Point,
        module_size: u32,
        url: Cow<'a, str>,
        caption: Cow<'a, str>,
        caption_gap: i32,
        ink: u8,
        caption_ink: u8,
    },
}

impl<'a> Element<'a> {
    pub fn text(position: Point, text: impl Into<Cow<'a, str>>, font: Font, ink: u8) -> Self {
        Self::Text {
            position,
            text: text.into(),
            style: TextStyle { font, ink, single_line: false },
        }
    }
    pub fn line(position: Point, text: impl Into<Cow<'a, str>>, font: Font, ink: u8) -> Self {
        Self::Text {
            position,
            text: text.into(),
            style: TextStyle { font, ink, single_line: true },
        }
    }
}

/// A draw list placed at panel bounds. Edit `area` or `content` directly; refresh notices.
#[derive(Clone, Debug)]
pub struct View<'a> {
    pub area: Rectangle,
    pub content: Vec<Element<'a>>,
}

impl<'a> View<'a> {
    pub fn new(area: Rectangle, content: Vec<Element<'a>>) -> Self {
        Self { area, content }
    }

    pub(super) fn state(&self) -> ViewState {
        let mut signature = Fingerprint::default();
        self.content.hash(&mut signature);
        ViewState { active: 1, area: self.area, content: signature.finish() }
    }
}

pub(super) fn selected(mut views: ViewSet) -> impl Iterator<Item = ViewId> {
    core::iter::from_fn(move || {
        if views == 0 {
            return None;
        }
        let id = views.trailing_zeros() as ViewId;
        views &= views - 1;
        Some(id)
    })
}

// ponytail: i64 math because view bounds may hold extreme unvalidated values.
pub(super) fn covers(outer: Rectangle, inner: Rectangle) -> bool {
    let (x, y) = (i64::from(inner.top_left.x), i64::from(inner.top_left.y));
    let (left, top) = (i64::from(outer.top_left.x), i64::from(outer.top_left.y));
    x >= left
        && y >= top
        && x + i64::from(inner.size.width) <= left + i64::from(outer.size.width)
        && y + i64::from(inner.size.height) <= top + i64::from(outer.size.height)
}

pub(super) fn overlaps(first: Rectangle, second: Rectangle) -> bool {
    let (x, y) = (i64::from(first.top_left.x), i64::from(first.top_left.y));
    let (left, top) = (i64::from(second.top_left.x), i64::from(second.top_left.y));
    !first.is_zero_sized()
        && !second.is_zero_sized()
        && x < left + i64::from(second.size.width)
        && left < x + i64::from(first.size.width)
        && y < top + i64::from(second.size.height)
        && top < y + i64::from(first.size.height)
}

/// Hash text as drawn, so unsupported characters compare equal to their '?' fallback.
struct DisplayText<'a, H>(&'a mut H);
impl<H: Hasher> Write for DisplayText<'_, H> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for character in text.chars() {
            self.0.write_u8(super::font::displayed(character) as u8);
        }
        Ok(())
    }
}

impl Hash for Element<'_> {
    fn hash<H: Hasher>(&self, hash: &mut H) {
        match self {
            Self::Text { position, text, style } => {
                0u8.hash(hash);
                position.hash(hash);
                DisplayText(hash).write_str(text).expect("signature formatting");
                hash.write_u8(0);
                style.hash(hash);
            }
            Self::Fill { area, ink } => {
                1u8.hash(hash);
                (area, ink).hash(hash);
            }
            Self::Qr { bottom_left, module_size, url, caption, caption_gap, ink, caption_ink } => {
                2u8.hash(hash);
                (bottom_left, module_size, url).hash(hash);
                DisplayText(hash).write_str(caption).expect("signature formatting");
                hash.write_u8(0);
                (caption_gap, ink, caption_ink).hash(hash);
            }
        }
    }
}
