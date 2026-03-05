pub mod fs_commands;

use crate::interpreter::shell_os::vfs::Vfs;

/// Trait implemented by all CLI commands.
/// Object-safe: no `Sized` bound and no default methods requiring `Self`.
pub trait Command {
    fn name(&self) -> &'static str;
    fn run(&self, args: &[&str], vfs: &mut Vfs) -> Result<(), String>;
}

pub use fs_commands::register_fs_commands;
