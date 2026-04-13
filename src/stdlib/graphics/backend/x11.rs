//! X11 native window backend for pasta graphics stdlib.
//!
//! Pixel pipeline:
//!   Canvas (ARGB u32 vec)
//!     → as_bgra_bytes() → Vec<u8> (BGRA, 4 bytes/pixel)
//!       → XCreateImage / XPutImage → X server → screen
//!
//!   CompactCanvas (2-byte pixel format)
//!     → to_bgra_bytes() → Vec<u8> (BGRA, 4 bytes/pixel)
//!       → XCreateImage / XPutImage → X server → screen
//!
//! WM_DELETE_WINDOW is registered so closing the window sets open=false
//! without crashing the process.
//!
//! Thread safety: X11Window is Send (display pointer is only touched from
//! the thread that owns it — the interpreter main thread).

use super::BackendWindow;
use crate::stdlib::graphics::canvas::Canvas;
use crate::stdlib::graphics::compact_pixel::CompactCanvas;

use std::ffi::CString;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Once;

use libc::{c_int, c_void, free, malloc};
use x11::xlib::{
    self, ButtonPress, ButtonRelease, ClientMessage, ConfigureNotify, CurrentTime, DestroyNotify,
    Display, EnterNotify, Expose, ExposureMask, FocusChangeMask, FocusIn, FocusOut, KeyPress,
    KeyPressMask, KeyRelease, KeyReleaseMask, LeaveNotify, MapNotify, MotionNotify,
    PointerMotionMask, ReparentNotify, RevertToParent, StructureNotifyMask, UnmapNotify,
    Window as XWindow, XCloseDisplay, XCreateGC, XCreateImage, XCreateSimpleWindow, XDefaultDepth,
    XDefaultScreen, XDefaultVisual, XDestroyImage, XDestroyWindow, XErrorEvent, XEvent, XFlush,
    XImage, XInternAtom, XLookupKeysym, XMapWindow, XNextEvent, XOpenDisplay, XPending, XPutImage,
    XRaiseWindow, XRootWindow, XSelectInput, XSetErrorHandler, XSetInputFocus, XSetWMProtocols,
    XStoreName, XSync, ZPixmap, GC,
};

static X_INIT: Once = Once::new();
static ERROR_OCCURRED: AtomicBool = AtomicBool::new(false);

// Enhanced X11 error handler with specific error identification
extern "C" fn x11_error_handler(_display: *mut Display, error_event: *mut XErrorEvent) -> c_int {
    unsafe {
        let error = &*error_event;
        let error_name = match error.error_code {
            1 => "BadRequest",
            2 => "BadValue",
            3 => "BadWindow",
            4 => "BadPixmap",
            5 => "BadAtom",
            6 => "BadCursor",
            7 => "BadFont",
            8 => "BadMatch",
            9 => "BadDrawable",
            10 => "BadAccess",
            _ => "Unknown",
        };
        let request_name = match error.request_code {
            42 => "SetInputFocus",
            43 => "GetInputFocus",
            12 => "ConfigureWindow",
            8 => "MapWindow",
            _ => "Unknown",
        };
        eprintln!(
            "[pasta/x11] X11 error: {}({}) from {}({})",
            error_name, error.error_code, request_name, error.request_code
        );

        // Only treat certain errors as fatal
        match error.error_code {
            3 | 4 | 9 => {
                // BadWindow, BadPixmap, BadDrawable - these are serious
                eprintln!("[pasta/x11] Fatal X11 error detected!");
                ERROR_OCCURRED.store(true, Ordering::Relaxed);
            }
            _ => {
                // Other errors are non-fatal warnings
                eprintln!("[pasta/x11] Non-fatal X11 error, continuing...");
            }
        }
    }
    0 // Don't crash
}

fn ensure_x_threads() {
    X_INIT.call_once(|| unsafe {
        xlib::XInitThreads();
        // Install our non-fatal error handler
        XSetErrorHandler(Some(x11_error_handler));
    });
}

