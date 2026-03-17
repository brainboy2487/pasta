//! Centralized error definitions and reporting utilities for the Pasta interpreter.
//!
//! Error codes are grouped by subsystem:
//!   E0xxx — Lexer / tokenizer
//!   E1xxx — Parser / syntax
//!   E2xxx — Runtime / evaluation
//!   E3xxx — Type system
//!   E4xxx — Graphics subsystem
//!   E5xxx — I/O and filesystem
//!   E6xxx — Networking
//!   E7xxx — Concurrency / threading
//!   E8xxx — AI / tensor subsystem
//!   E9xxx — Internal / assertion failures

use std::fmt;
use crate::parser::parser::ParseError;
use crate::parser::Span;

// ─────────────────────────────────────────────────────────────────────────────
// Verbosity / debug level
// ─────────────────────────────────────────────────────────────────────────────

/// Global debug verbosity.  Set via `PASTA_DEBUG=<level>` env var or at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DebugLevel {
    /// Silent — only fatal errors shown.
    Silent  = 0,
    /// Normal — errors and warnings.
    Normal  = 1,
    /// Verbose — includes hints and notes.
    Verbose = 2,
    /// Trace — full execution trace (statement-level).
    Trace   = 3,
    /// Spam — token-level and expression-level detail.
    Spam    = 4,
}

impl DebugLevel {
    pub fn from_env() -> Self {
        match std::env::var("PASTA_DEBUG").as_deref() {
            Ok("0") | Ok("silent") => Self::Silent,
            Ok("1") | Ok("normal") => Self::Normal,
            Ok("2") | Ok("verbose") => Self::Verbose,
            Ok("3") | Ok("trace")   => Self::Trace,
            Ok("4") | Ok("spam")    => Self::Spam,
            _ => Self::Normal,
        }
    }
    pub fn is_verbose(self) -> bool { self >= Self::Verbose }
    pub fn is_trace(self)   -> bool { self >= Self::Trace }
    pub fn is_spam(self)    -> bool { self >= Self::Spam }
}

// ─────────────────────────────────────────────────────────────────────────────
// ANSI color helpers (no external deps)
// ─────────────────────────────────────────────────────────────────────────────

pub fn use_color() -> bool {
    std::env::var("NO_COLOR").is_err()
        && std::env::var("PASTA_NO_COLOR").is_err()
        && atty_stderr()
}

fn atty_stderr() -> bool {
    // Simple isatty check without libc dep
    #[cfg(unix)]
    unsafe {
        extern "C" { fn isatty(fd: i32) -> i32; }
        isatty(2) != 0
    }
    #[cfg(not(unix))]
    { false }
}

pub fn red(s: &str)     -> String { if use_color() { format!("\x1b[31m{}\x1b[0m", s) } else { s.to_string() } }
pub fn yellow(s: &str)  -> String { if use_color() { format!("\x1b[33m{}\x1b[0m", s) } else { s.to_string() } }
pub fn cyan(s: &str)    -> String { if use_color() { format!("\x1b[36m{}\x1b[0m", s) } else { s.to_string() } }
pub fn bold(s: &str)    -> String { if use_color() { format!("\x1b[1m{}\x1b[0m",  s) } else { s.to_string() } }
pub fn dimmed(s: &str)  -> String { if use_color() { format!("\x1b[2m{}\x1b[0m",  s) } else { s.to_string() } }
pub fn green(s: &str)   -> String { if use_color() { format!("\x1b[32m{}\x1b[0m", s) } else { s.to_string() } }
pub fn magenta(s: &str) -> String { if use_color() { format!("\x1b[35m{}\x1b[0m", s) } else { s.to_string() } }

// ─────────────────────────────────────────────────────────────────────────────
// Traceback
// ─────────────────────────────────────────────────────────────────────────────

/// A single frame in a traceback.
#[derive(Debug, Clone)]
pub struct TraceFrame {
    pub span:    Span,
    pub context: String,
}

/// A traceback is a vector of frames, printed newest-to-oldest.
#[derive(Debug, Clone, Default)]
pub struct Traceback(pub Vec<TraceFrame>);

