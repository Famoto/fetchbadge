//! Fira Code glyphs: 4-bit coverage, high nibble first, 15 = full ink.
use embedded_graphics::{pixelcolor::Gray4, prelude::*};

pub struct Font {
    pub width: i32,
    pub height: i32,
    data: &'static [u8],
}

pub static SMALL: Font = Font::new(10, 20, include_bytes!("../fonts/fira_10x20.bin"));
pub static TEXT: Font = Font::new(16, 32, include_bytes!("../fonts/fira_16x32.bin"));
pub static TITLE: Font = Font::new(32, 64, include_bytes!("../fonts/fira_32x64.bin"));

impl Font {
    const fn new(width: i32, height: i32, data: &'static [u8]) -> Self {
        Self { width, height, data }
    }

    /// Draw from the top-left; newlines advance one cell. Unsupported characters use '?'.
    pub fn draw<D: DrawTarget<Color = Gray4>>(
        &self,
        target: &mut D,
        text: &str,
        origin: Point,
        ink: u8,
    ) -> Result<(), D::Error> {
        let glyph_len = (self.width * self.height / 2) as usize;
        let mut cursor = origin;
        for character in text.chars() {
            if character == '\n' {
                cursor = Point::new(origin.x, cursor.y + self.height);
                continue;
            }
            let character =
                if character.is_ascii_graphic() || character == ' ' { character } else { '?' };
            let index = character as usize - 0x20;
            let glyph = &self.data[index * glyph_len..][..glyph_len];
            let coverage = glyph.iter().flat_map(|&byte| [byte >> 4, byte & 15]);
            let visible = coverage.enumerate().filter(|&(_, coverage)| coverage > 0);
            let pixels = visible.map(|(offset, coverage)| {
                // Blend ink over white, leaving uncovered pixels untouched.
                let offset = offset as i32;
                let position = cursor + Point::new(offset % self.width, offset / self.width);
                Pixel(position, Gray4::new(15 - coverage * (15 - ink) / 15))
            });
            target.draw_iter(pixels)?;
            cursor.x += self.width;
        }
        Ok(())
    }
}
