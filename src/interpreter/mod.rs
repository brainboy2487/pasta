// pasta/src/interpreter/mod.rs
//! Interpreter module for PASTA
//!
//! Submodules:
//! - `environment`   — value types and scope chain
//! - `executor`      — public Executor type + startup/shutdown/orchestration
//! - `ex_frame`      — frame & scope management helpers (used by executor + ex_eval)
//! - `ex_eval`       — statement and expression evaluation
//! - `errors`        — centralized error types and traceback
//! - `int_api`       — stable external API for subsystems (module loader, REPL, tests)
//! - `repl`          — interactive REPL loop
//! - `shell`         — existing lightweight shell API
//! - `shell_os`      — integrated shell OS adapter
//! - `ai_network`    — AI layer utilities

pub mod environment;
/// Core statement and expression executor (see [`executor::Executor`]).
pub mod executor;
/// Frame and scope management helpers.
pub mod ex_frame;
/// Statement and expression evaluation.
pub mod ex_eval;
pub mod errors;

// Public interpreter API used by external subsystems (module loader, REPL, tests).
pub mod int_api;

// Explicitly bind the repl module to the repl.rs file in this directory.
#[path = "repl.rs"]
pub mod repl;

pub mod ai_network;
pub mod shell;     // existing lightweight shell API
pub mod shell_os;  // imported shell_OS module (adapter + code)

pub use executor::Executor;
pub use environment::{Environment, ThreadMeta, Value};

// Re-export interpreter API types for external consumers (module loader, tests).
pub use int_api::{InterpreterApi, ModuleEnvHandle, InterpreterSnapshot, default_interpreter_api};

// Helpful compile-time check: if repl.rs is missing, this test will fail early.
#[cfg(test)]
mod __repl_file_presence_check {
    use crate::interpreter::repl;

    #[test]
    fn repl_module_present() {
        let _ = repl::run_repl;
    }
}
