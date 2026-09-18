/// Filesystem command implementations (`ls`, `cd`, `mkdir`, `rm`, `cp`, `mv`).
pub mod fs_commands;

use crate::interpreter::shell_os::vfs::Vfs;

/// Trait implemented by all CLI commands.
/// Object-safe: no `Sized` bound and no default methods requiring `Self`.
pub trait Command {
    /// Return the command name as it appears on the command line (e.g. `"ls"`).
    fn name(&self) -> &'static str;
    /// Execute the command with the given argument tokens against `vfs`.
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String>;
}

pub use fs_commands::register_fs_commands;
