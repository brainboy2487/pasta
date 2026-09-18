// src/stdlib/graphics/builtins.rs
//! Graphics builtin adapters for the interpreter.
//!
//! This file provides concrete helper functions that create and manipulate
//! `Window` and `Canvas` objects from the stdlib graphics module. The
//! `register` function is intentionally left as a small stub showing common
//! registration patterns; adapt it to your runtime's registration API (see
//! examples in the comments).
//!
//! The helpers return `Result<..., String>` so they can be called directly
//! from interpreter dispatch code (e.g., `Executor::call_builtin`) or wrapped
//! into closures when registering with a `Runtime` object.

use crate::stdlib::graphics::canvas::{Canvas, SharedCanvas, rgb_to_u32};
use crate::stdlib::graphics::window::Window;
use std::sync::{Arc, Mutex};

/// Register graphics builtins with the interpreter runtime.
///
/// **Important:** adapt the body of this function to your runtime's API.
/// Two common patterns are shown below as comments:
///
/// 1) If your runtime exposes `register_fn(name: &str, f: fn(Vec<Value>) -> Result<Value>)`:
/// ```ignore
/// rt.register_fn("WINDOW", |vals| { /* convert vals -> args, call builtin_window */ });
/// ```
///
/// 2) If your runtime exposes `register(name: &str, adapter: AdapterBox)` (closure taking runtime & args):
/// ```ignore
/// rt.register("WINDOW", Box::new(|rt, args| { /* call builtin_window */ }));
/// ```
///
/// If you paste your runtime registration signature here I can generate the exact code.
pub fn register(_rt: &mut crate::interpreter::Executor) {
    // Example (pseudo-code):
    // rt.register_fn("WINDOW", |vals| {
    //     // convert vals to (title, w, h) and call builtin_window(...)
    // });
    //
    // Keep this function as the single place to wire builtins into your runtime.
}

/// Create a new Window. Args (as strings) expected: [title, width, height]
///
/// Returns a boxed Window on success.
pub fn builtin_window(args: Vec<String>) -> Result<Box<Window>, String> {
    if args.len() != 3 {
        return Err("WINDOW expects 3 arguments: title, width, height".to_string());
    }
    let title = &args[0];
    let width = args[1].parse::<usize>().map_err(|_| "WINDOW: width must be an integer".to_string())?;
    let height = args[2].parse::<usize>().map_err(|_| "WINDOW: height must be an integer".to_string())?;
    let w = Window::new(title, width, height).map_err(|e| format!("WINDOW: {}", e))?;
    Ok(Box::new(w))
}

/// Create a new Canvas. Args: [width, height]
///
/// Returns a SharedCanvas (Arc<Mutex<Canvas>>).
pub fn builtin_canvas(args: Vec<String>) -> Result<SharedCanvas, String> {
    if args.len() != 2 {
        return Err("CANVAS expects 2 arguments: width, height".to_string());
    }
    let width = args[0].parse::<usize>().map_err(|_| "CANVAS: width must be an integer".to_string())?;
    let height = args[1].parse::<usize>().map_err(|_| "CANVAS: height must be an integer".to_string())?;
    let c = Canvas::new(width, height);
    Ok(Arc::new(Mutex::new(c)))
}

/// Set a pixel on a SharedCanvas.
/// Args: canvas_handle (SharedCanvas), x, y, r, g, b
///
/// This helper assumes the caller already has a `SharedCanvas` reference.
/// If your interpreter stores handles (strings/ids) instead, resolve the handle
/// to a `SharedCanvas` before calling this function.
pub fn builtin_pixel(canvas: &SharedCanvas, x: isize, y: isize, r: u8, g: u8, b: u8) -> Result<(), String> {
    let color = rgb_to_u32(r, g, b);
    let mut c = canvas.lock().map_err(|_| "builtin_pixel: canvas lock poisoned".to_string())?;
    c.set_pixel(x, y, color);
    Ok(())
}

/// Blit a SharedCanvas to a Window.
/// Args: window (mutable reference), canvas (SharedCanvas)
pub fn builtin_blit(window: &mut Window, canvas: &SharedCanvas) -> Result<(), String> {
    let c = canvas.lock().map_err(|_| "builtin_blit: canvas lock poisoned".to_string())?;
    window.blit(&*c).map_err(|e| format!("BLIT failed: {}", e))
}

/// Query whether a Window is open. Returns `true` if open.
pub fn builtin_window_open(window: &mut Window) -> Result<bool, String> {
    Ok(window.is_open())
}

/// Close a Window (destroy resources).
pub fn builtin_close(window: &mut Window) -> Result<(), String> {
    window.close();
    Ok(())
}

