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
use std::io::Write;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Result};
use crate::interpreter::environment::Value;

/// IO redirection kind parsed from a command line.
#[derive(Debug, Default)]
enum Redirect {
    #[default]
    None,
    /// `> path` — truncate/create file and write stdout
    Out(String),
    /// `>> path` — append stdout to file
    Append(String),
    /// `< path` — stdin redirect marker (parsing only; command execution ignores it here)
    In,
}

/// Strip IO redirection operators from `line`, returning (clean_command, redirect).
/// Handles `>`, `>>`, and `<` that appear outside of quotes.
fn parse_redirect(line: &str) -> (String, Redirect) {
    // Scan characters respecting quotes to find the first unquoted redirection token.
    let chars: Vec<char> = line.chars().collect();
    let mut in_single = false;
    let mut in_double = false;
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '\'' if !in_double => in_single = !in_single,
            '"'  if !in_single => in_double = !in_double,
            '>'  if !in_single && !in_double => {
                let cmd_part = chars[..i].iter().collect::<String>();
                let rest     = chars[i+1..].iter().collect::<String>();
                if chars.get(i + 1) == Some(&'>') {
                    // ">>" append
                    let rest = chars[i+2..].iter().collect::<String>();
                    return (cmd_part.trim().to_string(), Redirect::Append(rest.trim().to_string()));
                }
                return (cmd_part.trim().to_string(), Redirect::Out(rest.trim().to_string()));
            }
            '<'  if !in_single && !in_double => {
                let cmd_part = chars[..i].iter().collect::<String>();
                return (cmd_part.trim().to_string(), Redirect::In);
            }
            _ => {}
        }
        i += 1;
    }
    (line.to_string(), Redirect::None)
}

/// Apply an IO redirect to a command result.
/// - `Redirect::None`   → return result unchanged
/// - `Redirect::Out`    → write result to file, return empty string on success
/// - `Redirect::Append` → append result to file, return empty string on success
/// - `Redirect::In`     → ignored here (stdin redirect not applicable to string output)
fn apply_redirect(result: Result<String>, redirect: &Redirect) -> Result<String> {
    match redirect {
        Redirect::None => result,
        Redirect::In => result, // stdin redirect: command already ran, nothing to do
        Redirect::Out(path) => {
            let output = result?;
            let mut f = fs::File::create(path)
                .map_err(|e| anyhow!("redirect >{}: {}", path, e))?;
            f.write_all(output.as_bytes())
                .map_err(|e| anyhow!("redirect >{}: {}", path, e))?;
            if !output.ends_with('\n') {
                f.write_all(b"\n").map_err(|e| anyhow!("redirect >{}: {}", path, e))?;
            }
            Ok(String::new())
        }
        Redirect::Append(path) => {
            let output = result?;
            let mut f = fs::OpenOptions::new().append(true).create(true).open(path)
                .map_err(|e| anyhow!("redirect >>{}: {}", path, e))?;
            f.write_all(output.as_bytes())
                .map_err(|e| anyhow!("redirect >>{}: {}", path, e))?;
            if !output.ends_with('\n') {
                f.write_all(b"\n").map_err(|e| anyhow!("redirect >>{}: {}", path, e))?;
            }
            Ok(String::new())
        }
    }
}

/// Expand a glob pattern against the filesystem, returning matching paths.
/// Supports `*` (any chars), `?` (one char), `[abc]` (char class).
/// Returns empty vec if the pattern contains no wildcards or has no matches.
pub fn expand_glob(pattern: &str, cwd: &Path) -> Vec<String> {
    if !pattern.contains('*') && !pattern.contains('?') && !pattern.contains('[') {
        return Vec::new(); // not a glob pattern
    }

    // Split into directory part and file pattern part.
    let (dir, file_pat) = if let Some(pos) = pattern.rfind('/') {
        (&pattern[..pos], &pattern[pos+1..])
    } else {
        ("", pattern)
    };

    let search_dir = if dir.is_empty() {
        cwd.to_path_buf()
    } else if Path::new(dir).is_absolute() {
        PathBuf::from(dir)
    } else {
        cwd.join(dir)
    };

    let Ok(entries) = fs::read_dir(&search_dir) else { return Vec::new(); };

    let mut matches = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if glob_match(file_pat, &name_str) {
            let full = if dir.is_empty() {
                name_str.to_string()
            } else {
                format!("{}/{}", dir, name_str)
            };
            matches.push(full);
        }
    }
    matches
}

