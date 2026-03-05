//! src/interpreter/shell.rs
//!
//! A mini shell implementation for PASTA that provides shell-like access to the filesystem
//! and system operations. This module contains hand-rolled Rust implementations of common
//! shell commands (cd, ls, mkdir, rm, cp, mv, cat, touch, etc.) that can be called from
//! PASTA scripts.
//!
//! Design:
//! - No external dependencies beyond std::fs and std::io
//! - Pure Rust implementations of shell commands
//! - Returns PASTA Value types for integration with the interpreter
//! - Maintains current working directory state
//! - Provides detailed error messages

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::interpreter::environment::Value;

/// Shell state for the PASTA interpreter, tracking current working directory
/// and other shell-related state
#[derive(Debug, Clone)]
pub struct Shell {
    pub cwd: PathBuf,
}

impl Shell {
    /// Create a new shell with the current working directory
    pub fn new() -> Result<Self> {
        let cwd = env::current_dir()?;
        Ok(Shell { cwd })
    }

    /// Get the current working directory as a string
    pub fn pwd(&self) -> String {
        self.cwd.display().to_string()
    }

    /// Change directory
    pub fn cd(&mut self, path: &str) -> Result<String> {
        let target = if path.is_empty() || path == "~" {
            dirs_home()?
        } else if path == ".." {
            self.cwd.parent().ok_or_else(|| anyhow!("Cannot go above root"))?.to_path_buf()
        } else if path == "." {
            self.cwd.clone()
        } else if path.starts_with('~') {
            let home = dirs_home()?;
            let rest = &path[1..];
            home.join(rest.trim_start_matches('/'))
        } else if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if !target.exists() {
            return Err(anyhow!("Directory does not exist: {}", path));
        }
        if !target.is_dir() {
            return Err(anyhow!("Not a directory: {}", path));
        }

        self.cwd = target;
        Ok(format!("Changed to {}", self.cwd.display()))
    }

    /// List directory contents
    pub fn ls(&self, path: Option<&str>) -> Result<Vec<Value>> {
        let dir_path = if let Some(p) = path {
            if p == "." {
                self.cwd.clone()
            } else if p == ".." {
                self.cwd.parent().unwrap_or(&self.cwd).to_path_buf()
            } else if Path::new(p).is_absolute() {
                PathBuf::from(p)
            } else {
                self.cwd.join(p)
            }
        } else {
            self.cwd.clone()
        };

        if !dir_path.exists() {
            return Err(anyhow!("Path does not exist: {}", dir_path.display()));
        }

        if dir_path.is_file() {
            // Single file: return its name
            return Ok(vec![Value::String(
                dir_path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| dir_path.display().to_string())
            )]);
        }

        let mut entries = Vec::new();
        for entry in fs::read_dir(&dir_path)? {
            if let Ok(entry) = entry {
                if let Ok(file_name) = entry.file_name().into_string() {
                    entries.push(Value::String(file_name));
                }
            }
        }

        entries.sort_by(|a, b| {
            if let (Value::String(sa), Value::String(sb)) = (a, b) {
                sa.cmp(sb)
            } else {
                std::cmp::Ordering::Equal
            }
        });

