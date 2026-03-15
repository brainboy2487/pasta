// src/interpreter/ex_frame.rs
//
//! Frame and environment management for the PASTA executor.
//!
//! This module owns:
//!  - `CallFrame` / `FrameStack` — logical call-stack tracking used to build
//!    rich `Traceback` objects for error messages.
//!  - Thin wrappers around `Environment` scope push/pop helpers so that the
//!    borrow-checker friction of doing these operations deep inside `ex_eval`
//!    is reduced.
//!
//! `Executor` embeds a `Traceback` directly (preserved from the original
//! design) and calls the helpers in this module to maintain it.  Nothing here
//! owns the `Environment` — it remains on `Executor` so that all subsystems
//! share one root.

use crate::interpreter::errors::{TraceFrame, Traceback};
use crate::interpreter::environment::{Environment, Value};
use crate::parser::Span;

// ── Re-export the public int_api handle so ex_eval can use one name ──────────

pub use crate::interpreter::int_api::ModuleEnvHandle;

// ── Traceback helpers ─────────────────────────────────────────────────────────

/// Push a new frame onto a `Traceback`.
///
/// Called by the executor before entering any statement handler.
#[inline]
pub fn push_frame(tb: &mut Traceback, span: Span, ctx: impl Into<String>) {
    tb.0.push(TraceFrame { span, context: ctx.into() });
}

/// Pop the most-recent frame from a `Traceback`.
///
/// Silently does nothing if the stack is empty (can happen when an error is
/// returned before the matching pop is reached).
#[inline]
pub fn pop_frame(tb: &mut Traceback) {
    tb.0.pop();
}

// ── Scope helpers ─────────────────────────────────────────────────────────────

/// Push a new lexical scope onto `env` and return `Ok(())`.
///
/// Thin wrapper that keeps the call-site in `ex_eval` free of direct
/// `Environment` imports for this operation.
#[inline]
pub fn push_scope(env: &mut Environment) {
    env.push_scope();
}

/// Pop the innermost lexical scope from `env`, emitting a diagnostic string
/// on failure (scope underflow) instead of panicking.
#[inline]
pub fn pop_scope(env: &mut Environment, context_hint: &str, diags: &mut Vec<String>) {
    if let Err(e) = env.pop_scope() {
        diags.push(format!("Warning: pop_scope failed ({}): {}", context_hint, e));
    }
}

/// Bind `name` in the current (innermost) scope.
#[inline]
pub fn set_local(env: &mut Environment, name: impl Into<String>, val: Value) {
    env.set_local(name.into(), val);
}

/// Bind `name` in the global (outermost) scope.
#[inline]
pub fn set_global(env: &mut Environment, name: impl Into<String>, val: Value) {
    env.set_global(name.into(), val);
}

/// Look up `name` through the full scope chain.  Returns `None` if not found.
#[inline]
pub fn get(env: &Environment, name: &str) -> Option<Value> {
    env.get(name)
}

/// Write `val` to the scope that already owns `name`, searching outward.
/// If `name` is not found anywhere, it is created in the innermost scope.
#[inline]
pub fn assign(env: &mut Environment, name: &str, val: Value) {
    env.assign(name, val);
}
