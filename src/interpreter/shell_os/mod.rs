//! pasta/src/interpreter/shell_os/mod.rs
//!
//! Integration adapter for the imported shell_OS code.
//! - Re-exports the original shell_OS submodules.
//! - Provides small, stable adapter functions the interpreter can call.
//! - Keeps the original CLI loop unchanged; adapters are thin wrappers.

pub mod cli;
pub mod commands;
pub mod vfs;
pub mod ops_log;

pub use cli::run_cli;

use crate::interpreter::environment::Value;
use crate::interpreter::executor::Executor;

/// Start an interactive shell session using a VFS booted from disk if available,
/// otherwise an empty VFS. Blocks until the user exits the shell.
///
/// The adapter accepts the Executor so it can access and mutate the executor's
/// environment without requiring two simultaneous mutable borrows.
pub fn run_shell(exe: &mut Executor) -> Result<Value, String> {
    // Create an empty VFS instance. Adjust later to boot from image or map `exe.env`.
    let mut vfs = vfs::Vfs::new_empty();

    // Verbosity can be derived from exe/env later; keep false for now.
    let verbose = false;

    // Run the CLI loop; map its unit result into a Pasta Value.
    cli::run_cli(&mut vfs, verbose).map(|_| Value::None)
}

/// Run the shell CLI using the provided VFS instance. Useful for tests or
/// when the caller wants to control the VFS lifecycle.
pub fn run_shell_with_vfs(vfs: &mut vfs::Vfs, verbose: bool) -> Result<Value, String> {
    cli::run_cli(vfs, verbose).map(|_| Value::None)
}