/// Match a filename against a glob pattern.
/// Supports `*`, `?`, and `[...]` character classes.
fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    glob_match_inner(&p, &n)
}

fn glob_match_inner(p: &[char], n: &[char]) -> bool {
    match (p.first(), n.first()) {
        (None, None) => true,
        (None, _) => false,
        (Some(&'*'), _) => {
            // '*' matches zero or more chars
            glob_match_inner(&p[1..], n)
                || (!n.is_empty() && glob_match_inner(p, &n[1..]))
        }
        (_, None) => false,
        (Some(&'?'), _) => glob_match_inner(&p[1..], &n[1..]),
        (Some(&'['), _) => {
            // find closing ']'
            if let Some(close) = p[1..].iter().position(|&c| c == ']') {
                let class = &p[1..close+1];
                let matched = class.contains(&n[0]);
                if matched {
                    glob_match_inner(&p[close+2..], &n[1..])
                } else {
                    false
                }
            } else {
                p[0] == n[0] && glob_match_inner(&p[1..], &n[1..])
            }
        }
        (Some(pc), Some(nc)) => pc == nc && glob_match_inner(&p[1..], &n[1..]),
    }
}

/// Expand `$VAR` and `${VAR}` references against the process environment./// Unset variables expand to an empty string.
pub fn expand_env_vars(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '$' {
            if chars.peek() == Some(&'{') {
                chars.next(); // consume '{'
                let name: String = chars.by_ref().take_while(|&c| c != '}').collect();
                result.push_str(&std::env::var(&name).unwrap_or_default());
            } else {
                let name: String = chars.by_ref()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if name.is_empty() {
                    result.push('$');
                } else {
                    result.push_str(&std::env::var(&name).unwrap_or_default());
                }
            }
        } else {
            result.push(ch);
        }
    }
    result
}

/// Tokenize a shell command line respecting single/double quotes and backslash escapes.
/// `cat "file with spaces.txt"` → ["cat", "file with spaces.txt"]
/// `echo 'it\'s fine'`         → ["echo", "it's fine"]
pub fn tokenize_command(line: &str) -> Result<Vec<String>> {
    let mut tokens: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut chars = line.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if !in_single => {
                // Backslash escape: consume next char literally
                if let Some(next) = chars.next() {
                    current.push(next);
                } else {
                    current.push('\\');
                }
            }
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            other => {
                current.push(other);
            }
        }
    }

    if in_single {
        return Err(anyhow!("unterminated single quote in command"));
    }
    if in_double {
        return Err(anyhow!("unterminated double quote in command"));
    }
    if !current.is_empty() {
        tokens.push(current);
    }

    Ok(tokens)
}