        Ok(entries)
    }

    /// List directory with detailed info (name, size, type)
    pub fn ls_long(&self, path: Option<&str>) -> Result<Vec<Value>> {
        let dir_path = if let Some(p) = path {
            if Path::new(p).is_absolute() {
                PathBuf::from(p)
            } else {
                self.cwd.join(p)
            }
        } else {
            self.cwd.clone()
        };

        if !dir_path.exists() {
            return Err(anyhow!("Path does not exist: {}", dir_path.display()));
        }

        let mut results = Vec::new();

        if dir_path.is_file() {
            let metadata = fs::metadata(&dir_path)?;
            let size = metadata.len();
            let is_dir = metadata.is_dir();
            let file_type = if is_dir { "dir" } else { "file" };
            let name = dir_path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            results.push(Value::String(format!("{} {} {}", name, size, file_type)));
            return Ok(results);
        }

        for entry in fs::read_dir(&dir_path)? {
            if let Ok(entry) = entry {
                let metadata = entry.metadata()?;
                let size = metadata.len();
                let is_dir = metadata.is_dir();
                let file_type = if is_dir { "dir" } else { "file" };
                if let Ok(name) = entry.file_name().into_string() {
                    results.push(Value::String(format!("{} {} {}", name, size, file_type)));
                }
            }
        }

        results.sort_by(|a, b| {
            if let (Value::String(sa), Value::String(sb)) = (a, b) {
                sa.cmp(sb)
            } else {
                std::cmp::Ordering::Equal
            }
        });

        Ok(results)
    }

    /// Create a directory
    pub fn mkdir(&self, path: &str, parents: bool) -> Result<String> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if parents {
            fs::create_dir_all(&full_path)?;
        } else {
            fs::create_dir(&full_path)?;
        }

        Ok(format!("Created directory: {}", full_path.display()))
    }

    /// Remove a file
    pub fn rm(&self, path: &str) -> Result<String> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if !full_path.exists() {
            return Err(anyhow!("File does not exist: {}", path));
        }

        if full_path.is_dir() {
            return Err(anyhow!("Cannot remove directory (use rmdir): {}", path));
        }

        fs::remove_file(&full_path)?;
        Ok(format!("Removed file: {}", full_path.display()))
    }

    /// Remove a directory (empty)
    pub fn rmdir(&self, path: &str) -> Result<String> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if !full_path.exists() {
            return Err(anyhow!("Directory does not exist: {}", path));
        }

        if !full_path.is_dir() {
            return Err(anyhow!("Not a directory: {}", path));
        }

        fs::remove_dir(&full_path)?;
        Ok(format!("Removed directory: {}", full_path.display()))
    }

    /// Remove directory and all contents recursively
    pub fn rmdir_recursive(&self, path: &str) -> Result<String> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if !full_path.exists() {
            return Err(anyhow!("Path does not exist: {}", path));
        }

        if full_path.is_file() {
            fs::remove_file(&full_path)?;
        } else {
            fs::remove_dir_all(&full_path)?;
        }

        Ok(format!("Removed: {}", full_path.display()))
    }

    /// Read file contents
    pub fn cat(&self, path: &str) -> Result<Vec<u8>> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if !full_path.exists() {
            return Err(anyhow!("File does not exist: {}", path));
        }

        fs::read(&full_path)
            .map_err(|e| anyhow!("Cannot read file: {}", e))
    }

    /// Create an empty file or update its timestamp
    pub fn touch(&self, path: &str) -> Result<String> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        if full_path.exists() {
            // File exists, just update timestamp
            let _ = fs::File::open(&full_path)?;
        } else {
            // Create empty file
            fs::File::create(&full_path)?;
        }

        Ok(format!("Touched: {}", full_path.display()))
    }

    /// Copy a file
    pub fn cp(&self, from: &str, to: &str) -> Result<String> {
        let from_path = if Path::new(from).is_absolute() {
            PathBuf::from(from)
        } else {
            self.cwd.join(from)
        };

        let to_path = if Path::new(to).is_absolute() {
            PathBuf::from(to)
        } else {
            self.cwd.join(to)
        };

        if !from_path.exists() {
            return Err(anyhow!("Source file does not exist: {}", from));
        }

        if from_path.is_dir() {
            return Err(anyhow!("Cannot copy directory (use cp -r): {}", from));
        }

        fs::copy(&from_path, &to_path)?;
        Ok(format!("Copied {} to {}", from_path.display(), to_path.display()))
    }

    /// Move/rename a file
    pub fn mv(&self, from: &str, to: &str) -> Result<String> {
        let from_path = if Path::new(from).is_absolute() {
            PathBuf::from(from)
        } else {
            self.cwd.join(from)
        };

        let to_path = if Path::new(to).is_absolute() {
            PathBuf::from(to)
        } else {
            self.cwd.join(to)
        };

        if !from_path.exists() {
            return Err(anyhow!("Source does not exist: {}", from));
        }

        fs::rename(&from_path, &to_path)?;
        Ok(format!("Moved {} to {}", from_path.display(), to_path.display()))
    }

    /// Check if path exists
    pub fn exists(&self, path: &str) -> bool {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        full_path.exists()
    }

    /// Check if path is a file
    pub fn is_file(&self, path: &str) -> bool {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        full_path.is_file()
    }

    /// Check if path is a directory
    pub fn is_dir(&self, path: &str) -> bool {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        full_path.is_dir()
    }

    /// Get file size in bytes
    pub fn file_size(&self, path: &str) -> Result<u64> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        fs::metadata(&full_path)
            .map(|m| m.len())
            .map_err(|e| anyhow!("Cannot get file size: {}", e))
    }

    /// Get absolute path
    pub fn realpath(&self, path: &str) -> Result<String> {
        let full_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else {
            self.cwd.join(path)
        };

        let canonical = match fs::canonicalize(&full_path) {
            Ok(path) => path,
            Err(_) => {
                // If canonicalize fails, at least return the normalized path
                full_path
            }
        };

        Ok(canonical.display().to_string())
    }
}

