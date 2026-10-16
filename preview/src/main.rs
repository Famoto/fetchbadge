use nametag_preview::{badge, config, Framebuffer};
use std::error::Error;

const PREVIEW_BATTERY_VOLTS: f32 = 3.98;
const OUTPUT_PATH: &str = "preview.png";

fn main() -> Result<(), Box<dyn Error>> {
    let mut framebuffer = Framebuffer::default();
    badge::Renderer::new(&mut framebuffer, &config::BADGE, &config::LAYOUT)
        .draw(PREVIEW_BATTERY_VOLTS)?;
    framebuffer.write_png(OUTPUT_PATH)?;
    println!("wrote {OUTPUT_PATH}");
    Ok(())
}
