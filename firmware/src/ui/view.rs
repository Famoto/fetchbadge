use super::{
    refresh::{Fingerprint, ViewState},
    Font,
};
use alloc::{borrow::Cow, fmt::format, rc::Rc, vec::Vec};
use core::{
    cell::OnceCell,
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

/// A deterministic recipe is visited for signatures without constructing a draw list.
/// Use `line` for formatting without allocating a temporary string during signature checks.
pub trait ElementSink<'a> {
    fn push(&mut self, element: Element<'a>);
    fn formatted(&mut self, position: Point, text: fmt::Arguments<'_>, style: TextStyle);

    fn line(&mut self, position: Point, text: fmt::Arguments<'_>, font: Font, ink: u8) {
        self.formatted(position, text, TextStyle { font, ink, single_line: true });
    }
}

type Recipe<'a> = dyn Fn(&mut dyn ElementSink<'a>) + 'a;

/// A view owns an eager draw list or a deterministic, lazily materialized recipe.
#[derive(Clone)]
pub struct View<'a> {
    pub area: Rectangle,
    content: OnceCell<Vec<Element<'a>>>,
    recipe: Option<(u64, Rc<Recipe<'a>>)>,
}

impl fmt::Debug for View<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("View")
            .field("area", &self.area)
            .field("content", &self.content.get())
            .finish_non_exhaustive()
    }
}

impl<'a> View<'a> {
    pub fn new(area: Rectangle, content: Vec<Element<'a>>) -> Self {
        Self { area, content: OnceCell::from(content), recipe: None }
    }

    /// The recipe must emit the same elements on every visit. Replace/edit the view to change it.
    /// The same recipe supplies both the visual signature and the eventual drawing elements.
    pub fn lazy(area: Rectangle, recipe: impl Fn(&mut dyn ElementSink<'a>) + 'a) -> Self {
        let mut signature = SignatureSink(Fingerprint::default());
        recipe(&mut signature);
        Self {
            area,
            content: OnceCell::new(),
            recipe: Some((signature.0.finish(), Rc::new(recipe))),
        }
    }

    pub fn content(&self) -> &[Element<'a>] {
        self.content.get_or_init(|| {
            let mut sink = ContentSink(Vec::new());
            self.recipe.as_ref().expect("unbuilt view has a recipe").1(&mut sink);
            sink.0
        })
    }

    pub fn content_mut(&mut self) -> &mut Vec<Element<'a>> {
        self.content();
        self.recipe = None;
        self.content.get_mut().expect("materialized content")
    }

    pub fn set_content(&mut self, content: Vec<Element<'a>>) {
        self.recipe = None;
        self.content = OnceCell::from(content);
    }

    /// Useful for inspecting whether a refresh needed this view's draw list.
    pub fn is_materialized(&self) -> bool {
        self.content.get().is_some()
    }

    pub(super) fn state(&self) -> ViewState {
        let content = if let Some((signature, _)) = &self.recipe {
            *signature
        } else {
            let mut signature = Fingerprint::default();
            for element in self.content() {
                element.hash(&mut signature);
            }
            signature.finish()
        };
        ViewState { active: 1, area: self.area, content }
    }
}

struct ContentSink<'a>(Vec<Element<'a>>);
impl<'a> ElementSink<'a> for ContentSink<'a> {
    fn push(&mut self, element: Element<'a>) {
        self.0.push(element);
    }
    fn formatted(&mut self, position: Point, text: fmt::Arguments<'_>, style: TextStyle) {
        self.push(Element::Text { position, text: format(text).into(), style });
    }
}

struct SignatureSink(Fingerprint);
impl<'a> ElementSink<'a> for SignatureSink {
    fn push(&mut self, element: Element<'a>) {
        element.hash(&mut self.0);
    }
    fn formatted(&mut self, position: Point, text: fmt::Arguments<'_>, style: TextStyle) {
        hash_text(position, text, style, &mut self.0);
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

pub(super) fn covers(outer: Rectangle, inner: Rectangle) -> bool {
    let (x, y) = (i64::from(inner.top_left.x), i64::from(inner.top_left.y));
    let (left, top) = (i64::from(outer.top_left.x), i64::from(outer.top_left.y));
    x >= left
        && y >= top
        && x + i64::from(inner.size.width) <= left + i64::from(outer.size.width)
        && y + i64::from(inner.size.height) <= top + i64::from(outer.size.height)
}

struct DisplayText<'a, H>(&'a mut H);
impl<H: Hasher> Write for DisplayText<'_, H> {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for character in text.chars() {
            self.0.write_u8(super::font::displayed(character) as u8);
        }
        Ok(())
    }
}

fn hash_text(position: Point, text: fmt::Arguments<'_>, style: TextStyle, hash: &mut impl Hasher) {
    0u8.hash(hash);
    position.hash(hash);
    DisplayText(hash).write_fmt(text).expect("signature formatting");
    hash.write_u8(0);
    style.hash(hash);
}

impl Hash for Element<'_> {
    fn hash<H: Hasher>(&self, hash: &mut H) {
        match self {
            Self::Text { position, text, style } => {
                hash_text(*position, format_args!("{text}"), *style, hash);
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
