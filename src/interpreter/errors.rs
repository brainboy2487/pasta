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
use crate::parser::Span;

/// A single frame in a traceback; captures the location and an optional
/// human‑readable description of what was being executed.
#[derive(Debug, Clone)]
pub struct TraceFrame {
    /// Source location of this frame.
    pub span: Span,
    /// Human-readable description of the operation being executed.
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
    /// A variable was referenced before being defined.
    UndefinedVariable(String),
    /// A value of an unexpected type was encountered.
    TypeMismatch {
        /// The type that was expected.
        expected: String,
        /// The type that was actually found.
        found: String,
    },
    /// An integer or float division by zero was attempted.
    DivisionByZero,
    /// A list or tensor index was outside the valid range.
    IndexOutOfBounds {
        /// The index that was accessed.
        index: usize,
        /// The length of the collection.
        len: usize,
    },
    /// A dynamic syntax error produced at runtime (e.g. from eval).
    SyntaxError,
    // ... hundreds more would go here
}

/// A runtime error produced by the interpreter.  Includes a span, kind, and
/// optional human message (typically generated from the kind).
#[derive(Debug, Clone)]
pub struct RuntimeError {
    /// Structured error classification.
    pub kind: RuntimeErrorKind,
    /// Source location where the error was detected.
    pub span: Span,
    /// Human-readable error message derived from `kind`.
    pub message: String,
    /// Call-stack frames captured at the point of the error.
    pub traceback: Traceback,
}

impl RuntimeError {
    /// Construct a `RuntimeError` from a kind and span, auto-generating the message.
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
    /// A parse/lexer error encountered during compilation.
    Syntax(ParseError),
    /// An error raised during program execution.
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
/// Pre-defined error message string constants used by the lexer and parser.
///
/// Centralising strings here makes it easy to keep diagnostic output
/// consistent and to add localisation later.
pub mod messages {
    /// Emitted when a binary operator is absent between two operands.
    pub const MISSING_OPERATOR: &str = "missing operator";
    /// Emitted when a string literal is not closed before end-of-line.
    pub const UNTERMINATED_STRING: &str = "unterminated string literal";
    /// Emitted when an unrecognised character sequence is encountered.
    pub const INVALID_TOKEN: &str = "invalid token";
    /// Lint warning for variables that are assigned but never read.
    pub const UNUSED_VARIABLE: &str = "variable defined but never used";
    /// Emitted when an operator is applied to an unsupported type.
    pub const TYPE_NOT_SUPPORTED: &str = "type not supported for operation";
    // parser-specific helpers (shown here for example; a real catalog would
    // include hundreds of strings keyed by error code).
    /// Expected an identifier token immediately after the `DEF` keyword.
    pub const EXPECTED_IDENTIFIER_AFTER_DEF: &str = "expected identifier after DEF";
    /// Expected the `DO` keyword (or `:`) after a function's parameter list.
    pub const EXPECTED_DO_AFTER_DEF: &str = "expected DO after function name";
    /// Expected an indented body block after the `DEF` header.
    pub const EXPECTED_INDENT_AFTER_DEF: &str = "expected indented block after DEF";
    /// Expected a dedent token to close the function body.
    pub const EXPECTED_DEDENT_AFTER_DEF: &str = "expected dedent after function body";
    /// Expected the `END` keyword to explicitly close a function definition.
    pub const EXPECTED_END_AFTER_DEF: &str = "expected END to close function definition";
    // ...more templates would follow
}
