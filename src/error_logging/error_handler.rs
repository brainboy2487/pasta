// src/error_logging/error_handler.rs
//! Error handler that formats runtime errors using the message catalog.
//!
//! Intended usage:
//!   - When the executor detects a stack/recursion guard, call
//!       ErrorHandler::stack_guard_triggered(depth, span, snippet)
//!   - When constructing a RuntimeError, call
//!       ErrorHandler::format_error("ARITY_MISMATCH", &params)
//!
//! The handler returns a formatted string suitable for printing to stderr
//! and can optionally attach the existing Traceback object.

use crate::interpreter::errors::Traceback;
use crate::parser::Span;
use serde::Serialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::error_logging::error_messages::ErrorCatalog;
use chrono::Utc;

static CATALOG: OnceLock<ErrorCatalog> = OnceLock::new();

/// Lazily loaded process-global error catalog.
pub fn catalog() -> &'static ErrorCatalog {
    CATALOG.get_or_init(|| {
        crate::error_logging::error_messages::default_catalog()
            .expect("failed to load error_messages.json")
    })
}

/// Extra structured context persisted alongside a diagnostic bundle.
#[derive(Debug, Clone, Default, Serialize)]
pub struct DiagnosticContext {
    /// Symbolic catalog key, if the diagnostic came from the error catalog.
    pub error_key: Option<String>,
    /// Stable external error code, if known.
    pub error_code: Option<String>,
    /// Primary human-readable summary.
    pub summary: Option<String>,
    /// Longer explanation of the failure.
    pub explanation: Option<String>,
    /// Suggested next action for the user.
    pub suggestion: Option<String>,
    /// Interpreter traceback rendered as text.
    pub interpreter_traceback: Option<String>,
    /// Source span rendered as `line:col-line:col`.
    pub source_span: Option<String>,
    /// Source snippet or expression/statement fragment relevant to the error.
    pub source_snippet: Option<String>,
}

/// Format an error message by key and parameters, including explanation/suggestion.
pub fn format_error(key: &str, params: &HashMap<&str, String>) -> String {
    if let Some(entry) = catalog().get(key) {
        let mut msg = entry.message.clone();
        for (k, v) in params.iter() {
            let placeholder = format!("{{{}}}", k);
            msg = msg.replace(&placeholder, v);
        }
        let mut lines = vec![msg];
        if let Some(explanation) = &entry.explanation {
            lines.push(format!("explanation: {}", explanation));
        }
        if let Some(suggestion) = &entry.suggestion {
            lines.push(format!("suggestion: {}", suggestion));
        }
        lines.join("\n")
    } else {
        format!("[UNKNOWN] Unknown error key: {}.", key)
    }
}

/// Build structured diagnostic context from a catalog entry and local runtime data.
pub fn diagnostic_context(
    key: &str,
    params: &HashMap<&str, String>,
    span: Option<&Span>,
    snippet: Option<&str>,
    tb: Option<&Traceback>,
) -> DiagnosticContext {
    let mut ctx = DiagnosticContext {
        error_key: Some(key.to_string()),
        source_span: span.map(format_span),
        source_snippet: snippet.map(ToOwned::to_owned),
        interpreter_traceback: tb.filter(|tb| !tb.is_empty()).map(ToString::to_string),
        ..DiagnosticContext::default()
    };

    if let Some(entry) = catalog().get(key) {
        let mut message = entry.message.clone();
        for (k, v) in params {
            let placeholder = format!("{{{}}}", k);
            message = message.replace(&placeholder, v);
        }
        ctx.error_code = Some(entry.code.clone());
        ctx.summary = Some(message);
        ctx.explanation = entry.explanation.clone();
        ctx.suggestion = entry.suggestion.clone();
    } else {
        ctx.summary = Some(format!("Unknown error key: {}", key));
    }

    ctx
}

