//! Fira Code glyphs: 4-bit coverage, high nibble first, 15 = full ink.
use embedded_graphics::{pixelcolor::Gray4, prelude::*};

pub struct Font {
    pub width: i32,
    pub height: i32,
    data: &'static [u8],
}

pub static SMALL: Font = Font {
    width: 10,
    height: 20,
    data: include_bytes!("../fonts/fira_10x20.bin"),
};
pub static TEXT: Font = Font {
    width: 16,
    height: 32,
    data: include_bytes!("../fonts/fira_16x32.bin"),
};
pub static TITLE: Font = Font {
    width: 32,
    height: 64,
    data: include_bytes!("../fonts/fira_32x64.bin"),
};

impl Font {
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
            let index = if (' '..='~').contains(&character) {
                character as usize - 0x20
            } else {
                '?' as usize - 0x20
            };
            let glyph = &self.data[index * glyph_len..][..glyph_len];
            let pixels = (0..self.width * self.height).filter_map(|offset| {
                let packed = glyph[offset as usize / 2];
                let coverage = if offset % 2 == 0 {
                    packed >> 4
                } else {
                    packed & 15
                };
                // Blend ink over the white background using glyph coverage.
                (coverage > 0).then(|| {
                    Pixel(
                        cursor + Point::new(offset % self.width, offset / self.width),
                        Gray4::new(15 - coverage * (15 - ink) / 15),
                    )
                })
            });
            target.draw_iter(pixels)?;
            cursor.x += self.width;
        }
        Ok(())
    }
}
