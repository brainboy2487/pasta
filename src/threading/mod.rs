//! src/threading/mod.rs
//!
//! The PASTA threading subsystem.
//!
//! Modules:
//!   threads    — PastaThread, ThreadStatus, global THREAD_REGISTRY
//!   thread_man — ThreadManager: spawn, kill, query
//!   thread_api — thin public API used by executor and REPL

pub mod threads;
pub mod thread_man;
pub mod thread_api;

pub use threads::{
    ThreadStatus, PastaThread, ThreadRegistry,
    THREAD_REGISTRY, next_thread_id,
    register_thread, finish_thread, kill_thread, list_threads,
};
pub use thread_man::ThreadManager;
pub use thread_api::{spawn_script_thread, kill_thread_by_id, threads_snapshot};
