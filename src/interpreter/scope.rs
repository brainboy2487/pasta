// src/interpreter/scope.rs
//!
//! Dedicated scope-management module for the PASTA interpreter.
//!
//! ## Design
//!
//! Every scope frame is tagged with a [`ScopeKind`]:
//! - **Global** — the root scope; variables live for the entire program.
//! - **Function** — a hard boundary (function/lambda call). New variables
//!   created inside stay inside and are discarded on return.
//! - **Block** — a soft boundary (IF body, loop body, DO body). New variables
//!   escape to the nearest Function-or-Global scope so they remain visible
//!   after the block ends.
//!
//! ## Assignment semantics (`scope_assign`)
//!
//! 1. If the name already exists in ANY scope → update it in-place.
//! 2. If the name is new → walk up from the innermost scope and find the first
//!    scope whose kind is `Function` or `Global`. Create the variable there.
//!
//! This means:
//! - Loop/IF variables persist after the block exits (they land in the
//!   enclosing function/global scope).
//! - Function-local variables are properly isolated from callers.
//! - No more `scopes.len() > 1` heuristic.

use crate::interpreter::environment::{Environment, ScopeKind, Value};
use crate::interpreter::errors::Traceback;
use crate::interpreter::errors::{ambient_push, ambient_pop};
use crate::interpreter::errors::TraceFrame;
use crate::parser::Span;

// ── Scope enter / leave ───────────────────────────────────────────────────────

/// Push a new scope frame of the given kind.
///
/// Use [`ScopeKind::Function`] for function/lambda calls and
/// [`ScopeKind::Block`] for IF bodies, loop bodies, and DO bodies.
#[inline]
pub fn enter(env: &mut Environment, kind: ScopeKind) {
    env.push_scope(kind);
}

/// Pop the innermost scope frame.
///
/// On underflow (attempted pop of the global scope) a diagnostic warning is
/// appended to `diags` instead of panicking.
#[inline]
pub fn leave(env: &mut Environment, context_hint: &str, diags: &mut Vec<String>) {
    if let Err(e) = env.pop_scope() {
        diags.push(format!("Warning: scope leave failed ({}): {}", context_hint, e));
    }
}

// ── Variable assignment ───────────────────────────────────────────────────────

/// Scope-aware variable assignment.
///
/// Rules:
/// 1. If `name` already exists in any scope → update it there.
/// 2. If `name` is new → create it in the nearest `Function` or `Global` scope,
///    skipping over any intervening `Block` scopes.
///
/// This ensures that variables assigned inside IF/loop bodies are visible
/// outside them (they land in the enclosing function/global scope), while
/// function-local variables remain isolated from their callers.
pub fn scope_assign(env: &mut Environment, name: &str, val: Value) -> Result<(), String> {
    // Guard: consts cannot be reassigned — throw a runtime error.
    if env.is_const(name) {
        return Err(format!("cannot reassign CONST '{}'", name));
    }

    // Rule 1 – update existing binding, but do NOT cross a Function boundary.
    // This prevents a plain DEF function from mutating a caller's variable of
    // the same name.  (Use GLOB.DEF to define a function that may write globals.)
    for scope in env.scopes.iter_mut().rev() {
        if scope.get(name).is_some() {
            scope.set(name, val);
            return Ok(());
        }
        // Stop searching once we hit a Function frame — don't peek into the
        // enclosing call frame or global scope.
        if matches!(scope.kind, ScopeKind::Function) {
            break;
        }
    }

    // Rule 2 – new variable: find nearest Function or Global scope.
    let target_idx = env.scopes.iter().enumerate().rev()
        .find(|(_, s)| matches!(s.kind, ScopeKind::Function | ScopeKind::Global))
        .map(|(i, _)| i)
        .unwrap_or(0);

    env.scopes[target_idx].set(name.to_string(), val);
    Ok(())
}

// ── Traceback helpers (re-exported here so callers only need one import) ──────

