//! Host access to the hardware-independent firmware and preview framebuffer.
extern crate alloc;

#[path = "../../firmware/src/app.rs"]
pub mod app;
#[path = "../../firmware/src/badge.rs"]
pub mod badge;
#[path = "../../firmware/src/config.rs"]
pub mod config;
#[path = "../../firmware/src/refresh.rs"]
pub mod refresh;

mod framebuffer;
pub use framebuffer::Framebuffer;