/// Shell state for the PASTA interpreter, tracking current working directory
/// and other shell-related state
#[derive(Debug, Clone)]
pub struct Shell {
    /// The current working directory for this shell session.
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
        } else if let Some(rest) = path.strip_prefix('~') {
            let home = dirs_home()?;
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
        for entry in fs::read_dir(&dir_path)?.flatten() {
            if let Ok(file_name) = entry.file_name().into_string() {
                entries.push(Value::String(file_name));
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

        for entry in fs::read_dir(&dir_path)?.flatten() {
            let metadata = entry.metadata()?;
            let size = metadata.len();
            let is_dir = metadata.is_dir();
            let file_type = if is_dir { "dir" } else { "file" };
            if let Ok(name) = entry.file_name().into_string() {
                results.push(Value::String(format!("{} {} {}", name, size, file_type)));
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

    /// Run a pipeline of commands, feeding each stage's stdout into the next.
    fn run_pipeline(&mut self, stages: &[String]) -> Result<String> {
        let mut input = String::new();
        for (i, stage) in stages.iter().enumerate() {
            let stage = stage.trim().to_string();
            if stage.is_empty() {
                continue;
            }
            // For all stages except the first, inject the previous output as
            // a synthetic `echo`-style prefix by appending to stdin-bearing commands.
            // Since our shell commands are pure string→string, we feed the prior
            // output to grep/cat-like commands via a temporary file approach.
            let result = if i == 0 || input.is_empty() {
                self.run_command_single(&stage)
            } else {
                self.run_command_with_stdin(&stage, &input)
            };
            input = result?;
        }
        Ok(input)
    }

    /// Run a single command (no pipe splitting) and return its output.
    fn run_command_single(&mut self, line: &str) -> Result<String> {
        let (clean_line, redirect) = parse_redirect(line);
        let clean_line = expand_env_vars(&clean_line);
        let raw_tokens = tokenize_command(&clean_line)?;
        if raw_tokens.is_empty() { return Ok(String::new()); }
        let tokens: Vec<String> = {
            let mut expanded = vec![raw_tokens[0].clone()];
            for tok in &raw_tokens[1..] {
                let mut matches = expand_glob(tok, &self.cwd);
                if matches.is_empty() { expanded.push(tok.clone()); }
                else { matches.sort(); expanded.extend(matches); }
            }
            expanded
        };
        let result = self.dispatch_command(&tokens);
        apply_redirect(result, &redirect)
    }

    /// Run a command treating `stdin_data` as the piped input.
    /// For filter-style builtins (grep, cat), the input is processed directly.
    /// For other commands, stdin is ignored and the command runs normally.
    fn run_command_with_stdin(&mut self, line: &str, stdin_data: &str) -> Result<String> {
        let tokens = tokenize_command(line.trim())?;
        if tokens.is_empty() { return Ok(String::new()); }
        match tokens[0].as_str() {
            "grep" => {
                let pattern = tokens.get(1).map(|s| s.as_str()).unwrap_or("");
                if pattern.is_empty() {
                    return Err(anyhow!("grep: missing pattern"));
                }
                let matched: String = stdin_data
                    .lines()
                    .filter(|line| line.contains(pattern))
                    .collect::<Vec<_>>()
                    .join("\n");
                Ok(matched)
            }
            "cat" => Ok(stdin_data.to_string()),
            "wc" => {
                let flag = tokens.get(1).map(|s| s.as_str()).unwrap_or("-l");
                let count = match flag {
                    "-l" | "--lines" => stdin_data.lines().count(),
                    "-w" | "--words" => stdin_data.split_whitespace().count(),
                    "-c" | "--bytes" => stdin_data.len(),
                    _ => stdin_data.lines().count(),
                };
                Ok(count.to_string())
            }
            "sort" => {
                let mut lines: Vec<&str> = stdin_data.lines().collect();
                lines.sort_unstable();
                Ok(lines.join("\n"))
            }
            "uniq" => {
                let mut result: Vec<&str> = Vec::new();
                let mut last = "";
                for line in stdin_data.lines() {
                    if line != last { result.push(line); last = line; }
                }
                Ok(result.join("\n"))
            }
            "head" => {
                let n: usize = tokens.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                let lines: Vec<&str> = stdin_data.lines().take(n).collect();
                Ok(lines.join("\n"))
            }
            "tail" => {
                let n: usize = tokens.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
                let all: Vec<&str> = stdin_data.lines().collect();
                let start = all.len().saturating_sub(n);
                Ok(all[start..].join("\n"))
            }
            _ => self.run_command_single(line),
        }
    }

    /// Dispatch a pre-tokenized command, returning its output.
    fn dispatch_command(&mut self, tokens: &[String]) -> Result<String> {
        if tokens.is_empty() { return Ok(String::new()); }
        let cmd = tokens[0].as_str();
        let mut parts = tokens[1..].iter().map(|s| s.as_str());

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
                Ok(format_list(self.ls(arg)?))
            }
            "ls_long" => {
                let arg = parts.next();
                Ok(format_list(self.ls_long(arg)?))
            }
            "mkdir" => {
                let mut parents = false;
                let mut path = None;
                for p in parts {
                    if p == "-p" || p == "-P" { parents = true; }
                    else if path.is_none() { path = Some(p); }
                }
                self.mkdir(path.unwrap_or(""), parents)
            }
            "rm" => self.rm(parts.next().unwrap_or("")),
            "rmdir" => self.rmdir(parts.next().unwrap_or("")),
            "rmdir_r" | "rmdir_recursive" | "rm_r" => self.rmdir_recursive(parts.next().unwrap_or("")),
            "cat" => {
                let path = parts.next().unwrap_or("");
                let bytes = self.cat(path)?;
                String::from_utf8(bytes).map_err(|e| anyhow!("invalid utf8: {}", e))
            }
            "touch"     => self.touch(parts.next().unwrap_or("")),
            "cp"        => { let f = parts.next().unwrap_or(""); let t = parts.next().unwrap_or(""); self.cp(f, t) }
            "mv"        => { let f = parts.next().unwrap_or(""); let t = parts.next().unwrap_or(""); self.mv(f, t) }
            "exists"    => Ok(self.exists(parts.next().unwrap_or("")).to_string()),
            "is_file"   => Ok(self.is_file(parts.next().unwrap_or("")).to_string()),
            "is_dir"    => Ok(self.is_dir(parts.next().unwrap_or("")).to_string()),
            "file_size" => { let sz = self.file_size(parts.next().unwrap_or(""))?; Ok(sz.to_string()) }
            "realpath"  => self.realpath(parts.next().unwrap_or("")),
            "help"      => Ok("shell commands: pwd cd ls ls_long mkdir [-p] rm rmdir \
                               rmdir_r cat touch cp mv exists is_file is_dir file_size realpath \
                               echo grep wc sort uniq head tail find exit".to_string()),
            // Text processing builtins
            "echo"      => Ok(tokens[1..].join(" ")),
            "grep" => {
                let pattern = parts.next().unwrap_or("");
                let file    = parts.next().unwrap_or("");
                if pattern.is_empty() { return Err(anyhow!("grep: missing pattern")); }
                let content = if file.is_empty() {
                    return Err(anyhow!("grep: missing file (use pipe for stdin)"));
                } else {
                    let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.cwd.join(file) };
                    fs::read_to_string(&path).map_err(|e| anyhow!("grep: {}", e))?
                };
                let matched: Vec<&str> = content.lines().filter(|l| l.contains(pattern)).collect();
                Ok(matched.join("\n"))
            }
            "wc" => {
                let flag = parts.next().unwrap_or("-l");
                let file = parts.next().unwrap_or("");
                if file.is_empty() { return Err(anyhow!("wc: missing file")); }
                let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.cwd.join(file) };
                let content = fs::read_to_string(&path).map_err(|e| anyhow!("wc: {}", e))?;
                let count = match flag {
                    "-l" | "--lines" => content.lines().count(),
                    "-w" | "--words" => content.split_whitespace().count(),
                    "-c" | "--bytes" => content.len(),
                    _ => content.lines().count(),
                };
                Ok(count.to_string())
            }
            "sort" => {
                let file = parts.next().unwrap_or("");
                if file.is_empty() { return Err(anyhow!("sort: missing file")); }
                let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.cwd.join(file) };
                let content = fs::read_to_string(&path).map_err(|e| anyhow!("sort: {}", e))?;
                let mut lines: Vec<&str> = content.lines().collect();
                lines.sort_unstable();
                Ok(lines.join("\n"))
            }
            "uniq" => {
                let file = parts.next().unwrap_or("");
                if file.is_empty() { return Err(anyhow!("uniq: missing file")); }
                let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.cwd.join(file) };
                let content = fs::read_to_string(&path).map_err(|e| anyhow!("uniq: {}", e))?;
                let mut result: Vec<&str> = Vec::new();
                let mut last = "";
                for line in content.lines() { if line != last { result.push(line); last = line; } }
                Ok(result.join("\n"))
            }
            "head" => {
                let first_arg = parts.next().unwrap_or("");
                let (n, file) = if first_arg.starts_with('-') {
                    (first_arg[1..].parse::<usize>().unwrap_or(10), parts.next().unwrap_or(""))
                } else {
                    (10, first_arg)
                };
                if file.is_empty() { return Err(anyhow!("head: missing file")); }
                let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.cwd.join(file) };
                let content = fs::read_to_string(&path).map_err(|e| anyhow!("head: {}", e))?;
                Ok(content.lines().take(n).collect::<Vec<_>>().join("\n"))
            }
            "tail" => {
                let first_arg = parts.next().unwrap_or("");
                let (n, file) = if first_arg.starts_with('-') {
                    (first_arg[1..].parse::<usize>().unwrap_or(10), parts.next().unwrap_or(""))
                } else {
                    (10, first_arg)
                };
                if file.is_empty() { return Err(anyhow!("tail: missing file")); }
                let path = if Path::new(file).is_absolute() { PathBuf::from(file) } else { self.cwd.join(file) };
                let content = fs::read_to_string(&path).map_err(|e| anyhow!("tail: {}", e))?;
                let all: Vec<&str> = content.lines().collect();
                let start = all.len().saturating_sub(n);
                Ok(all[start..].join("\n"))
            }
            "find" => {
                let dir  = parts.next().unwrap_or(".");
                let name_pat = if parts.next() == Some("-name") { parts.next().unwrap_or("*") } else { "*" };
                let base = if Path::new(dir).is_absolute() { PathBuf::from(dir) } else { self.cwd.join(dir) };
                let mut found = Vec::new();
                find_recursive(&base, name_pat, &mut found);
                found.sort();
                Ok(found.join("\n"))
            }
            _ => Err(anyhow!("unknown shell command: {}", cmd)),
        }
    }

    /// Execute a simple shell command string, returning human-readable output.
    pub fn run_command(&mut self, line: &str) -> Result<String> {
        // Handle multi-stage pipes: cmd1 | cmd2 | cmd3
        let stages = split_pipe(line);
        if stages.len() > 1 {
            return self.run_pipeline(&stages);
        }
        self.run_command_single(line)
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

/// Split a command line on unquoted `|` characters into pipeline stages.
pub fn split_pipe(line: &str) -> Vec<String> {
    let mut stages: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;
    for ch in line.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"'  if !in_single => in_double = !in_double,
            '|'  if !in_single && !in_double => {
                stages.push(std::mem::take(&mut current));
            }
            other => current.push(other),
        }
    }
    if !current.trim().is_empty() {
        stages.push(current);
    }
    stages
}

