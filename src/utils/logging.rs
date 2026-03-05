// src/utils/logging.rs
//! Lightweight logging utilities for PASTA
//!
//! This module provides a small, dependency-free logging facility intended for
//! use inside the library and tests. It is not a replacement for `log`/`env_logger`
//! in large applications, but it is intentionally simple, thread-safe, and easy
//! to configure at runtime.
//!
//! Features:
//! - Global logger configured via `init_logger` (console and optional file).
//! - Log levels: Error, Warn, Info, Debug, Trace.
//! - Macros: `pasta_error!`, `pasta_warn!`, `pasta_info!`, `pasta_debug!`, `pasta_trace!`.
//! - Simple timestamped formatting and optional file output.
//!
//! Usage:
//! ```rust
//! use crate::utils::logging::*;
//! init_logger(LogLevel::Info, None).unwrap();
//! pasta_info!("starting up: {}", 42);
//! ```

use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Log level ordering (higher = more verbose).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
    Trace = 5,
}

impl LogLevel {
    /// Parse from a case-insensitive string like "info" or "DEBUG".
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Some(LogLevel::Error),
            "warn" | "warning" => Some(LogLevel::Warn),
            "info" => Some(LogLevel::Info),
            "debug" => Some(LogLevel::Debug),
            "trace" => Some(LogLevel::Trace),
            _ => None,
        }
    }
}

/// Internal logger state.
struct Logger {
    level: Mutex<LogLevel>,
    file: Mutex<Option<File>>,
    /// If true, also echo to stderr (useful for CLI).
    echo_stderr: bool,
}

impl Logger {
    fn new(level: LogLevel, file: Option<File>, echo_stderr: bool) -> Self {
        Self {
            level: Mutex::new(level),
            file: Mutex::new(file),
            echo_stderr,
        }
    }

    fn should_log(&self, lvl: LogLevel) -> bool {
        let current = *self.level.lock().unwrap();
        lvl <= current
    }

    fn log(&self, lvl: LogLevel, target: &str, msg: &str) {
        if !self.should_log(lvl) {
            return;
        }
        let ts = timestamp_now();
        let lvl_str = match lvl {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN ",
            LogLevel::Info => "INFO ",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        };
        let line = format!("{} [{}] {} - {}\n", ts, lvl_str, target, msg);

        // Write to file if configured
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                // Best-effort write; ignore errors to avoid panics in logging
                let _ = f.write_all(line.as_bytes());
                let _ = f.flush();
            }
        }

        // Always write to stdout; errors ignored
        if self.echo_stderr {
            let _ = io::stderr().write_all(line.as_bytes());
        } else {
            let _ = io::stdout().write_all(line.as_bytes());
        }
    }

    fn set_level(&self, lvl: LogLevel) {
        if let Ok(mut guard) = self.level.lock() {
            *guard = lvl;
        }
    }

    fn set_file(&self, file: Option<File>) {
        if let Ok(mut guard) = self.file.lock() {
            *guard = file;
        }
    }
}

/// Global logger instance (initialized via `init_logger`).
static GLOBAL_LOGGER: OnceLock<Logger> = OnceLock::new();

/// Initialize the global logger.
///
/// - `level`: initial log level.
/// - `file_path`: optional path to append log output to. If `None`, no file is used.
/// - `echo_stderr`: if true, log lines are written to stderr instead of stdout.
///
/// Calling `init_logger` more than once will return the already-initialized logger.
pub fn init_logger(level: LogLevel, file_path: Option<&str>, echo_stderr: bool) -> io::Result<()> {
    // If already initialized, update configuration instead of reinitializing.
    if let Some(logger) = GLOBAL_LOGGER.get() {
        logger.set_level(level);
        if let Some(path) = file_path {
            let f = OpenOptions::new().create(true).append(true).open(path)?;
            logger.set_file(Some(f));
        }
        return Ok(());
    }

    let file = if let Some(path) = file_path {
        Some(OpenOptions::new().create(true).append(true).open(path)?)
    } else {
        None
    };

    let logger = Logger::new(level, file, echo_stderr);
    GLOBAL_LOGGER
        .set(logger)
        .map_err(|_| io::Error::new(io::ErrorKind::Other, "logger already initialized"))?;
    Ok(())
}

