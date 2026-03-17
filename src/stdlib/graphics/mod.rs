// src/stdlib/graphics/mod.rs
//! Graphics stdlib registration (Linux/X11).
pub mod canvas;
pub mod draw;
pub mod window;
pub mod backend;
pub mod builtins;

use crate::interpreter::Executor as Runtime; // adjust to your runtime path

pub fn register_builtins(rt: &mut Runtime) {
    builtins::register(rt);
}