// ─────────────────────────────────────────────────────────────────────────────

/// Native X11 window and upload state used by the graphics backend.
pub struct X11Window {
    display: *mut Display,
    screen: i32,
    window: XWindow,
    gc: GC,
    /// Window width in pixels.
    pub width: usize,
    /// Window height in pixels.
    pub height: usize,
    open: AtomicBool,
    mapped: AtomicBool,
    focused: AtomicBool,
    wm_delete: xlib::Atom,
    /// Cached XImage — reused across blits when dimensions match.
    ximage: *mut XImage,
    /// Raw pixel buffer owned by us (malloc'd), pointed to by ximage.data.
    xbuf: *mut c_void,
    xbuf_bytes: usize,
    /// Keyboard events pending collection by WINDOW_KEY.
    key_queue: std::collections::VecDeque<String>,
    /// Track if we've successfully requested focus
    #[allow(dead_code)]
    focus_requested: AtomicBool,
}

// SAFETY: X11Window is only used from the interpreter's main thread.
unsafe impl Send for X11Window {}

impl X11Window {
    /// Close the native X11 window and release upload buffers.
    pub fn close(&mut self) {
        unsafe {
            if !self.ximage.is_null() {
                (*self.ximage).data = std::ptr::null_mut();
                x11::xlib::XDestroyImage(self.ximage);
                self.ximage = std::ptr::null_mut();
            }
            if !self.xbuf.is_null() {
                libc::free(self.xbuf);
                self.xbuf = std::ptr::null_mut();
            }
            x11::xlib::XDestroyWindow(self.display, self.window);
            x11::xlib::XSync(self.display, 0);
            x11::xlib::XCloseDisplay(self.display);
        }
        self.open.store(false, std::sync::atomic::Ordering::SeqCst);
    }

    /// Create a new X11 window with the requested title and size.
    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, String> {
        ensure_x_threads();
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(
                    "XOpenDisplay failed — is DISPLAY set? Try: export DISPLAY=:0".to_string(),
                );
            }

            let screen = XDefaultScreen(display);
            let root = XRootWindow(display, screen);
            let depth = XDefaultDepth(display, screen);

            // Use black background, white border
            let black = xlib::XBlackPixel(display, screen);
            let _white = xlib::XWhitePixel(display, screen);

            let window = XCreateSimpleWindow(
                display,
                root,
                50,
                50, // Position at (50,50) instead of (0,0) to avoid edge issues
                width as u32,
                height as u32,
                2,     // border width (make it more visible)
                black, // border color
                black, // background color
            );

            if window == 0 {
                XCloseDisplay(display);
                return Err("XCreateSimpleWindow failed".to_string());
            }

            // Set window title
            let ctitle = CString::new(title).unwrap_or_default();
            XStoreName(display, window, ctitle.as_ptr());

            // Register WM_DELETE_WINDOW protocol
            let _wm_protocols = CString::new("WM_PROTOCOLS").unwrap();
            let wm_delete_str = CString::new("WM_DELETE_WINDOW").unwrap();
            let wm_delete = XInternAtom(display, wm_delete_str.as_ptr(), 0);
            XSetWMProtocols(
                display,
                window,
                &wm_delete as *const xlib::Atom as *mut xlib::Atom,
                1,
            );

            // Select comprehensive input events
            XSelectInput(
                display,
                window,
                ExposureMask
                    | KeyPressMask
                    | KeyReleaseMask
                    | StructureNotifyMask
                    | FocusChangeMask
                    | PointerMotionMask,
            );

            let gc = XCreateGC(display, window, 0, ptr::null_mut());

            // Map window and wait for it to be ready
            XMapWindow(display, window);
            XFlush(display);

