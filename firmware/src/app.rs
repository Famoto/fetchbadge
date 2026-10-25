//! Build the default scene for one wake cycle and always power down before returning.
use crate::{
    badge,
    config::{BadgeConfig, LayoutConfig},
    ui::{Backend, RefreshRequest, RenderError},
};
use embedded_graphics::prelude::Point;

pub trait Hardware: Backend {
    /// Power the panel rail, allow it to settle, and sample the battery.
    fn measure(&mut self) -> f32;
    /// Whether this wake has touch input; `wait_tap` returns None without it.
    fn touch_enabled(&self) -> bool {
        false
    }
    /// Block until the next tap in panel coordinates, or None once the touch window closes.
    fn wait_tap(&mut self) -> Option<Point> {
        None
    }
    fn power_off(&mut self);
}

pub fn run<H: Hardware>(
    hardware: &mut H,
    badge: &BadgeConfig<'_>,
    layout: &LayoutConfig,
    full_after: Option<u32>,
) -> Result<(), RenderError<H::Error>> {
    let result = present(hardware, badge, layout, full_after);
    hardware.power_off();
    result
}

/// Draw the badge; with touch, toggle the full-screen QR on taps until the window closes.
fn present<H: Hardware>(
    hardware: &mut H,
    badge: &BadgeConfig<'_>,
    layout: &LayoutConfig,
    full_after: Option<u32>,
) -> Result<(), RenderError<H::Error>> {
    let volts = hardware.measure();
    let touch = hardware.touch_enabled();
    // Partial updates fade neighbouring rows on this panel, so touch mode always redraws fully.
    let request = if touch { RefreshRequest::Full } else { RefreshRequest::Changed };
    let hinted = BadgeConfig { qr_caption: badge.qr_touch_caption, ..*badge };
    let mut pipeline = badge::frame(if touch { &hinted } else { badge }, layout, volts)
        .map_err(RenderError::InvalidLayout)?;
    pipeline.refresh(hardware, request, full_after)?;
    if !touch {
        return Ok(());
    }
    while let Some(tap) = hardware.wait_tap() {
        if pipeline.get(badge::QR_FULL).is_some() {
            pipeline.deregister(badge::QR_FULL).map_err(RenderError::InvalidLayout)?;
        } else if layout.bonus.area.contains(tap) {
            let qr = badge::qr_full(badge, layout);
            pipeline.register(badge::QR_FULL, qr).map_err(RenderError::InvalidLayout)?;
        } else {
            continue;
        }
        pipeline.refresh(hardware, request, full_after)?;
    }
    // Window over: back to the plain badge, hint gone.
    badge::frame(badge, layout, volts)
        .map_err(RenderError::InvalidLayout)?
        .refresh(hardware, request, full_after)
}