impl Shell {
    /// Execute a simple shell command string, returning human-readable output.
    ///
    /// Supported commands are a small subset of the methods on `Shell`:
    ///
    /// ```text
    /// pwd
    /// cd <path>
    /// ls [path]
    /// ls_long [path]
    /// mkdir [-p] <path>
    /// rm <file>
    /// rmdir <dir>
    /// rmdir_r <path>
    /// cat <file>
    /// touch <file>
    /// cp <src> <dst>
    /// mv <src> <dst>
    /// exists <path>
    /// is_file <path>
    /// is_dir <path>
    /// file_size <path>
    /// realpath <path>
    /// help
    /// exit
    /// ```
    ///
    /// The output is a string suitable for printing; lists are joined with
    /// newline separators.
    pub fn run_command(&mut self, line: &str) -> Result<String> {
        let mut parts = line.split_whitespace();
        let cmd = match parts.next() {
            Some(c) => c,
            None => return Ok(String::new()),
        };

        // helper for formatting list values produced by shell methods
        fn format_list(lst: Vec<Value>) -> String {
            lst.into_iter()
                .map(|v| match v {
                    Value::String(s) => s,
                    Value::Number(n) => {
                        if n.fract() == 0.0 && n.abs() < 1e15 {
                            format!("{}", n as i64)
                        } else {
                            format!("{}", n)
                        }
                    }
                    other => format!("{:?}", other),
                })
                .collect::<Vec<_>>()
                .join("\n")
        }

        match cmd {
            "pwd" => Ok(self.pwd()),
            "cd" => {
                let arg = parts.next().unwrap_or("");
                self.cd(arg)
            }
            "ls" => {
                let arg = parts.next();
                let list = self.ls(arg);
                Ok(format_list(list?))
            }
            "ls_long" => {
                let arg = parts.next();
                let list = self.ls_long(arg);
                Ok(format_list(list?))
            }
            "mkdir" => {
                let mut parents = false;
                let mut path = None;
                for p in parts {
                    if p == "-p" || p == "-P" {
                        parents = true;
                    } else if path.is_none() {
                        path = Some(p);
                    }
                }
                let path = path.unwrap_or("");
                self.mkdir(path, parents)
            }
            "rm" => {
                let path = parts.next().unwrap_or("");
                self.rm(path)
            }
            "rmdir" => {
                let path = parts.next().unwrap_or("");
                self.rmdir(path)
            }
            "rmdir_r" | "rmdir_recursive" | "rm_r" => {
                let path = parts.next().unwrap_or("");
                self.rmdir_recursive(path)
            }
            "cat" => {
                let path = parts.next().unwrap_or("");
                let bytes = self.cat(path)?;
                String::from_utf8(bytes).map_err(|e| anyhow!("invalid utf8: {}", e))
                    .map(|s| s)
            }
            "touch" => {
                let path = parts.next().unwrap_or("");
                self.touch(path)
            }
            "cp" => {
                let from = parts.next().unwrap_or("");
                let to = parts.next().unwrap_or("");
                self.cp(from, to)
            }
            "mv" => {
                let from = parts.next().unwrap_or("");
                let to = parts.next().unwrap_or("");
                self.mv(from, to)
            }
            "exists" => {
                let path = parts.next().unwrap_or("");
                Ok(self.exists(path).to_string())
            }
            "is_file" => {
                let path = parts.next().unwrap_or("");
                Ok(self.is_file(path).to_string())
            }
            "is_dir" => {
                let path = parts.next().unwrap_or("");
                Ok(self.is_dir(path).to_string())
            }
            "file_size" => {
                let path = parts.next().unwrap_or("");
                let sz = self.file_size(path)?;
                Ok(sz.to_string())
            }
            "realpath" => {
                let path = parts.next().unwrap_or("");
                self.realpath(path)
            }
            "help" => Ok("shell commands: pwd cd ls ls_long mkdir [-p] rm rmdir \
                            rmdir_r cat touch cp mv exists is_file is_dir file_size realpath exit".to_string()),
            _ => Err(anyhow!("unknown shell command: {}", cmd)),
        }
    }
}