/// Called when a stack/recursion guard triggers.
pub fn stack_guard_triggered(
    depth: usize,
    span: Option<&Span>,
    expr_snippet: &str,
    tb: Option<&Traceback>,
) -> String {
    let mut params = HashMap::new();
    params.insert("depth", depth.to_string());
    params.insert("expr", expr_snippet.to_string());
    if let Some(span) = span {
        params.insert("span", format_span(span));
    }

    let mut out = format_error("STACK_OVERFLOW", &params);
    if let Some(tb) = tb {
        if !tb.is_empty() {
            out.push_str("\nTraceback:\n");
            out.push_str(&format!("{}", tb));
        }
    }
    out
}

/// Persisted diagnostic bundle for post-mortem debugging.
#[derive(Serialize)]
struct DiagnosticBundle {
    /// Timestamp of capture in RFC3339 format.
    ts: String,
    /// Process id that emitted the diagnostic.
    pid: u32,
    /// Thread name at the moment of capture.
    thread: String,
    /// Rust backtrace captured during failure.
    backtrace: String,
    /// Recent runtime event log entries, if available.
    recent_events: Vec<String>,
    /// Structured error context captured from the interpreter.
    context: DiagnosticContext,
}

fn format_span(span: &Span) -> String {
    format!(
        "{}:{}-{}:{}",
        span.start_line, span.start_col, span.end_line, span.end_col
    )
}

fn persist_diagnostic_to_dir(
    dir: &Path,
    backtrace: &str,
    recent_events: Vec<String>,
    context: DiagnosticContext,
) -> anyhow::Result<PathBuf> {
    let pid = std::process::id();
    let bundle = DiagnosticBundle {
        ts: Utc::now().to_rfc3339(),
        pid,
        thread: std::thread::current().name().unwrap_or("unnamed").to_string(),
        backtrace: backtrace.to_string(),
        recent_events,
        context,
    };
    let mut path = dir.to_path_buf();
    std::fs::create_dir_all(&path)?;
    let fname = format!("stack_overflow_{}_{}.json", bundle.ts.replace(':', "-"), pid);
    path.push(fname);
    let mut f = File::create(&path)?;
    let s = serde_json::to_string_pretty(&bundle)?;
    f.write_all(s.as_bytes())?;
    Ok(path)
}

/// Persist a diagnostic bundle to `tools/diagnostics`.
pub fn persist_diagnostic(
    backtrace: &str,
    recent_events: Vec<String>,
    context: DiagnosticContext,
) -> anyhow::Result<PathBuf> {
    persist_diagnostic_to_dir(Path::new("tools/diagnostics"), backtrace, recent_events, context)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn format_error_includes_explanation_and_suggestion() {
        let mut params = HashMap::new();
        params.insert("depth", "201".to_string());
        params.insert("expr", "call foo".to_string());

        let rendered = format_error("STACK_OVERFLOW", &params);
        assert!(rendered.contains("Stack overflow detected at depth 201."));
        assert!(rendered.contains("explanation:"));
        assert!(rendered.contains("suggestion:"));
    }

    #[test]
    fn persist_diagnostic_writes_structured_context() {
        let dir = tempdir().expect("temp dir should be created");
        let path = persist_diagnostic_to_dir(
            dir.path(),
            "bt",
            vec!["event-a".to_string()],
            DiagnosticContext {
                error_key: Some("STACK_OVERFLOW".to_string()),
                summary: Some("stack overflow".to_string()),
                interpreter_traceback: Some("Traceback:\n  at 1:1 call foo".to_string()),
                ..DiagnosticContext::default()
            },
        )
        .expect("diagnostic should persist");

        let text = std::fs::read_to_string(path).expect("diagnostic file should be readable");
        assert!(text.contains("\"error_key\": \"STACK_OVERFLOW\""));
        assert!(text.contains("\"summary\": \"stack overflow\""));
        assert!(text.contains("\"interpreter_traceback\": \"Traceback:\\n  at 1:1 call foo\""));
    }
}
