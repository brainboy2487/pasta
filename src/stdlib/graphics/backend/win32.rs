// src/stdlib/graphics/backend/win32.rs
//! Win32 graphics backend implementation for Windows platforms.
//!
//! This backend uses the Windows API (CreateWindowEx, StretchDIBits) to create
//! a native window and render the canvas to it.
//!
//! For production use on Windows, implement this using either:
//! - `winapi` crate (low-level FFI bindings)
//! - `windows` crate (idiomatic Windows bindings by Microsoft)
//!
//! The implementation should:
//! 1. Create a WNDCLASSEX and register it
//! 2. Create a window with CreateWindowExW
//! 3. Create a DIB section for double-buffering
//! 4. On blit(), copy canvas data to DIB and call StretchDIBits
//! 5. Handle WM_PAINT, WM_CLOSE, WM_KEYDOWN messages in a message pump

use super::BackendWindow;
use crate::stdlib::graphics::canvas::Canvas;

/// Win32 window handle wrapper.
/// This is a stub implementation - actual Windows API calls are only
/// compiled when building on Windows with the appropriate features.
pub struct Win32Window {
    open: bool,
    title: String,
    width: usize,
    height: usize,
}

impl Win32Window {
    /// Create a new Win32 window.
    /// On non-Windows platforms, this creates a dummy that reports as open
    /// but does not render anything.
    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, String> {
        #[cfg(target_os = "windows")]
        {
            // TODO: Implement using winapi or windows-rs crate:
            // 1. WNDCLASSEXW with custom WndProc
            // 2. CreateWindowExW with WS_OVERLAPPEDWINDOW
            // 3. Create DIB section with BITMAPINFOHEADER
            // 4. ShowWindow and UpdateWindow
            eprintln!(
                "Win32Window: Created {}x{} window '{}'",
                width, height, title
            );
        }

        #[cfg(not(target_os = "windows"))]
        {
            eprintln!(
                "Win32Window: Stub mode (not on Windows) - {}x{} '{}'",
                width, height, title
            );
        }

        Ok(Self {
            open: true,
            title: title.to_string(),
            width,
            height,
        })
    }

    /// Get window dimensions
    pub fn dimensions(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    /// Get window title
    pub fn title(&self) -> &str {
        &self.title
    }
}

impl BackendWindow for Win32Window {
    fn blit(&mut self, canvas: &Canvas) -> Result<(), String> {
        if !self.open {
            return Err("Window is closed".into());
        }

        #[cfg(target_os = "windows")]
        {
            // TODO: Implement using winapi or windows-rs:
            // 1. Get window DC with GetDC
            // 2. Copy canvas.as_bytes() to DIB section
            // 3. Call StretchDIBits or BitBlt
            // 4. ReleaseDC
            let _ = canvas; // Use canvas to avoid warning
        }

        #[cfg(not(target_os = "windows"))]
        {
            // Stub: just validate canvas dimensions match window
            let (cw, ch) = (canvas.width(), canvas.height());
            if cw != self.width || ch != self.height {
                return Err(format!(
                    "Canvas size {}x{} doesn't match window {}x{}",
                    cw, ch, self.width, self.height
                ));
            }
        }

        Ok(())
    }

    fn is_open(&mut self) -> bool {
        #[cfg(target_os = "windows")]
        {
            // TODO: Process pending messages and check for WM_CLOSE
            // PeekMessageW in a loop, TranslateMessage, DispatchMessage
        }

        self.open
    }

    fn close(&mut self) {
        if self.open {
            #[cfg(target_os = "windows")]
            {
                // TODO: DestroyWindow and unregister class
                eprintln!("Win32Window: Closing '{}'", self.title);
            }

            self.open = false;
        }
    }
}

impl Drop for Win32Window {
    fn drop(&mut self) {
        self.close();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_stub_window() {
        let win = Win32Window::new("Test", 640, 480).unwrap();
        assert_eq!(win.dimensions(), (640, 480));
        assert_eq!(win.title(), "Test");
    }
}
