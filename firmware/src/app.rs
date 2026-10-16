//! Sequential wake-cycle controller, independent of the ESP hardware.
use crate::refresh::{self, SavedState, UpdatePlan};

pub trait Hardware {
    type Error;
    /// Power the panel rail, allow it to settle, and sample the battery.
    fn measure(&mut self) -> f32;
    fn load_saved(&mut self) -> SavedState;
    fn store_saved(&mut self, saved: SavedState);
    /// Clear and render the update; the controller flushes it separately.
    fn render(&mut self, update: &UpdatePlan) -> Result<(), Self::Error>;
    fn flush(&mut self) -> Result<(), Self::Error>;
    fn power_off(&mut self);
}

/// Keep returned errors inside the update scope so panel shutdown always follows.
pub fn run<H: Hardware>(
    hardware: &mut H,
    button_or_timer_wake: bool,
    full_after: Option<u32>,
) -> Result<(), H::Error> {
    let result = (|| {
        let volts = hardware.measure();
        if let Some(update) =
            refresh::plan(hardware.load_saved(), button_or_timer_wake, volts, full_after)
        {
            hardware.store_saved(refresh::EMPTY);
            hardware.render(&update)?;
            hardware.flush()?;
            // The physical panel now matches the proposed retained reading.
            hardware.store_saved(update.next_saved);
        }
        Ok(())
    })();
    hardware.power_off();
    result
}