/// Recursively find files matching a glob pattern under `base`.
fn find_recursive(base: &Path, pattern: &str, results: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(base) else { return; };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let path = entry.path();
        if glob_match(pattern, &name_str) {
            results.push(path.display().to_string());
        }
        if path.is_dir() {
            find_recursive(&path, pattern, results);
        }
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

    #[test]
    fn tokenize_basic() {
        let t = tokenize_command("ls -la /tmp").unwrap();
        assert_eq!(t, vec!["ls", "-la", "/tmp"]);
    }

    #[test]
    fn tokenize_double_quotes() {
        let t = tokenize_command(r#"cat "file with spaces.txt""#).unwrap();
        assert_eq!(t, vec!["cat", "file with spaces.txt"]);
    }

    #[test]
    fn tokenize_single_quotes() {
        let t = tokenize_command("echo 'hello world'").unwrap();
        assert_eq!(t, vec!["echo", "hello world"]);
    }

    #[test]
    fn tokenize_backslash_escape() {
        let t = tokenize_command(r"echo hello\ world").unwrap();
        assert_eq!(t, vec!["echo", "hello world"]);
    }

    #[test]
    fn tokenize_unterminated_quote() {
        assert!(tokenize_command(r#"echo "unclosed"#).is_err());
    }

    #[test]
    fn shell_run_command_cd_quoted_path() {
        let mut shell = Shell::new().unwrap();
        let result = shell.run_command("cd \"/tmp\"");
        assert!(result.is_ok());
        assert_eq!(shell.pwd(), "/tmp");
    }

    #[test]
    fn redirect_out_creates_file() {
        let mut shell = Shell::new().unwrap();
        let path = "/tmp/pasta_redirect_test.txt";
        let _ = fs::remove_file(path);
        let result = shell.run_command(&format!("pwd > {}", path));
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), ""); // redirected, no stdout
        let content = fs::read_to_string(path).unwrap();
        assert!(!content.is_empty());
        let _ = fs::remove_file(path);
    }

    #[test]
    fn redirect_append_appends() {
        let mut shell = Shell::new().unwrap();
        let path = "/tmp/pasta_redirect_append.txt";
        let _ = fs::remove_file(path);
        shell.run_command(&format!("pwd > {}", path)).unwrap();
        shell.run_command(&format!("pwd >> {}", path)).unwrap();
        let content = fs::read_to_string(path).unwrap();
        // File should have two lines
        assert!(content.lines().count() >= 2);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn redirect_parse_out() {
        let (cmd, _) = parse_redirect("pwd > /tmp/out.txt");
        assert_eq!(cmd, "pwd");
    }

    #[test]
    fn redirect_parse_append() {
        let (cmd, _) = parse_redirect("ls >> /tmp/log.txt");
        assert_eq!(cmd, "ls");
    }

    #[test]
    fn env_var_expansion_basic() {
        std::env::set_var("PASTA_TEST_VAR", "hello");
        let expanded = expand_env_vars("echo $PASTA_TEST_VAR");
        assert_eq!(expanded, "echo hello");
    }

    #[test]
    fn env_var_expansion_braces() {
        std::env::set_var("PASTA_TEST_VAR2", "world");
        let expanded = expand_env_vars("echo ${PASTA_TEST_VAR2}!");
        assert_eq!(expanded, "echo world!");
    }

    #[test]
    fn env_var_expansion_unset_is_empty() {
        let expanded = expand_env_vars("echo $PASTA_DEFINITELY_NOT_SET_XYZ");
        assert_eq!(expanded, "echo ");
    }

    #[test]
    fn shell_cd_uses_home_env() {
        std::env::set_var("HOME", "/tmp");
        let mut shell = Shell::new().unwrap();
        let result = shell.run_command("cd $HOME");
        assert!(result.is_ok());
        assert_eq!(shell.pwd(), "/tmp");
    }

    #[test]
    fn glob_match_star() {
        assert!(glob_match("*.txt", "hello.txt"));
        assert!(!glob_match("*.txt", "hello.rs"));
    }

    #[test]
    fn glob_match_question() {
        assert!(glob_match("?.txt", "a.txt"));
        assert!(!glob_match("?.txt", "ab.txt"));
    }

    #[test]
    fn glob_match_class() {
        assert!(glob_match("[abc].txt", "a.txt"));
        assert!(!glob_match("[abc].txt", "d.txt"));
    }

    #[test]
    fn glob_expand_tmp() {
        let cwd = PathBuf::from("/tmp");
        let _ = expand_glob("*", &cwd); // must not panic
    }

    #[test]
    fn shell_echo_builtin() {
        let mut shell = Shell::new().unwrap();
        let out = shell.run_command("echo hello world").unwrap();
        assert_eq!(out, "hello world");
    }

    #[test]
    fn shell_pipe_echo_grep() {
        let mut shell = Shell::new().unwrap();
        // Create a temp file with known content
        let path = "/tmp/pasta_pipe_test.txt";
        fs::write(path, "apple\nbanana\napricot\ncherry\n").unwrap();
        // cat file | grep a  →  lines containing 'a'
        let out = shell.run_command(&format!("cat {} | grep a", path)).unwrap();
        assert!(out.contains("apple"));
        assert!(out.contains("banana"));
        assert!(out.contains("apricot"));
        assert!(!out.contains("cherry"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn shell_pipe_three_stages() {
        let mut shell = Shell::new().unwrap();
        let path = "/tmp/pasta_pipe3_test.txt";
        fs::write(path, "foo\nbar\nfoo\nbaz\n").unwrap();
        // cat | sort | uniq
        let out = shell.run_command(&format!("cat {} | sort | uniq", path)).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert!(lines.contains(&"bar"));
        assert!(lines.contains(&"baz"));
        assert!(lines.contains(&"foo"));
        assert_eq!(lines.iter().filter(|&&l| l == "foo").count(), 1); // deduped
        let _ = fs::remove_file(path);
    }

    #[test]
    fn split_pipe_basic() {
        let s = split_pipe("ls | grep txt | wc -l");
        assert_eq!(s.len(), 3);
    }

    #[test]
    fn split_pipe_quoted_pipe_not_split() {
        let s = split_pipe(r#"echo "a|b""#);
        assert_eq!(s.len(), 1);
    }
}
