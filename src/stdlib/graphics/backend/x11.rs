//! X11 native window backend for pasta graphics stdlib.
//!
//! Pixel pipeline:
//!   Canvas (ARGB u32 vec)
//!     → as_bgra_bytes() → Vec<u8> (BGRA, 4 bytes/pixel)
//!       → XCreateImage / XPutImage → X server → screen
//!
//! WM_DELETE_WINDOW is registered so closing the window sets open=false
//! without crashing the process.
//!
//! Thread safety: X11Window is Send (display pointer is only touched from
//! the thread that owns it — the interpreter main thread).

use crate::stdlib::graphics::canvas::Canvas;
use super::BackendWindow;

use std::ffi::CString;
use std::ptr;
use std::mem;
use std::sync::Once;
use std::sync::atomic::{AtomicBool, Ordering};

use libc::{malloc, free, c_void};
use x11::xlib::{
    self,
    Display, Window as XWindow, GC, XImage,
    XOpenDisplay, XCloseDisplay, XDefaultScreen,
    XRootWindow, XDefaultDepth, XDefaultVisual,
    XCreateSimpleWindow, XMapWindow, XStoreName,
    XCreateGC, XFlush, XSync,
    XPending, XNextEvent, XEvent,
    XPutImage, XCreateImage, XDestroyImage,
    XDestroyWindow, XInternAtom, XSetWMProtocols,
    XSelectInput, ZPixmap,
    ExposureMask, KeyPressMask, StructureNotifyMask,
    ClientMessage,
};

static X_INIT: Once = Once::new();

fn ensure_x_threads() {
    X_INIT.call_once(|| unsafe { xlib::XInitThreads(); });
}

// ─────────────────────────────────────────────────────────────────────────────

pub struct X11Window {
    display:        *mut Display,
    screen:         i32,
    window:         XWindow,
    gc:             GC,
    pub width:      usize,
    pub height:     usize,
    open:           AtomicBool,
    wm_delete:      xlib::Atom,
    /// Cached XImage — reused across blits when dimensions match.
    ximage:         *mut XImage,
    /// Raw pixel buffer owned by us (malloc'd), pointed to by ximage.data.
    xbuf:           *mut c_void,
    xbuf_bytes:     usize,
}

// SAFETY: X11Window is only used from the interpreter's main thread.
unsafe impl Send for X11Window {}

impl X11Window {
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

    pub fn new(title: &str, width: usize, height: usize) -> Result<Self, String> {
        ensure_x_threads();
        unsafe {
            let display = XOpenDisplay(ptr::null());
            if display.is_null() {
                return Err(
                    "XOpenDisplay failed — is DISPLAY set? Try: export DISPLAY=:0".to_string()
                );
            }

            let screen = XDefaultScreen(display);
            let root   = XRootWindow(display, screen);
            let depth  = XDefaultDepth(display, screen);

            // Use black background, white border
            let black = xlib::XBlackPixel(display, screen);
            let white = xlib::XWhitePixel(display, screen);

            let window = XCreateSimpleWindow(
                display, root,
                0, 0,
                width as u32, height as u32,
                1,            // border width
                black,        // border color
                black,        // background color
            );

            if window == 0 {
                XCloseDisplay(display);
                return Err("XCreateSimpleWindow failed".to_string());
            }

            // Set window title
            let ctitle = CString::new(title).unwrap_or_default();
            XStoreName(display, window, ctitle.as_ptr());

            // Register WM_DELETE_WINDOW protocol
            let wm_protocols = CString::new("WM_PROTOCOLS").unwrap();
            let wm_delete_str = CString::new("WM_DELETE_WINDOW").unwrap();
            let wm_delete = XInternAtom(display, wm_delete_str.as_ptr(), 0);
            XSetWMProtocols(
                display, window,
                &wm_delete as *const xlib::Atom as *mut xlib::Atom,
                1
            );

            // Select input events
            XSelectInput(display, window,
                ExposureMask | KeyPressMask | StructureNotifyMask);

            let gc = XCreateGC(display, window, 0, ptr::null_mut());

            XMapWindow(display, window);
            XFlush(display);

            // Pre-allocate pixel buffer
            let bytes = width * height * 4;
            let xbuf  = malloc(bytes) as *mut c_void;
            if xbuf.is_null() {
                XDestroyWindow(display, window);
                XCloseDisplay(display);
                return Err("malloc failed for X11 pixel buffer".to_string());
            }
            // Zero-init (black)
            ptr::write_bytes(xbuf as *mut u8, 0, bytes);

            let visual = XDefaultVisual(display, screen);
            let ximage = XCreateImage(
                display, visual,
                depth as u32,
                ZPixmap,
                0,
                xbuf as *mut i8,
                width  as u32,
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
                open:        AtomicBool::new(true),
                wm_delete,
                ximage,
                xbuf,
                xbuf_bytes:  bytes,
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

        let bytes  = w * h * 4;
        let xbuf   = malloc(bytes) as *mut c_void;
        if xbuf.is_null() { return; }
        ptr::write_bytes(xbuf as *mut u8, 0, bytes);

        let depth  = XDefaultDepth(self.display, self.screen);
        let visual = XDefaultVisual(self.display, self.screen);
        let ximage = XCreateImage(
            self.display, visual,
            depth as u32, ZPixmap, 0,
            xbuf as *mut i8,
            w as u32, h as u32,
            32, (w * 4) as i32,
        );
        if ximage.is_null() { free(xbuf); return; }

        self.xbuf       = xbuf;
        self.xbuf_bytes = bytes;
        self.ximage     = ximage;
        self.width      = w;
        self.height     = h;
    }

    /// Copy BGRA pixels from canvas into the XImage buffer.
    unsafe fn upload_canvas(&mut self, canvas: &Canvas) {
        self.ensure_ximage(canvas.width, canvas.height);
        if self.xbuf.is_null() { return; }

        let dst = self.xbuf as *mut u8;
        let pixels = &canvas.pixels;

        // Convert ARGB (0xAARRGGBB) → BGRA (4 bytes: B G R pad)
        // X11 ZPixmap 32bpp on little-endian: byte order is B G R X
        for (i, &px) in pixels.iter().enumerate() {
            let r = ((px >> 16) & 0xFF) as u8;
            let g = ((px >>  8) & 0xFF) as u8;
            let b = ( px        & 0xFF) as u8;
            let base = i * 4;
            *dst.add(base)     = b;
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
                    t if t == ClientMessage => {
                        let atom = ev.client_message.data.get_long(0) as xlib::Atom;
                        if atom == self.wm_delete {
                            self.open.store(false, Ordering::SeqCst);
                        }
                    }
                    _ => {}
                }
            }
            self.open.load(Ordering::SeqCst)
        }
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
                0, 0, 0, 0,
                self.width  as u32,
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
                unsafe { free(self.xbuf); }
                self.xbuf = ptr::null_mut();
            }
        }
    }
}