impl Traceback {
    pub fn push(&mut self, span: Span, ctx: impl Into<String>) {
        self.0.push(TraceFrame { span, context: ctx.into() });
    }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

impl fmt::Display for Traceback {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for frame in self.0.iter().rev() {
            writeln!(f, "  {} {}:{}  {}",
                dimmed("at"),
                frame.span.start_line, frame.span.start_col,
                dimmed(&frame.context))?;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RuntimeErrorKind — full enumeration
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum RuntimeErrorKind {
    // ── E2xxx: Runtime / evaluation ─────────────────────────────────────────
    /// E2001: Variable referenced before assignment.
    UndefinedVariable(String),
    /// E2002: Function called that has not been defined.
    UndefinedFunction(String),
    /// E2003: Wrong number of arguments to a function call.
    ArityMismatch { name: String, expected: usize, got: usize },
    /// E2004: Division or modulo by zero.
    DivisionByZero,
    /// E2005: Stack overflow (recursion too deep).
    StackOverflow { depth: usize },
    /// E2006: While/do loop exceeded iteration limit.
    LoopLimitExceeded { limit: usize },
    /// E2007: Explicit runtime assertion failed.
    AssertionFailed { message: String },
    /// E2008: Return used outside of a function.
    ReturnOutsideFunction,
    /// E2009: Break/continue outside of a loop.
    BreakOutsideLoop,
    /// E2010: Value is not callable.
    NotCallable { got: String },
    /// E2011: Attempted to iterate over a non-iterable value.
    NotIterable { got: String },
    /// E2012: Negative or invalid repeat count.
    InvalidRepeatCount { got: f64 },
    /// E2013: Object field does not exist.
    FieldNotFound { object: String, field: String },
    /// E2014: Object mutation not permitted.
    MutationNotAllowed { object: String, field: String },
    /// E2015: Module or import not found.
    ModuleNotFound { name: String },
    /// E2016: Symbol not exported from module.
    SymbolNotExported { module: String, symbol: String },
    /// E2017: Heap reference is dangling (GC collected it).
    DanglingHeapRef { id: u64 },
    /// E2018: Numeric overflow.
    NumericOverflow { op: String },
    /// E2019: Numeric underflow.
    NumericUnderflow { op: String },
    /// E2020: Invalid numeric literal.
    InvalidNumericLiteral { raw: String },
    /// E2021: Thread with the given name or id not found.
    ThreadNotFound { id: String },
    /// E2022: Deadlock detected.
    Deadlock { context: String },
    /// E2023: Timeout waiting for a value.
    Timeout { ms: u64 },
    /// E2024: Scope stack underflow (pop on empty stack).
    ScopeUnderflow,
    /// E2025: Priority constraint violated.
    ConstraintViolation { message: String },

    // ── E3xxx: Type system ───────────────────────────────────────────────────
    /// E3001: Wrong type for an operation.
    TypeMismatch { expected: String, found: String },
    /// E3002: Operand types are incompatible.
    IncompatibleTypes { left: String, right: String, op: String },
    /// E3003: Cannot coerce value to target type.
    CoercionFailed { from: String, to: String },
    /// E3004: List index out of bounds.
    IndexOutOfBounds { index: isize, len: usize },
    /// E3005: Key not found in map/object.
    KeyNotFound { key: String },
    /// E3006: Slice range is invalid.
    InvalidSlice { start: isize, end: isize, len: usize },
    /// E3007: Tensor shape mismatch.
    TensorShapeMismatch { left: String, right: String },
    /// E3008: Tensor dtype mismatch.
    TensorDtypeMismatch { left: String, right: String },
    /// E3009: Cannot compare values of these types.
    UncomparableTypes { left: String, right: String },

    // ── E4xxx: Graphics subsystem ────────────────────────────────────────────
    /// E4001: Window creation failed.
    WindowCreateFailed { title: String, reason: String },
    /// E4002: Canvas creation failed.
    CanvasFailed { width: usize, height: usize, reason: String },
    /// E4003: Unknown window or canvas handle.
    UnknownHandle { handle: String },
    /// E4004: Pixel coordinates out of bounds.
    PixelOutOfBounds { x: isize, y: isize, width: usize, height: usize },
    /// E4005: Blit dimension mismatch.
    BlitDimensionMismatch { win_w: usize, win_h: usize, canvas_w: usize, canvas_h: usize },
    /// E4006: Cannot save framebuffer to path.
    FramebufferSaveFailed { path: String, reason: String },
    /// E4007: X11 display connection failed.
    X11ConnectFailed { reason: String },
    /// E4008: X11 window creation failed.
    X11WindowFailed { reason: String },
    /// E4009: Graphics operation on closed window.
    WindowClosed { handle: String },
    /// E4010: Invalid color component value.
    InvalidColorComponent { component: String, value: f64 },
    /// E4011: Unsupported pixel format.
    UnsupportedPixelFormat { format: String },
    /// E4012: Backend not available (e.g. x11 feature not compiled in).
    BackendUnavailable { backend: String },

    // ── E5xxx: I/O and filesystem ────────────────────────────────────────────
    /// E5001: File not found.
    FileNotFound { path: String },
    /// E5002: Permission denied.
    PermissionDenied { path: String },
    /// E5003: File read failed.
    ReadFailed { path: String, reason: String },
    /// E5004: File write failed.
    WriteFailed { path: String, reason: String },
    /// E5005: Path is invalid or malformed.
    InvalidPath { path: String },
    /// E5006: Unexpected end of file.
    UnexpectedEof { path: String },

    // ── E7xxx: Concurrency / threading ───────────────────────────────────────
    /// E7001: Cannot spawn more threads.
    ThreadLimitReached { limit: usize },
    /// E7002: Thread panicked.
    ThreadPanicked { id: u64, message: String },
    /// E7003: Channel send failed.
    ChannelSendFailed { reason: String },
    /// E7004: Channel receive failed.
    ChannelRecvFailed { reason: String },

    // ── E8xxx: AI / tensor ────────────────────────────────────────────────────
    /// E8001: Model not found or not loaded.
    ModelNotFound { name: String },
    /// E8002: Training failed.
    TrainingFailed { reason: String },
    /// E8003: Inference failed.
    InferenceFailed { reason: String },
    /// E8004: Invalid tensor operation.
    InvalidTensorOp { op: String, reason: String },

    // ── E9xxx: Internal ───────────────────────────────────────────────────────
    /// E9001: Internal assertion failed (bug in the interpreter).
    InternalAssertion { message: String },
    /// E9002: Unimplemented feature.
    Unimplemented { feature: String },
    /// E9003: Unreachable code reached.
    Unreachable { location: String },

    // ── Catch-all ─────────────────────────────────────────────────────────────
    /// Generic error with a message.
    Other { message: String },

    // Legacy variants kept for compatibility
    SyntaxError,
}

impl RuntimeErrorKind {
    /// Returns the numeric error code string (e.g. "E2001").
    pub fn code(&self) -> &'static str {
        match self {
            Self::UndefinedVariable(_)          => "E2001",
            Self::UndefinedFunction(_)          => "E2002",
            Self::ArityMismatch { .. }           => "E2003",
            Self::DivisionByZero                => "E2004",
            Self::StackOverflow { .. }           => "E2005",
            Self::LoopLimitExceeded { .. }       => "E2006",
            Self::AssertionFailed { .. }         => "E2007",
            Self::ReturnOutsideFunction         => "E2008",
            Self::BreakOutsideLoop              => "E2009",
            Self::NotCallable { .. }             => "E2010",
            Self::NotIterable { .. }             => "E2011",
            Self::InvalidRepeatCount { .. }      => "E2012",
            Self::FieldNotFound { .. }           => "E2013",
            Self::MutationNotAllowed { .. }      => "E2014",
            Self::ModuleNotFound { .. }          => "E2015",
            Self::SymbolNotExported { .. }       => "E2016",
            Self::DanglingHeapRef { .. }         => "E2017",
            Self::NumericOverflow { .. }         => "E2018",
            Self::NumericUnderflow { .. }        => "E2019",
            Self::InvalidNumericLiteral { .. }   => "E2020",
            Self::ThreadNotFound { .. }          => "E2021",
            Self::Deadlock { .. }                => "E2022",
            Self::Timeout { .. }                 => "E2023",
            Self::ScopeUnderflow                => "E2024",
            Self::ConstraintViolation { .. }     => "E2025",
            Self::TypeMismatch { .. }            => "E3001",
            Self::IncompatibleTypes { .. }       => "E3002",
            Self::CoercionFailed { .. }          => "E3003",
            Self::IndexOutOfBounds { .. }        => "E3004",
            Self::KeyNotFound { .. }             => "E3005",
            Self::InvalidSlice { .. }            => "E3006",
            Self::TensorShapeMismatch { .. }     => "E3007",
            Self::TensorDtypeMismatch { .. }     => "E3008",
            Self::UncomparableTypes { .. }       => "E3009",
            Self::WindowCreateFailed { .. }      => "E4001",
            Self::CanvasFailed { .. }            => "E4002",
            Self::UnknownHandle { .. }           => "E4003",
            Self::PixelOutOfBounds { .. }        => "E4004",
            Self::BlitDimensionMismatch { .. }   => "E4005",
            Self::FramebufferSaveFailed { .. }   => "E4006",
            Self::X11ConnectFailed { .. }        => "E4007",
            Self::X11WindowFailed { .. }         => "E4008",
            Self::WindowClosed { .. }            => "E4009",
            Self::InvalidColorComponent { .. }   => "E4010",
            Self::UnsupportedPixelFormat { .. }  => "E4011",
            Self::BackendUnavailable { .. }      => "E4012",
            Self::FileNotFound { .. }            => "E5001",
            Self::PermissionDenied { .. }        => "E5002",
            Self::ReadFailed { .. }              => "E5003",
            Self::WriteFailed { .. }             => "E5004",
            Self::InvalidPath { .. }             => "E5005",
            Self::UnexpectedEof { .. }           => "E5006",
            Self::ThreadLimitReached { .. }      => "E7001",
            Self::ThreadPanicked { .. }          => "E7002",
            Self::ChannelSendFailed { .. }       => "E7003",
            Self::ChannelRecvFailed { .. }       => "E7004",
            Self::ModelNotFound { .. }           => "E8001",
            Self::TrainingFailed { .. }          => "E8002",
            Self::InferenceFailed { .. }         => "E8003",
            Self::InvalidTensorOp { .. }         => "E8004",
            Self::InternalAssertion { .. }       => "E9001",
            Self::Unimplemented { .. }           => "E9002",
            Self::Unreachable { .. }             => "E9003",
            Self::SyntaxError                   => "E1000",
            Self::Other { .. }                   => "E0000",
        }
    }

