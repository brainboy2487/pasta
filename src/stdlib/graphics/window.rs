// src/stdlib/graphics/window.rs
//! Platform-agnostic Window wrapper. Backends implement BackendWindow.

use crate::stdlib::graphics::backend::BackendWindow;
use crate::stdlib::graphics::canvas::Canvas;

/// Platform-neutral graphics window wrapper.
pub struct Window {
    /// Backend-specific window implementation.
    pub backend: Box<dyn BackendWindow + Send>,
    /// Current window width in pixels.
    pub width: usize,
    /// Current window height in pixels.
    pub height: usize,
}

impl Window {
    /// Create a new window using the active backend.
    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, String> {
        let backend = crate::stdlib::graphics::backend::create_window(title, width, height)?;
        Ok(Self {
            backend,
            width,
            height,
        })
    }

    /// Blit the provided canvas into the window.
    pub fn blit(&mut self, canvas: &Canvas) -> Result<(), String> {
        self.backend.blit(canvas)
    }
    /// Poll whether the underlying window remains open.
    pub fn is_open(&mut self) -> bool {
        self.backend.is_open()
    }
    /// Close the underlying native window.
    pub fn close(&mut self) {
        self.backend.close();
    }
}
