// src/stdlib/graphics/window.rs
//! Platform-agnostic Window wrapper. Backends implement BackendWindow.

use crate::stdlib::graphics::canvas::Canvas;
use crate::stdlib::graphics::backend::BackendWindow;

pub struct Window {
    pub backend: Box<dyn BackendWindow + Send>,
    pub width: usize,
    pub height: usize,
}

impl Window {
    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, String> {
        let backend = crate::stdlib::graphics::backend::create_window(title, width, height)?;
        Ok(Self { backend, width, height })
    }

    pub fn blit(&mut self, canvas: &Canvas) -> Result<(), String> { self.backend.blit(canvas) }
    pub fn is_open(&mut self) -> bool { self.backend.is_open() }
    pub fn close(&mut self) { self.backend.close(); }
}