    /// Generate the human-readable message for this error kind.
    pub fn message(&self) -> String {
        match self {
            Self::UndefinedVariable(n)  => format!("undefined variable '{}'", n),
            Self::UndefinedFunction(n)  => format!("undefined function '{}'", n),
            Self::ArityMismatch { name, expected, got }
                => format!("'{}' expects {} argument(s), got {}", name, expected, got),
            Self::DivisionByZero        => "division by zero".to_string(),
            Self::StackOverflow { depth }
                => format!("stack overflow at depth {}", depth),
            Self::LoopLimitExceeded { limit }
                => format!("loop exceeded limit of {} iterations — use set_while_limit() to raise it", limit),
            Self::AssertionFailed { message } => format!("assertion failed: {}", message),
            Self::ReturnOutsideFunction => "RETURN used outside of a function".to_string(),
            Self::BreakOutsideLoop      => "break/continue used outside of a loop".to_string(),
            Self::NotCallable { got }   => format!("value of type '{}' is not callable", got),
            Self::NotIterable { got }   => format!("value of type '{}' is not iterable", got),
            Self::InvalidRepeatCount { got }
                => format!("repeat count must be a non-negative integer, got {}", got),
            Self::FieldNotFound { object, field }
                => format!("object '{}' has no field '{}'", object, field),
            Self::MutationNotAllowed { object, field }
                => format!("field '{}' on '{}' is not mutable", field, object),
            Self::ModuleNotFound { name }
                => format!("module '{}' not found", name),
            Self::SymbolNotExported { module, symbol }
                => format!("'{}' does not export '{}'", module, symbol),
            Self::DanglingHeapRef { id }
                => format!("heap reference {} is dangling (garbage collected)", id),
            Self::NumericOverflow { op }
                => format!("numeric overflow in '{}'", op),
            Self::NumericUnderflow { op }
                => format!("numeric underflow in '{}'", op),
            Self::InvalidNumericLiteral { raw }
                => format!("invalid numeric literal '{}'", raw),
            Self::ThreadNotFound { id }
                => format!("thread '{}' not found", id),
            Self::Deadlock { context }
                => format!("deadlock detected: {}", context),
            Self::Timeout { ms }
                => format!("timed out after {}ms", ms),
            Self::ScopeUnderflow        => "scope stack underflow (interpreter bug)".to_string(),
            Self::ConstraintViolation { message }
                => format!("constraint violation: {}", message),
            Self::TypeMismatch { expected, found }
                => format!("type mismatch: expected '{}', found '{}'", expected, found),
            Self::IncompatibleTypes { left, right, op }
                => format!("cannot apply '{}' to '{}' and '{}'", op, left, right),
            Self::CoercionFailed { from, to }
                => format!("cannot coerce '{}' to '{}'", from, to),
            Self::IndexOutOfBounds { index, len }
                => format!("index {} out of bounds for collection of length {}", index, len),
            Self::KeyNotFound { key }
                => format!("key '{}' not found", key),
            Self::InvalidSlice { start, end, len }
                => format!("slice [{}..{}] is invalid for length {}", start, end, len),
            Self::TensorShapeMismatch { left, right }
                => format!("tensor shape mismatch: {} vs {}", left, right),
            Self::TensorDtypeMismatch { left, right }
                => format!("tensor dtype mismatch: {} vs {}", left, right),
            Self::UncomparableTypes { left, right }
                => format!("cannot compare '{}' and '{}'", left, right),
            Self::WindowCreateFailed { title, reason }
                => format!("failed to create window '{}': {}", title, reason),
            Self::CanvasFailed { width, height, reason }
                => format!("failed to create {}x{} canvas: {}", width, height, reason),
            Self::UnknownHandle { handle }
                => format!("unknown graphics handle '{}'", handle),
            Self::PixelOutOfBounds { x, y, width, height }
                => format!("pixel ({},{}) out of bounds for {}x{} canvas", x, y, width, height),
            Self::BlitDimensionMismatch { win_w, win_h, canvas_w, canvas_h }
                => format!("BLIT: window {}x{} != canvas {}x{}", win_w, win_h, canvas_w, canvas_h),
            Self::FramebufferSaveFailed { path, reason }
                => format!("cannot save framebuffer to '{}': {}", path, reason),
            Self::X11ConnectFailed { reason }
                => format!("X11 connection failed: {} (is DISPLAY set?)", reason),
            Self::X11WindowFailed { reason }
                => format!("X11 window creation failed: {}", reason),
            Self::WindowClosed { handle }
                => format!("graphics operation on closed window '{}'", handle),
            Self::InvalidColorComponent { component, value }
                => format!("color component '{}' value {} is out of range [0,255]", component, value),
            Self::UnsupportedPixelFormat { format }
                => format!("unsupported pixel format '{}'", format),
            Self::BackendUnavailable { backend }
                => format!("graphics backend '{}' is not available (recompile with --features {})", backend, backend),
            Self::FileNotFound { path }
                => format!("file not found: '{}'", path),
            Self::PermissionDenied { path }
                => format!("permission denied: '{}'", path),
            Self::ReadFailed { path, reason }
                => format!("read failed on '{}': {}", path, reason),
            Self::WriteFailed { path, reason }
                => format!("write failed on '{}': {}", path, reason),
            Self::InvalidPath { path }
                => format!("invalid path: '{}'", path),
            Self::UnexpectedEof { path }
                => format!("unexpected end of file: '{}'", path),
            Self::ThreadLimitReached { limit }
                => format!("cannot spawn more threads: limit of {} reached", limit),
            Self::ThreadPanicked { id, message }
                => format!("thread {} panicked: {}", id, message),
            Self::ChannelSendFailed { reason }
                => format!("channel send failed: {}", reason),
            Self::ChannelRecvFailed { reason }
                => format!("channel receive failed: {}", reason),
            Self::ModelNotFound { name }
                => format!("model '{}' not found", name),
            Self::TrainingFailed { reason }
                => format!("training failed: {}", reason),
            Self::InferenceFailed { reason }
                => format!("inference failed: {}", reason),
            Self::InvalidTensorOp { op, reason }
                => format!("invalid tensor op '{}': {}", op, reason),
            Self::InternalAssertion { message }
                => format!("internal error (please report): {}", message),
            Self::Unimplemented { feature }
                => format!("'{}' is not yet implemented", feature),
            Self::Unreachable { location }
                => format!("reached unreachable code at {}", location),
            Self::SyntaxError               => "syntax error".to_string(),
            Self::Other { message }         => message.clone(),
        }
    }

