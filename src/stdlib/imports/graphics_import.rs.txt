// src/stdlib/imports/graphics_import.rs
//! Import shim for graphics. Called by import handler to register builtins.

use crate::interpreter::Runtime;

pub fn import_graphics(rt: &mut Runtime) {
    crate::stdlib::graphics::register_builtins(rt);
}
