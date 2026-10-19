//! Build the default scene for one wake cycle and always power down before returning.
use crate::{
    badge,
    config::{BadgeConfig, LayoutConfig},
    ui::{Backend, RefreshRequest, RenderError},
};

pub trait Hardware: Backend {
    /// Power the panel rail, allow it to settle, and sample the battery.
    fn measure(&mut self) -> f32;
    fn power_off(&mut self);
}

pub fn run<H: Hardware>(
    hardware: &mut H,
    badge: &BadgeConfig<'_>,
    layout: &LayoutConfig,
    full_after: Option<u32>,
) -> Result<(), RenderError<H::Error>> {
    let result = badge::frame(badge, layout, hardware.measure())
        .map_err(RenderError::InvalidLayout)
        .and_then(|pipeline| pipeline.refresh(hardware, RefreshRequest::Changed, full_after));
    hardware.power_off();
    result
}