/// Push a call-stack frame onto a `Traceback` (and its thread-local mirror).
#[inline]
pub fn push_frame(tb: &mut Traceback, span: Span, ctx: impl Into<String>) {
    let ctx_s = ctx.into();
    tb.0.push(TraceFrame { span: span.clone(), context: ctx_s.clone() });
    ambient_push(span, ctx_s);
}

/// Pop the most-recent frame from a `Traceback` (silent no-op if empty).
#[inline]
pub fn pop_frame(tb: &mut Traceback) {
    tb.0.pop();
    ambient_pop();
}

// ── Convenience variable accessors ───────────────────────────────────────────

/// Look up `name` through the full scope chain. Returns `None` if not found.
#[inline]
pub fn get(env: &Environment, name: &str) -> Option<Value> {
    env.get(name)
}

/// Bind `name` unconditionally in the global (index-0) scope.
#[inline]
pub fn set_global(env: &mut Environment, name: impl Into<String>, val: Value) {
    env.set_global(name, val);
}

/// Bind `name` unconditionally in the innermost scope.
///
/// Prefer `scope_assign` for normal variable assignment. Use this only when
/// you explicitly want a local binding (e.g. function parameters, loop aliases).
#[inline]
pub fn set_local(env: &mut Environment, name: impl Into<String>, val: Value) {
    env.set_local(name, val);
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_var_in_block_escapes_to_global() {
        let mut env = Environment::new();
        let mut diags = Vec::new();

        // Simulate entering an IF body (Block scope).
        enter(&mut env, ScopeKind::Block);

        // Assign a brand-new variable — should land in the Global scope.
        let _ = scope_assign(&mut env, "x", Value::Number(42.0));

        // Pop the block.
        leave(&mut env, "test block", &mut diags);
        assert!(diags.is_empty());

        // x must still be visible at global scope.
        assert_eq!(env.get("x"), Some(Value::Number(42.0)));
    }

    #[test]
    fn new_var_in_function_stays_local() {
        let mut env = Environment::new();
        let mut diags = Vec::new();

        // Simulate entering a function call (Function scope).
        enter(&mut env, ScopeKind::Function);

        // Assign a brand-new variable — should stay in the Function scope.
        let _ = scope_assign(&mut env, "y", Value::Number(7.0));

        // Variable is visible while inside.
        assert_eq!(env.get("y"), Some(Value::Number(7.0)));

        // Pop the function scope.
        leave(&mut env, "test fn", &mut diags);
        assert!(diags.is_empty());

        // y must NOT be visible outside the function.
        assert_eq!(env.get("y"), None);
    }

    #[test]
    fn update_existing_var_from_inside_block() {
        let mut env = Environment::new();
        let mut diags = Vec::new();

        // Define x at global scope.
        env.set_global("x", Value::Number(0.0));

        // Enter a block (IF body).
        enter(&mut env, ScopeKind::Block);

        // Update x — should update the global binding, not create a shadowing copy.
        let _ = scope_assign(&mut env, "x", Value::Number(99.0));

        leave(&mut env, "block", &mut diags);

        // Global x must reflect the update.
        assert_eq!(env.get("x"), Some(Value::Number(99.0)));
    }

    #[test]
    fn nested_blocks_escape_to_function_scope() {
        let mut env = Environment::new();
        let mut diags = Vec::new();

        // Function scope.
        enter(&mut env, ScopeKind::Function);
        // Nested IF inside the function.
        enter(&mut env, ScopeKind::Block);

        // Brand-new variable inside the IF — should land in the Function scope.
        let _ = scope_assign(&mut env, "z", Value::Number(3.14));

        leave(&mut env, "inner block", &mut diags);

        // z must still be visible inside the function scope.
        assert_eq!(env.get("z"), Some(Value::Number(3.14)));

        leave(&mut env, "function", &mut diags);

        // z must NOT be visible outside the function.
        assert_eq!(env.get("z"), None);
    }

    #[test]
    fn leave_on_empty_scope_emits_diagnostic() {
        let mut env = Environment::new();
        let mut diags = Vec::new();
        leave(&mut env, "underflow test", &mut diags);
        assert_eq!(diags.len(), 1);
        assert!(diags[0].contains("Warning"));
    }
}