    /// Returns a hint string to help the user fix the error (if available).
    pub fn hint(&self) -> Option<&'static str> {
        match self {
            Self::UndefinedVariable(_)      => Some("check spelling and ensure the variable is assigned before use"),
            Self::UndefinedFunction(_)      => Some("define the function with DEF before calling it"),
            Self::DivisionByZero            => Some("guard with an IF check before dividing"),
            Self::LoopLimitExceeded { .. }  => Some("ensure your loop has a reachable exit condition"),
            Self::NotCallable { .. }        => Some("only DEF functions and lambdas can be called"),
            Self::IndexOutOfBounds { .. }   => Some("check list length before indexing"),
            Self::WindowCreateFailed { .. } => Some("ensure a display is available; try headless mode"),
            Self::X11ConnectFailed { .. }   => Some("set DISPLAY=:0 or run in a desktop session"),
            Self::BackendUnavailable { .. } => Some("rebuild with: cargo build --features x11"),
            Self::LoopLimitExceeded { .. }  => Some("use exec.set_while_limit(N) to raise the limit"),
            Self::ModuleNotFound { .. }     => Some("check that the .ph file exists in stdlib/ or headers/"),
            Self::TypeMismatch { .. }       => Some("use TYPEOF() to inspect value types at runtime"),
            Self::StackOverflow { .. }      => Some("check for unbounded recursion"),
            _                               => None,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// RuntimeError
// ─────────────────────────────────────────────────────────────────────────────

/// A runtime error produced by the interpreter.
#[derive(Debug, Clone)]
pub struct RuntimeError {
    pub kind:      RuntimeErrorKind,
    pub span:      Span,
    pub message:   String,
    pub traceback: Traceback,
    /// Source file name, if known.
    pub file:      Option<String>,
    /// The source line text, for inline display.
    pub source_line: Option<String>,
}

impl RuntimeError {
    pub fn new(kind: RuntimeErrorKind, span: Span) -> Self {
        let message = kind.message();
        Self { kind, span, message, traceback: Traceback::default(), file: None, source_line: None }
    }

    pub fn with_traceback(mut self, tb: Traceback) -> Self {
        self.traceback = tb; self
    }

    pub fn with_file(mut self, file: impl Into<String>) -> Self {
        self.file = Some(file.into()); self
    }

    pub fn with_source_line(mut self, line: impl Into<String>) -> Self {
        self.source_line = Some(line.into()); self
    }

    /// Format a Rust-compiler-style diagnostic.
    pub fn pretty(&self) -> String {
        let mut out = String::new();
        let code = self.kind.code();
        let file = self.file.as_deref().unwrap_or("<script>");

        // error[E2001]: undefined variable 'x'
        out.push_str(&format!("{} {}: {}\n",
            red(&bold(&format!("error[{}]", code))),
            bold(&self.message),
            dimmed("")
        ));
        // --> file:line:col
        out.push_str(&format!("  {} {}:{}:{}\n",
            cyan("-->"),
            file,
            self.span.start_line,
            self.span.start_col
        ));
        // source line + caret
        if let Some(ref src) = self.source_line {
            let line_no = format!("{}", self.span.start_line);
            out.push_str(&format!("  {} {}\n", dimmed(&format!("{} |", line_no)), src));
            let pad = " ".repeat(self.span.start_col.saturating_sub(1));
            let caret = "^".repeat((self.span.end_col.saturating_sub(self.span.start_col)).max(1));
            out.push_str(&format!("  {} {}{}\n",
                dimmed(&format!("{} |", " ".repeat(line_no.len()))),
                pad,
                red(&caret)
            ));
        }
        // hint
        if let Some(hint) = self.kind.hint() {
            out.push_str(&format!("  {} {}\n", yellow("hint:"), hint));
        }
        // traceback
        if !self.traceback.is_empty() {
            out.push_str(&format!("{}\n{}", dimmed("traceback:"), self.traceback));
        }
        out
    }
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if std::env::var("PASTA_PRETTY").is_ok() {
            write!(f, "{}", self.pretty())
        } else {
            write!(f, "error[{}]: {} at {}:{}",
                self.kind.code(), self.message,
                self.span.start_line, self.span.start_col)?;
            if !self.traceback.is_empty() {
                write!(f, "\n{}", self.traceback)?;
            }
            Ok(())
        }
    }
}

impl std::error::Error for RuntimeError {}

// ─────────────────────────────────────────────────────────────────────────────
// PastaError — unified top-level error
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum PastaError {
    Syntax(ParseError),
    Runtime(RuntimeError),
}

impl fmt::Display for PastaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PastaError::Syntax(e)  => write!(f, "error[E1000]: syntax error: {} at {}:{}",
                e.message, e.span.start_line, e.span.start_col),
            PastaError::Runtime(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for PastaError {}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic — a warning or note (non-fatal)
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticLevel { Warning, Note, Hint }

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level:   DiagnosticLevel,
    pub code:    String,
    pub message: String,
    pub span:    Span,
}

impl Diagnostic {
    pub fn warning(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self { level: DiagnosticLevel::Warning, code: code.into(), message: message.into(), span }
    }
    pub fn note(code: impl Into<String>, message: impl Into<String>, span: Span) -> Self {
        Self { level: DiagnosticLevel::Note, code: code.into(), message: message.into(), span }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.level {
            DiagnosticLevel::Warning => yellow(&bold("warning")),
            DiagnosticLevel::Note    => cyan(&bold("note")),
            DiagnosticLevel::Hint    => green(&bold("hint")),
        };
        write!(f, "{}[{}]: {} at {}:{}", prefix, self.code, self.message,
            self.span.start_line, self.span.start_col)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Message constants
// ─────────────────────────────────────────────────────────────────────────────

pub mod messages {
    // Lexer
    pub const UNTERMINATED_STRING:       &str = "unterminated string literal";
    pub const INVALID_TOKEN:             &str = "invalid token";
    pub const INVALID_ESCAPE:            &str = "invalid escape sequence";
    pub const INVALID_UNICODE:           &str = "invalid unicode escape";
    pub const LEADING_ZERO:              &str = "leading zeros in numeric literal";
    pub const MISSING_OPERATOR:          &str = "missing operator between operands";

    // Parser
    pub const EXPECTED_IDENTIFIER:       &str = "expected identifier";
    pub const EXPECTED_IDENTIFIER_AFTER_DEF: &str = "expected identifier after DEF";
    pub const EXPECTED_DO_AFTER_DEF:     &str = "expected DO or ':' after function name";
    pub const EXPECTED_INDENT_AFTER_DEF: &str = "expected indented block after DEF";
    pub const EXPECTED_DEDENT_AFTER_DEF: &str = "expected DEDENT after function body";
    pub const EXPECTED_END_AFTER_DEF:    &str = "expected END to close function definition";
    pub const EXPECTED_COLON:            &str = "expected ':'";
    pub const EXPECTED_RPAREN:           &str = "expected ')'";
    pub const EXPECTED_RBRACKET:         &str = "expected ']'";
    pub const EXPECTED_RBRACE:           &str = "expected '}'";
    pub const EXPECTED_EQ:               &str = "expected '=' in assignment";
    pub const UNEXPECTED_EOF:            &str = "unexpected end of input";
    pub const UNEXPECTED_TOKEN:          &str = "unexpected token";
    pub const INVALID_ASSIGNMENT_TARGET: &str = "invalid assignment target";

    // Runtime
    pub const UNUSED_VARIABLE:           &str = "variable defined but never used";
    pub const TYPE_NOT_SUPPORTED:        &str = "type not supported for this operation";
    pub const SHADOWED_VARIABLE:         &str = "variable shadows an outer definition";
    pub const LOOP_VARIABLE_MODIFIED:    &str = "loop variable modified inside loop body";

    // Graphics
    pub const NO_DISPLAY:                &str = "no display available (DISPLAY not set)";
    pub const INVALID_DIMENSIONS:        &str = "width and height must be greater than zero";
    pub const HANDLE_ALREADY_CLOSED:     &str = "handle has already been closed";
}
