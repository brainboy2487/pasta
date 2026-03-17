//! Graphics backend — platform window abstraction.
//!
//! BackendWindow trait + platform implementations.
//! Linux: X11 (always compiled on linux, no feature flag needed for struct access)
//! The `x11` Cargo feature gates the actual X11 crate dependency.

use crate::stdlib::graphics::canvas::Canvas;

pub trait BackendWindow {
    fn blit(&mut self, canvas: &Canvas) -> Result<(), String>;
    fn is_open(&mut self) -> bool;
    fn close(&mut self);
}

#[cfg(target_os = "linux")]
pub mod x11;

#[cfg(windows)]
pub mod win32;

/// Create a platform window. Falls back gracefully if no display is available.
pub fn create_window(
    title: &str,
    width: usize,
    height: usize,
) -> Result<Box<dyn BackendWindow + Send>, String> {
    #[cfg(all(target_os = "linux", feature = "x11"))]
    {
        return Ok(Box::new(x11::X11Window::new(title, width, height)?));
    }
    #[cfg(not(all(target_os = "linux", feature = "x11")))]
    {
        let _ = (title, width, height);
        Err("No native window backend available. Rebuild with --features x11".to_string())
    }
}