impl Default for Shell {
    fn default() -> Self {
        Shell::new().unwrap_or_else(|_| Shell {
            cwd: PathBuf::from("/"),
        })
    }
}

/// Helper to get home directory
fn dirs_home() -> Result<PathBuf> {
    if let Ok(home) = env::var("HOME") {
        Ok(PathBuf::from(home))
    } else if let Ok(home) = env::var("USERPROFILE") {
        // Windows
        Ok(PathBuf::from(home))
    } else {
        Err(anyhow!("Cannot determine home directory"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_creation() {
        let shell = Shell::new();
        assert!(shell.is_ok());
    }

    #[test]
    fn shell_pwd() {
        let shell = Shell::new().unwrap();
        let pwd = shell.pwd();
        assert!(!pwd.is_empty());
    }

    #[test]
    fn shell_is_file_and_is_dir() {
        let shell = Shell::new().unwrap();
        assert!(shell.is_dir("."));
        assert!(shell.exists("."));
    }

    #[test]
    fn shell_mkdir_and_rmdir() {
        let shell = Shell::new().unwrap();
        let test_dir = "/tmp/pasta_shell_test_mkdir";

        // Clean up if it exists
        let _ = shell.rmdir_recursive(test_dir);

        // Create directory
        let result = shell.mkdir(test_dir, false);
        assert!(result.is_ok());
        assert!(shell.is_dir(test_dir));

        // Remove directory
        let result = shell.rmdir(test_dir);
        assert!(result.is_ok());
    }

    #[test]
    fn shell_touch_and_rm() {
        let shell = Shell::new().unwrap();
        let test_file = "/tmp/pasta_shell_test_file.txt";

        // Clean up if it exists
        let _ = fs::remove_file(test_file);

        // Create file
        let result = shell.touch(test_file);
        assert!(result.is_ok());
        assert!(shell.is_file(test_file));

        // Remove file
        let result = shell.rm(test_file);
        assert!(result.is_ok());
        assert!(!shell.exists(test_file));
    }

    #[test]
    fn shell_cp() {
        let shell = Shell::new().unwrap();
        let src = "/tmp/pasta_shell_cp_src.txt";
        let dst = "/tmp/pasta_shell_cp_dst.txt";

        // Clean up
        let _ = fs::remove_file(src);
        let _ = fs::remove_file(dst);

        // Create source
        fs::write(src, "test content").unwrap();

        // Copy
        let result = shell.cp(src, dst);
        assert!(result.is_ok());
        assert!(shell.is_file(dst));

        // Clean up
        let _ = fs::remove_file(src);
        let _ = fs::remove_file(dst);
    }

    #[test]
    fn shell_ls() {
        let shell = Shell::new().unwrap();
        let result = shell.ls(Some("."));
        assert!(result.is_ok());
        let entries = result.unwrap();
        assert!(!entries.is_empty());
    }

    #[test]
    fn shell_realpath() {
        let shell = Shell::new().unwrap();
        let result = shell.realpath(".");
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(!path.is_empty());
    }

    #[test]
    fn shell_run_command_basic() {
        let mut shell = Shell::new().unwrap();
        let out = shell.run_command("pwd").unwrap();
        assert!(!out.is_empty());
    }

    #[test]
    fn shell_run_command_cd_and_pwd() {
        let mut shell = Shell::new().unwrap();
        let orig = shell.pwd();
        let _ = shell.run_command("cd /").unwrap();
        assert_eq!(shell.pwd(), "/");
        // return to original location
        let _ = shell.run_command(&format!("cd {}", orig));
    }
}
