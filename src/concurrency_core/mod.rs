#![allow(dead_code)]
// concurrency_core module root
// Purpose: standalone async/threading runtime core for Pasta.
// This module is intentionally self-contained and pure Rust.
// TODO: Confirm public API surface and integration plan before wiring into interpreter.

pub mod task;
pub mod scheduler;
pub mod clock;
pub mod lineage;
pub mod adoption;
pub mod async_runtime;
pub mod bridge;
pub mod snapshot;

pub use task::{TaskHandle, TaskStatus};
pub use scheduler::Runtime;
