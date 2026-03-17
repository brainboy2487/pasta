// src/utils/errors.rs
//! Centralized error types and helpers for the PASTA project.
//!
//! This module defines a compact, ergonomic error type used across the codebase,
//! plus convenient conversions from common error sources. The goal is to have a
//! single canonical error type that is easy to match on in tests and callers,
//! while still carrying human-friendly messages and optional source errors.
//!
//! Design notes:
//! - We avoid external dependencies for the error type to keep the crate lightweight.
//! - For richer diagnostics in higher layers you can still use `anyhow::Error`
//!   or wrap this error type as needed.
//! - Use `Result<T>` alias exported here for convenience.

use std::fmt;
use std::io;
use std::error::Error as StdError;

/// The canonical error kind for the project.
///
/// Keep this enum broad but descriptive; add variants as new subsystems require
/// more specific handling.
#[derive(Debug)]
pub enum ErrorKind {
    /// I/O related errors (file, network, pipes).
    Io,
    /// Parsing or lexing errors.
    Parse,
    /// Semantic or runtime errors (constraints, evaluation).
    Runtime,
    /// Requested item not found.
    NotFound,
    /// Invalid input or user error.
    InvalidInput,
    /// Unsupported operation on this platform or configuration.
    Unsupported,
    /// External dependency or system error (e.g., device probe failed).
    External,
    /// Constraint engine related errors.
    Constraint,
    /// Scheduler related errors.
    Scheduler,
    /// Device auto-configuration errors.
    Device,
    /// ASM sandbox/runtime errors.
    Asm,
    /// Bitwise/low-level operation errors.
    Bitwise,
    /// RNG/hardware random errors.
    Rng,
    /// Generic catch-all.
    Other,
}

/// Primary error type used across the codebase.
///
/// It contains an `ErrorKind`, a human-readable message, and an optional boxed
/// source error for chaining.
#[derive(Debug)]
pub struct Error {
    pub kind: ErrorKind,
    pub message: String,
    pub source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    /// Create a new error with kind and message.
    pub fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            source: None,
        }
    }

    /// Create a new error with a source error for chaining.
    pub fn with_source<E>(kind: ErrorKind, message: impl Into<String>, src: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self {
            kind,
            message: message.into(),
            source: Some(Box::new(src)),
        }
    }

    /// Convenience: create an Io error wrapper.
    pub fn io<E>(message: impl Into<String>, src: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::with_source(ErrorKind::Io, message, src)
    }

    /// Convenience: create a Parse error wrapper.
    pub fn parse<E>(message: impl Into<String>, src: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        Self::with_source(ErrorKind::Parse, message, src)
    }

    /// Convert to a boxed std::error::Error for interoperability.
    pub fn into_boxed(self) -> Box<dyn StdError + Send + Sync> {
        if let Some(src) = self.source {
            // Wrap message and source in a simple wrapper type
            Box::new(self)
        } else {
            Box::new(self)
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(src) = &self.source {
            write!(f, "{}: {} (source: {})", format_kind(&self.kind), self.message, src)
        } else {
            write!(f, "{}: {}", format_kind(&self.kind), self.message)
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.source.as_ref().map(|b| &**b as &(dyn StdError + 'static))
    }
}

fn format_kind(k: &ErrorKind) -> &'static str {
    match k {
        ErrorKind::Io => "IoError",
        ErrorKind::Parse => "ParseError",
        ErrorKind::Runtime => "RuntimeError",
        ErrorKind::NotFound => "NotFound",
        ErrorKind::InvalidInput => "InvalidInput",
        ErrorKind::Unsupported => "Unsupported",
        ErrorKind::External => "External",
        ErrorKind::Constraint => "ConstraintError",
        ErrorKind::Scheduler => "SchedulerError",
        ErrorKind::Device => "DeviceError",
        ErrorKind::Asm => "AsmError",
        ErrorKind::Bitwise => "BitwiseError",
        ErrorKind::Rng => "RngError",
        ErrorKind::Other => "Error",
    }
}

/// Common `Result` alias using the crate's `Error`.
pub type Result<T> = std::result::Result<T, Error>;

// -----------------------------
// Conversions from common types
// -----------------------------

impl From<io::Error> for Error {
    fn from(e: io::Error) -> Self {
        Error::with_source(ErrorKind::Io, format!("I/O error: {}", e), e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::with_source(ErrorKind::Parse, format!("JSON parse error: {}", e), e)
    }
}

impl From<anyhow::Error> for Error {
    fn from(e: anyhow::Error) -> Self {
        // Try to downcast to known error types if possible
        if let Some(ioe) = e.downcast_ref::<io::Error>() {
            return Error::with_source(ErrorKind::Io, format!("{}", e), ioe.to_owned());
        }
        Error::with_source(ErrorKind::Other, format!("{}", e), e)
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Error::new(ErrorKind::Other, s)
    }
}

impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Error::new(ErrorKind::Other, s.to_string())
    }
}

// -----------------------------
// Small helper macros
// -----------------------------

/// Bail out with an error (like `anyhow::bail!`).
#[macro_export]
macro_rules! bail {
    ($kind:expr, $($arg:tt)*) => {
        return Err($crate::utils::errors::Error::new($kind, format!($($arg)*)));
    };
}

/// Create an error and return it (like `anyhow::ensure!`).
#[macro_export]
macro_rules! ensure {
    ($cond:expr, $kind:expr, $($arg:tt)*) => {
        if !($cond) {
            return Err($crate::utils::errors::Error::new($kind, format!($($arg)*)));
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_display_error() {
        let e = Error::new(ErrorKind::InvalidInput, "bad value");
        let s = format!("{}", e);
        assert!(s.contains("InvalidInput") || s.contains("bad value"));
    }

    #[test]
    fn from_io_error() {
        let ioe = io::Error::new(io::ErrorKind::Other, "disk");
        let e: Error = ioe.into();
        assert!(matches!(e.kind, ErrorKind::Io));
    }

    #[test]
    fn bail_macro_works() {
        fn f() -> Result<()> {
            bail!(ErrorKind::NotFound, "missing key {}", "x");
        }
        let r = f();
        assert!(r.is_err());
        let e = r.err().unwrap();
        assert!(matches!(e.kind, ErrorKind::NotFound));
    }

    #[test]
    fn ensure_macro_works() {
        fn f(ok: bool) -> Result<()> {
            ensure!(ok, ErrorKind::InvalidInput, "not ok");
            Ok(())
        }
        assert!(f(true).is_ok());
        assert!(f(false).is_err());
    }
}