            // Wait for MapNotify event to ensure window is actually mapped
            let mut map_received = false;
            let mut attempts = 0;
            while !map_received && attempts < 100 {
                if XPending(display) > 0 {
                    let mut ev: XEvent = mem::zeroed();
                    XNextEvent(display, &mut ev);
                    if ev.get_type() == MapNotify {
                        map_received = true;
                    } else {
                    }
                }
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(1));
            }

            if !map_received {}

            // Now try to raise and focus the window
            XRaiseWindow(display, window);
            XFlush(display);

            // Small delay before requesting focus
            std::thread::sleep(std::time::Duration::from_millis(10));

            // Request focus (this might fail, that's OK)
            ERROR_OCCURRED.store(false, Ordering::Relaxed);
            XSetInputFocus(display, window, RevertToParent, CurrentTime);
            XSync(display, 0);

            let focus_success = !ERROR_OCCURRED.load(Ordering::Relaxed);
            if focus_success {
            } else {
            }

            // Pre-allocate pixel buffer
            let bytes = width * height * 4;
            let xbuf = malloc(bytes) as *mut c_void;
            if xbuf.is_null() {
                XDestroyWindow(display, window);
                XCloseDisplay(display);
                return Err("malloc failed for X11 pixel buffer".to_string());
            }
            // Zero-init (black)
            ptr::write_bytes(xbuf as *mut u8, 0, bytes);

            let visual = XDefaultVisual(display, screen);
            let ximage = XCreateImage(
                display,
                visual,
                depth as u32,
                ZPixmap,
                0,
                xbuf as *mut i8,
                width as u32,
                height as u32,
                32,
                (width * 4) as i32,
            );

            if ximage.is_null() {
                free(xbuf);
                XDestroyWindow(display, window);
                XCloseDisplay(display);
                return Err("XCreateImage failed".to_string());
            }

            Ok(Self {
                display,
                screen,
                window,
                gc,
                width,
                height,
                open: AtomicBool::new(true),
                mapped: AtomicBool::new(map_received),
                focused: AtomicBool::new(focus_success),
                wm_delete,
                ximage,
                xbuf,
                xbuf_bytes: bytes,
                key_queue: std::collections::VecDeque::new(),
                focus_requested: AtomicBool::new(focus_success),
            })
        }
    }

    /// Rebuild XImage if canvas dimensions changed.
    unsafe fn ensure_ximage(&mut self, w: usize, h: usize) {
        if w == self.width && h == self.height && !self.ximage.is_null() {
            return;
        }
        // Free old resources (null out data ptr first so XDestroyImage doesn't double-free)
        if !self.ximage.is_null() {
            (*self.ximage).data = ptr::null_mut();
            XDestroyImage(self.ximage);
            self.ximage = ptr::null_mut();
        }
        if !self.xbuf.is_null() {
            free(self.xbuf);
            self.xbuf = ptr::null_mut();
        }

        let bytes = w * h * 4;
        let xbuf = malloc(bytes) as *mut c_void;
        if xbuf.is_null() {
            return;
        }
        ptr::write_bytes(xbuf as *mut u8, 0, bytes);

        let depth = XDefaultDepth(self.display, self.screen);
        let visual = XDefaultVisual(self.display, self.screen);
        let ximage = XCreateImage(
            self.display,
            visual,
            depth as u32,
            ZPixmap,
            0,
            xbuf as *mut i8,
            w as u32,
            h as u32,
            32,
            (w * 4) as i32,
        );
        if ximage.is_null() {
            free(xbuf);
            return;
        }

        self.xbuf = xbuf;
        self.xbuf_bytes = bytes;
        self.ximage = ximage;
        self.width = w;
        self.height = h;
    }

    /// Copy BGRA pixels from canvas into the XImage buffer.
    unsafe fn upload_canvas(&mut self, canvas: &Canvas) {
        self.ensure_ximage(canvas.width, canvas.height);
        if self.xbuf.is_null() {
            return;
        }

        let dst = self.xbuf as *mut u8;
        let bytes = canvas.as_bytes();

        // 0xAARRGGBB on little-endian is laid out in memory as B,G,R,A, which is
        // directly usable for the 32bpp XImage path used here.
        if cfg!(target_endian = "little") {
            ptr::copy_nonoverlapping(bytes.as_ptr(), dst, bytes.len());
            return;
        }

        let pixels = &canvas.pixels;
        for (i, &px) in pixels.iter().enumerate() {
            let base = i * 4;
            *dst.add(base) = (px & 0xFF) as u8;
            *dst.add(base + 1) = ((px >> 8) & 0xFF) as u8;
            *dst.add(base + 2) = ((px >> 16) & 0xFF) as u8;
            *dst.add(base + 3) = ((px >> 24) & 0xFF) as u8;
        }
    }

    /// Copy BGRA pixels from a CompactCanvas (2-byte pixels) into the XImage buffer.
    unsafe fn upload_compact_canvas(&mut self, canvas: &CompactCanvas) {
        self.ensure_ximage(canvas.width, canvas.height);
        if self.xbuf.is_null() {
            return;
        }

        let dst = self.xbuf as *mut u8;

        // Convert compact 2-byte pixels to BGRA (4 bytes: B G R pad)
        // CompactPixel decodes to RGB via RGB332 or intensity mode
        for (i, &px_raw) in canvas.pixels.iter().enumerate() {
            let px = crate::stdlib::graphics::compact_pixel::CompactPixel(px_raw);
            let (r, g, b) = px.to_rgb(); // Use to_rgb() which respects RGB332/intensity mode
            let base = i * 4;
            *dst.add(base) = b;
            *dst.add(base + 1) = g;
            *dst.add(base + 2) = r;
            *dst.add(base + 3) = 0; // padding
        }
    }

    /// Handle pending X events. Returns false if window was closed.
    pub fn poll(&mut self) -> bool {
        unsafe {
            while XPending(self.display) > 0 {
                let mut ev: XEvent = mem::zeroed();
                XNextEvent(self.display, &mut ev);

                match ev.get_type() {
                    // Window destruction events - close the window
                    t if t == ClientMessage => {
                        let atom = ev.client_message.data.get_long(0) as xlib::Atom;
                        if atom == self.wm_delete {
                            self.open.store(false, Ordering::SeqCst);
                            return false;
                        }
                    }
                    t if t == DestroyNotify => {
                        self.open.store(false, Ordering::SeqCst);
                        return false;
                    }

                    // Window mapping/visibility events - update state but don't close
                    t if t == MapNotify => {
                        self.mapped.store(true, Ordering::SeqCst);
                    }
                    t if t == UnmapNotify => {
                        self.mapped.store(false, Ordering::SeqCst);
                        // Don't close window - it might just be minimized
                    }

                    // Window management events - normal WM behavior
                    t if t == ReparentNotify => {
                        // This is expected when WM decorates our window
                    }
                    t if t == ConfigureNotify => {
                        let config = ev.configure;
                        // Update our size if it changed
                        if config.width as usize != self.width
                            || config.height as usize != self.height
                        {}
                    }

                    // Focus events - track focus state
                    t if t == FocusIn => {
                        self.focused.store(true, Ordering::SeqCst);
                    }
                    t if t == FocusOut => {
                        self.focused.store(false, Ordering::SeqCst);
                    }

                    // Redraw events
                    t if t == Expose => {
                        // Application should handle redraws, just log for now
                    }

                    // Keyboard events - only process if window has focus and is mapped
                    t if t == KeyPress => {
                        if !self.focused.load(Ordering::SeqCst) {
                            continue;
                        }
                        if !self.mapped.load(Ordering::SeqCst) {
                            continue;
                        }

                        let ks = XLookupKeysym(&mut ev.key as *mut _, 0) as u64;

                        let key_to_add = match ks {
                            // Arrow keys
                            0xff51 => Some("Left".to_string()),
                            0xff52 => Some("Up".to_string()),
                            0xff53 => Some("Right".to_string()),
                            0xff54 => Some("Down".to_string()),
                            // Escape key
                            0xff1b => Some("Escape".to_string()),
                            // Enter/Return
                            0xff0d | 0xff8d => Some("Enter".to_string()),
                            // Space and printable ASCII
                            0x0020..=0x007e => {
                                let ch = (ks as u8) as char;
                                Some(ch.to_string())
                            }
                            // Function keys
                            0xffbe..=0xffc9 => {
                                let f_num = (ks - 0xffbe + 1) as u8;
                                Some(format!("F{}", f_num))
                            }
                            _ => None,
                        };

                        if let Some(key) = key_to_add {
                            self.key_queue.push_back(key);
                        }
                    }

                    t if t == KeyRelease => {}

                    // Mouse events (log but don't handle for now)
                    t if t == ButtonPress => {}
                    t if t == ButtonRelease => {}
                    t if t == MotionNotify => {
                        // Don't log every motion event, too verbose
                    }

                    // Mouse enter/leave events
                    t if t == EnterNotify => {}
                    t if t == LeaveNotify => {}

                    _ => {}
                }
            }

            // Return current window state
            self.open.load(Ordering::SeqCst)
        }
    }

    /// Return the most recent key pressed since the last call, clearing the queue.
    /// Returns an empty string if no key was pressed.
    /// This is called by WINDOW_KEY() function in PASTA.
    /// NOTE: Does NOT poll for new events - that should be done by is_open() calls.
    pub fn latest_key(&mut self) -> String {
        // Get the most recent key and clear the entire queue
        let result = if let Some(key) = self.key_queue.back().cloned() {
            key
        } else {
            String::new()
        };

        // Clear the queue after getting the result
        self.key_queue.clear();
        result
    }

    /// Push canvas pixels to the X11 window immediately.
    pub fn present(&mut self, canvas: &Canvas) -> Result<(), String> {
        unsafe {
            self.upload_canvas(canvas);
            if self.ximage.is_null() {
                return Err("X11: no XImage available for present".to_string());
            }
            XPutImage(
                self.display,
                self.window,
                self.gc,
                self.ximage,
                0,
                0,
                0,
                0,
                self.width as u32,
                self.height as u32,
            );
            XFlush(self.display);
            Ok(())
        }
    }

    /// Push compact canvas pixels (2-byte format) to the X11 window.
    pub fn present_compact(&mut self, canvas: &CompactCanvas) -> Result<(), String> {
        unsafe {
            self.upload_compact_canvas(canvas);
            if self.ximage.is_null() {
                return Err("X11: no XImage available for present_compact".to_string());
            }
            XPutImage(
                self.display,
                self.window,
                self.gc,
                self.ximage,
                0,
                0,
                0,
                0,
                self.width as u32,
                self.height as u32,
            );
            XFlush(self.display);
            Ok(())
        }
    }
}

impl BackendWindow for X11Window {
    fn blit(&mut self, canvas: &Canvas) -> Result<(), String> {
        self.present(canvas)
    }

    fn is_open(&mut self) -> bool {
        self.poll()
    }

    fn close(&mut self) {
        unsafe {
            // Null out XImage data pointer before destroy to prevent double-free
            if !self.ximage.is_null() {
                (*self.ximage).data = ptr::null_mut();
                XDestroyImage(self.ximage);
                self.ximage = ptr::null_mut();
            }
            if !self.xbuf.is_null() {
                free(self.xbuf);
                self.xbuf = ptr::null_mut();
            }
            XDestroyWindow(self.display, self.window);
            XSync(self.display, 0);
            XCloseDisplay(self.display);
        }
        self.open.store(false, Ordering::SeqCst);
    }
}

impl Drop for X11Window {
    fn drop(&mut self) {
        if self.open.load(Ordering::SeqCst) {
            self.close();
        } else {
            // Already closed — just free the buffer if XImage was detached
            if !self.xbuf.is_null() && self.ximage.is_null() {
                unsafe {
                    free(self.xbuf);
                }
                self.xbuf = ptr::null_mut();
            }
        }
    }
}