/// Internal helper to get a reference to the global logger.
///
/// If the logger has not been initialized, a default logger is created with
/// `Info` level and no file, echoing to stdout.
fn get_logger() -> &'static Logger {
    GLOBAL_LOGGER.get_or_init(|| Logger::new(LogLevel::Info, None, false))
}

/// Set the global log level at runtime.
pub fn set_level(level: LogLevel) {
    let logger = get_logger();
    logger.set_level(level);
}

/// Convenience: set log level from string (ignores invalid values).
pub fn set_level_from_str(s: &str) {
    if let Some(l) = LogLevel::from_str(s) {
        set_level(l);
    }
}

/// Log a message at the given level and target.
pub fn log(level: LogLevel, target: &str, msg: &str) {
    let logger = get_logger();
    logger.log(level, target, msg);
}

/// Format current system time as `YYYY-MM-DDTHH:MM:SS.mmmZ` (UTC-ish).
fn timestamp_now() -> String {
    let now = SystemTime::now();
    let dur = now.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = dur.as_secs();
    let millis = dur.subsec_millis();
    // Simple UTC-like formatting without external crates
    // Note: this is seconds since epoch; for human-friendly date we would need chrono.
    format!("{}.{:03}", secs, millis)
}

// -----------------------------
// Logging macros
// -----------------------------
// Macros provide a convenient, printf-style interface and capture module path as target.

/// Log an error message.
#[macro_export]
macro_rules! pasta_error {
    ($($arg:tt)*) => {{
        let target = module_path!();
        $crate::utils::logging::log($crate::utils::logging::LogLevel::Error, target, &format!($($arg)*));
    }};
}

/// Log a warning message.
#[macro_export]
macro_rules! pasta_warn {
    ($($arg:tt)*) => {{
        let target = module_path!();
        $crate::utils::logging::log($crate::utils::logging::LogLevel::Warn, target, &format!($($arg)*));
    }};
}

/// Log an info message.
#[macro_export]
macro_rules! pasta_info {
    ($($arg:tt)*) => {{
        let target = module_path!();
        $crate::utils::logging::log($crate::utils::logging::LogLevel::Info, target, &format!($($arg)*));
    }};
}

/// Log a debug message.
#[macro_export]
macro_rules! pasta_debug {
    ($($arg:tt)*) => {{
        let target = module_path!();
        $crate::utils::logging::log($crate::utils::logging::LogLevel::Debug, target, &format!($($arg)*));
    }};
}

/// Log a trace message.
#[macro_export]
macro_rules! pasta_trace {
    ($($arg:tt)*) => {{
        let target = module_path!();
        $crate::utils::logging::log($crate::utils::logging::LogLevel::Trace, target, &format!($($arg)*));
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::io::Read;

    #[test]
    fn default_logger_works() {
        // Ensure default logger exists and Info level logs are emitted.
        let _ = get_logger();
        pasta_info!("test default logger {}", 1);
        // No assertions here; just ensure no panic.
    }

    #[test]
    fn init_logger_file_and_level() {
        let mut tmp = std::env::temp_dir();
        tmp.push("pasta_logging_test.log");
        let _ = fs::remove_file(&tmp);

        init_logger(LogLevel::Debug, Some(tmp.to_str().unwrap()), false).unwrap();
        pasta_debug!("debug message {}", 42);
        pasta_info!("info message");

        // Give the OS a moment to flush
        std::thread::sleep(std::time::Duration::from_millis(10));

        // Read file and check contents
        let mut f = File::open(&tmp).expect("log file should exist");
        let mut s = String::new();
        f.read_to_string(&mut s).unwrap();
        assert!(s.contains("debug message"));
        assert!(s.contains("info message"));

        // Clean up
        let _ = fs::remove_file(&tmp);
    }

    #[test]
    fn set_level_changes_behavior() {
        init_logger(LogLevel::Info, None, false).unwrap();
        set_level(LogLevel::Error);
        // This info message should be filtered out (no panic if called)
        pasta_info!("this should be filtered");
        // Error should still be logged
        pasta_error!("this is an error");
    }
}
