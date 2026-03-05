// pasta/src/interpreter/mod.rs
//! Interpreter module for PASTA
//!
//! Submodules:
//! - `environment`
//! - `executor`
//! - `repl`
//! - `shell` (legacy, small API)
//! - `shell_os` (imported shell_OS module)

pub mod environment;
pub mod executor;
pub mod errors;

// Explicitly bind the repl module to the repl.rs file in this directory.
// This is robust on case-sensitive filesystems and when files are moved.
#[path = "repl.rs"]
pub mod repl;

pub mod ai_network;
pub mod shell;     // existing lightweight shell API
pub mod shell_os;  // imported shell_OS module (adapter + code)

pub use executor::Executor;
pub use environment::{Environment, ThreadMeta, Value};

// Helpful compile-time check: if repl.rs is missing, this test will fail early
#[cfg(test)]
mod __repl_file_presence_check {
    // This test will fail to compile if the `repl` module file is missing.
    #[test]
    fn repl_module_present() {
        // If the module is present, this will compile and run as a no-op.
        let _ = crate::repl::run_repl;
    }
}
