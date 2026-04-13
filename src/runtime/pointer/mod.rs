//! PASTA Pointer & Reference System (v1.4.4)
//!
//! This module implements the unified pointer abstraction for PASTA, providing:
//! - Memory pointers (MEM)
//! - File handles (FILE)
//! - Device handles (DEV)
//! - Network handles (NET)
//!
//! Each pointer type supports PULL/PUSH operations and integrates with
//! the Saucey GC for automatic lifetime management.

pub mod pointer;
pub mod registry;
pub mod context;
pub mod ops;
pub mod gc;

pub use pointer::{Pointer, PointerKind, PointerMetadata, PointerId};
pub use registry::{PointerRegistry, SharedPointerRegistry, new_shared_registry};
pub use context::PointerContext;
pub use gc::PointerGcTracker;
