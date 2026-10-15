extern crate alloc;

use embedded_graphics::{pixelcolor::Gray4, prelude::*};
use std::{error::Error, fs::File, path::Path};

#[path = "../../firmware/src/badge.rs"]
mod badge;
#[path = "../../firmware/src/config.rs"]
mod config;

#[cfg(test)]
#[path = "../../firmware/src/refresh.rs"]
mod refresh;

#[cfg(test)]
mod tests;

const PREVIEW_BATTERY_VOLTS: f32 = 3.98;
const OUTPUT_PATH: &str = "preview.png";

struct Framebuffer {
    pixels: Vec<u8>,
}

impl Framebuffer {
    fn new() -> Self {
        Self {
            pixels: vec![255; (config::PANEL_SIZE.width * config::PANEL_SIZE.height) as usize],
        }
    }

    fn write_png(&self, path: impl AsRef<Path>) -> Result<(), Box<dyn Error>> {
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

fn main() -> Result<(), Box<dyn Error>> {
    let mut framebuffer = Framebuffer::new();
    badge::draw(
        &mut framebuffer,
        &config::BADGE,
        &config::LAYOUT,
        PREVIEW_BATTERY_VOLTS,
    )?;
    framebuffer.write_png(OUTPUT_PATH)?;
    println!("wrote {OUTPUT_PATH}");
    Ok(())
}
