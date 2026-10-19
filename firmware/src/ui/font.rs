//! Shared Fira Code glyphs: 4-bit coverage, high nibble first, 15 = full ink.
use embedded_graphics::{pixelcolor::Gray4, prelude::*};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Font {
    Small,
    Text,
    Title,
}

/// Shared by glyph lookup and refresh signatures so fallback text compares identically.
pub(super) fn displayed(character: char) -> char {
    if character.is_ascii_graphic() || character == ' ' || character == '\n' {
        character
    } else {
        '?'
    }
}

impl Font {
    fn data(self) -> (i32, i32, &'static [u8]) {
        match self {
            Self::Small => (10, 20, include_bytes!("../fonts/fira_10x20.bin")),
            Self::Text => (16, 32, include_bytes!("../fonts/fira_16x32.bin")),
            Self::Title => (32, 64, include_bytes!("../fonts/fira_32x64.bin")),
        }
    }
    pub fn size(self) -> Size {
        let (width, height, _) = self.data();
        Size::new(width as u32, height as u32)
    }

    /// Draw from the top-left; newlines advance one cell. Unsupported characters use '?'.
    pub fn draw<D: DrawTarget<Color = Gray4>>(
        self,
        target: &mut D,
        text: &str,
        origin: Point,
        ink: u8,
    ) -> Result<(), D::Error> {
        let (width, height, data) = self.data();
        let glyph_len = (width * height / 2) as usize;
        let mut cursor = origin;
        for character in text.chars() {
            if character == '\n' {
                cursor = Point::new(origin.x, cursor.y + height);
                continue;
            }
            let character = displayed(character);
            let index = character as usize - 0x20;
            let glyph = &data[index * glyph_len..][..glyph_len];
            let coverage = glyph.iter().flat_map(|&byte| [byte >> 4, byte & 15]);
            let visible = coverage.enumerate().filter(|&(_, coverage)| coverage > 0);
            let pixels = visible.map(|(offset, coverage)| {
                // Blend ink over white, leaving uncovered pixels untouched.
                let offset = offset as i32;
                let position = cursor + Point::new(offset % width, offset / width);
                Pixel(position, Gray4::new(15 - coverage * (15 - ink) / 15))
            });
            target.draw_iter(pixels)?;
            cursor.x += width;
        }
        Ok(())
    }
}
