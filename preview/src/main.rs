use nametag_preview::{badge, config, ui::ViewSet, Framebuffer};
use std::error::Error;

const PREVIEW_BATTERY_VOLTS: f32 = 3.98;
const OUTPUT_PATH: &str = "preview.png";

fn main() -> Result<(), Box<dyn Error>> {
    let mut framebuffer = Framebuffer::default();
    let pipeline = badge::frame(&config::BADGE, &config::LAYOUT, PREVIEW_BATTERY_VOLTS)?;
    pipeline.draw(&mut framebuffer, ViewSet::MAX)?;
    framebuffer.write_png(OUTPUT_PATH)?;
    println!("wrote {OUTPUT_PATH}");
    Ok(())
}
