// src/stdlib/graphics/backend/win32.rs
//! Minimal Win32 backend stub. Implement CreateWindowEx and StretchDIBits blit here.
// TODO: implement using winapi or windows-rs.

use crate::stdlib::graphics::canvas::Canvas;
use super::BackendWindow;

pub struct Win32Window {
    open: bool,
}

impl Win32Window {
    pub fn new(_title: &str, _width: usize, _height: usize) -> Result<Self, String> {
        // TODO: create Win32 window and DIB section
        Ok(Self { open: true })
    }
}

impl BackendWindow for Win32Window {
    fn blit(&mut self, _canvas: &Canvas) -> Result<(), String> {
        // TODO: call StretchDIBits or UpdateLayeredWindow with canvas.as_bytes()
        Ok(())
    }
    fn is_open(&mut self) -> bool {
        self.open
    }
    fn close(&mut self) {
        self.open = false;
        // TODO: destroy window
    }
}
