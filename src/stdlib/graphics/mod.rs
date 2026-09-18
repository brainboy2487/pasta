// src/stdlib/graphics/mod.rs
//! Graphics stdlib registration (Linux/X11) - Now importable only
/// Native backend traits and implementations.
pub mod backend;
/// Canvas storage and pixel helpers.
pub mod canvas;
/// Compact pixel/canvas representations used by fast paths.
pub mod compact_pixel;
/// Drawing primitives and color palette helpers.
pub mod draw;
/// Platform-agnostic window wrapper.
pub mod window;

/// Importable stateless graphics helper functions.
pub mod api;
/// Window event queue helpers.
pub mod events;
/// Pixel format conversion utilities.
pub mod pixel_format;

// Re-export compact pixel types for easier access
use crate::interpreter::Executor as Runtime;
pub use compact_pixel::{CompactCanvas, CompactPixel};
pub use pixel_format::Rgb; // adjust to your runtime path

/// Register stateless importable graphics helper functions.
pub fn register_graphics_api(rt: &mut Runtime) {
    // Color helpers
    rt.register_builtin("color_rgb".to_string(), api::color_rgb);
    rt.register_builtin("color_rgba".to_string(), api::color_rgba);
    rt.register_builtin("color_hsv".to_string(), api::color_hsv);
    rt.register_builtin("color_lerp".to_string(), api::color_lerp);

    // 16-bit / RGB565 color helpers
    rt.register_builtin("color_rgb16".to_string(), api::color_rgb16);
    rt.register_builtin("color_from565".to_string(), api::color_from565);
    rt.register_builtin("color_to565".to_string(), api::color_to565);
    rt.register_builtin("color_wheel".to_string(), api::color_wheel);
    rt.register_builtin("palette565_size".to_string(), api::palette565_size);
}
