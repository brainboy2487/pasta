//! Centralized error definitions and reporting utilities for the Pasta
//! interpreter.  This module defines the various error kinds (syntax,
//! runtime, etc.), message templates, and a lightweight traceback facility
//! that can be attached to errors produced during execution.
//!
//! The goal is to move away from ad‑hoc `anyhow!` strings and instead have a
//! rich, enumerated set of errors with spans, codes, and automatically
//! formatted backtraces.  A set of message constants is provided below; in a
//! real system there would be hundreds of them, but for now we include a
//! representative sample and the infrastructure to add more.

use std::fmt;

use crate::parser::parser::ParseError;
use crate::parser::ast::Span;

/// A single frame in a traceback; captures the location and an optional
/// human‑readable description of what was being executed.
#[derive(Debug, Clone)]
pub struct TraceFrame {
    pub span: Span,
    pub context: String,
}

/// A traceback is simply a vector of frames, printed from newest to oldest.
#[derive(Debug, Clone, Default)]
pub struct Traceback(pub Vec<TraceFrame>);

impl fmt::Display for Traceback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for frame in &self.0 {
            writeln!(f, "  at {}:{}:{} -- {}", frame.span.start_line, frame.span.start_col, frame.span.end_line, frame.context)?;
        }
        Ok(())
    }
}

/// Error codes for runtime errors.  Each variant corresponds to a
/// well‑defined condition and has an associated format string template.
/// More codes can be added as new language features appear.
#[derive(Debug, Clone)]
pub enum RuntimeErrorKind {
    UndefinedVariable(String),
    TypeMismatch { expected: String, found: String },
    DivisionByZero,
    IndexOutOfBounds { index: usize, len: usize },
    SyntaxError, // used for dynamic syntax failures
    // ... hundreds more would go here
}

/// A runtime error produced by the interpreter.  Includes a span, kind, and
/// optional human message (typically generated from the kind).
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub kind: RuntimeErrorKind,
    pub span: Span,
    pub message: String,
    pub traceback: Traceback,
}

impl RuntimeError {
    pub fn new(kind: RuntimeErrorKind, span: Span) -> Self {
        let message = match &kind {
            RuntimeErrorKind::UndefinedVariable(name) => {
                format!("Undefined variable '{}'", name)
            }
            RuntimeErrorKind::TypeMismatch { expected, found } => {
                format!("Type mismatch: expected {}, found {}", expected, found)
            }
            RuntimeErrorKind::DivisionByZero => "Division by zero".to_string(),
            RuntimeErrorKind::IndexOutOfBounds { index, len } => {
                format!("Index {} out of bounds for length {}", index, len)
            }
            RuntimeErrorKind::SyntaxError => "Syntax error".to_string(),
        };
        Self {
            kind,
            span,
            message,
            traceback: Traceback::default(),
        }
    }

    /// Attach a traceback (e.g. from an executor) and return self.
    pub fn with_traceback(mut self, tb: Traceback) -> Self {
        self.traceback = tb;
        self
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Error: {} at {}:{}", self.message, self.span.start_line, self.span.start_col)?;
        if !self.traceback.0.is_empty() {
            writeln!(f, "Traceback:")?;
            write!(f, "{}", self.traceback)?;
        }
        Ok(())
    }
}

impl std::error::Error for RuntimeError {}

/// A unified error type carrying either a parse/syntax error or a runtime
/// error.  Convenience helpers make it easy to convert from the underlying
/// types.
#[derive(Debug, Clone)]
pub enum PastaError {
    Syntax(ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for PastaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PastaError::Syntax(e) => write!(f, "Syntax error: {} at {}:{}", e.message, e.span.start_line, e.span.start_col),
            PastaError::Runtime(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for PastaError {}

// A handful of predefined message templates; in a full system these might be
// generated from a table, localised, or assigned numeric codes.
pub mod messages {
    pub const MISSING_OPERATOR: &str = "missing operator";
    pub const UNTERMINATED_STRING: &str = "unterminated string literal";
    pub const INVALID_TOKEN: &str = "invalid token";
    pub const UNUSED_VARIABLE: &str = "variable defined but never used";
    pub const TYPE_NOT_SUPPORTED: &str = "type not supported for operation";
    // parser-specific helpers (shown here for example; a real catalog would
    // include hundreds of strings keyed by error code).
    pub const EXPECTED_IDENTIFIER_AFTER_DEF: &str = "expected identifier after DEF";
    pub const EXPECTED_DO_AFTER_DEF: &str = "expected DO after function name";
    pub const EXPECTED_INDENT_AFTER_DEF: &str = "expected indented block after DEF";
    pub const EXPECTED_DEDENT_AFTER_DEF: &str = "expected dedent after function body";
    pub const EXPECTED_END_AFTER_DEF: &str = "expected END to close function definition";
    // ...more templates would follow
}
