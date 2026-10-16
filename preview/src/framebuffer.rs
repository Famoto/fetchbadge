use crate::config;
use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use std::{error::Error, fs::File, path::Path};

/// White grayscale framebuffer shared by the preview and host tests.
pub struct Framebuffer {
    pixels: Vec<u8>,
}

impl Default for Framebuffer {
    fn default() -> Self {
        Self { pixels: vec![255; (config::PANEL_SIZE.width * config::PANEL_SIZE.height) as usize] }
    }
}

impl Framebuffer {
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    pub fn write_png(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
        let file = File::create(path)?;
        let mut encoder =
            png::Encoder::new(file, config::PANEL_SIZE.width, config::PANEL_SIZE.height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.write_header()?.write_image_data(&self.pixels)?;
        Ok(())
    }
}

impl OriginDimensions for Framebuffer {
    fn size(&self) -> Size {
        config::PANEL_SIZE
    }
}

impl DrawTarget for Framebuffer {
    type Color = Gray4;
    type Error = core::convert::Infallible;
    fn draw_iter<I: IntoIterator<Item = Pixel<Gray4>>>(
        &mut self,
        pixels: I,
    ) -> Result<(), Self::Error> {
        for Pixel(p, c) in pixels {
            if self.bounding_box().contains(p) {
                self.pixels[p.y as usize * config::PANEL_SIZE.width as usize + p.x as usize] =
                    c.luma() * 17;
            }
        }
        Ok(())
    }
}
