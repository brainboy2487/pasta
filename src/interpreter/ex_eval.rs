// src/interpreter/ex_eval.rs
//
//! Statement and expression evaluation for the PASTA executor.
//!
//! All logic that was previously in the giant `impl Executor` block of
//! `executor.rs` lives here, broken into two public entry-points:
//!
//!  * `eval_stmt` — dispatch a single `Statement` to its handler.
//!  * `eval_expr` — recursively evaluate an `Expr` and return a `Value`.
//!
//! Both functions take `&mut Executor` so they can reach every field
//! (environment, functions table, GC, diagnostics, traceback, etc.) without
//! duplicating state.  The split is purely organisational — the borrow graph
//! is unchanged from the monolithic file.

use anyhow::{anyhow, Result};

use crate::interpreter::executor::Executor;
use crate::interpreter::environment::{ScopeKind, Value};
use crate::interpreter::ex_frame;
use crate::interpreter::scope as scoped;
use crate::parser::*;
use crate::parser::ast::ScopeModifier;
use crate::semantics::{Relation, ExprSimple, ConstraintExpr};

use crate::error_logging::error_handler;
use std::cell::Cell;
use crate::interpreter::errors::{RuntimeError, RuntimeErrorKind};

// RECURSION_GUARD_ADDED

thread_local! {
    /// Current recursion depth for eval functions on this thread.
    static RECURSION_DEPTH: Cell<usize> = Cell::new(0);
}

/// Maximum allowed recursion depth before the guard triggers.
#[allow(dead_code)]
const RECURSION_LIMIT: usize = 1000;

/// RAII guard that increments the thread-local recursion depth on creation
/// and decrements it on drop.
pub(crate) struct RecursionGuard;
impl RecursionGuard {
    /// Try to enter a recursion frame. Returns Some(guard) if depth < limit,
    /// otherwise returns None to indicate the guard would trip.
    #[allow(dead_code)]
    fn enter() -> Option<Self> {
        RECURSION_DEPTH.with(|d| {
            let depth = d.get();
            if depth >= RECURSION_LIMIT {
                None
            } else {
                d.set(depth + 1);
                Some(RecursionGuard)
            }
        })
    }
}
impl Drop for RecursionGuard {
    fn drop(&mut self) {
        RECURSION_DEPTH.with(|d| {
            let cur = d.get();
            d.set(cur.saturating_sub(1));
        });
    }
}
/* STACK_OVERFLOW_SAFE_GUARD: non-duplicating recursion guard helper
   Reuses existing RecursionGuard and RECURSION_DEPTH.
*/
pub(crate) struct RecursionGuardTrip {
    pub depth: usize,
    pub backtrace: String,
    pub recent_events: Vec<String>,
}

pub(crate) fn enter_recursion_guard_with_threshold(
    threshold: usize,
) -> std::result::Result<RecursionGuard, RecursionGuardTrip> {
    // increment depth and get new value
    let depth = RECURSION_DEPTH.with(|d| {
        let cur = d.get();
        d.set(cur.saturating_add(1));
        cur.saturating_add(1)
    });

    if depth > threshold {
        // decrement immediately to avoid leaking the counter
        RECURSION_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));

        let bt = std::backtrace::Backtrace::force_capture();
        let bt_str = format!("{:?}", bt);
        let recent = take_recent_events();
        Err(RecursionGuardTrip {
            depth,
            backtrace: bt_str,
            recent_events: recent,
        })
    } else {
        Ok(RecursionGuard)
    }
}

// ── Internal formatting helper (mirrors the one in executor.rs) ───────────────

#[inline]
#[allow(dead_code)]
#[allow(dead_code)]
fn fmt_f64(n: f64) -> String {
    if n.fract().abs() < f64::EPSILON {
        format!("{}", n.round() as i64)
    } else {
        format!("{}", n)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry-points
// ─────────────────────────────────────────────────────────────────────────────

/// Dispatch a single AST statement, returning any yielded `Value`.
///
/// This is the primary statement execution loop, extracted verbatim from the
/// original `Executor::execute_statement`.  All internal helpers that it
/// calls (`eval_expr`, `call_builtin`, etc.) are defined later in this file
/// or remain on `Executor` as public / pub(crate) methods.
pub fn eval_stmt(exec: &mut Executor, stmt: &Statement) -> Result<Option<Value>> {
    let span = match stmt {
        Statement::Assignment { span, .. }
        | Statement::ConstAssignment { span, .. }
        | Statement::MultiAssignment { span, .. }
        | Statement::FunctionDef { span, .. }
        | Statement::DoBlock { span, .. }
        | Statement::WhileBlock { span, .. }
        | Statement::ForIn { span, .. }
        | Statement::PriorityOverride { span, .. }
        | Statement::Constraint { span, .. }
        | Statement::Print { span, .. }
        | Statement::If { span, .. }
        | Statement::End { span }
        | Statement::ExprStmt { span, .. }
        | Statement::ObjDecl { span, .. }
        | Statement::SpawnBlock { span, .. }
        | Statement::Break { span }
        | Statement::Continue { span }
        | Statement::Other { span, .. }
        // v1.4.4 pointer system
        | Statement::GotoLabel { span, .. }
        | Statement::GotoBlock { span, .. }
        | Statement::LoopBlock { span, .. }
        | Statement::Pull { span, .. }
        | Statement::Push { span, .. }
        | Statement::Alloc { span, .. }
        | Statement::Free { span, .. }
        | Statement::Info { span, .. }
        | Statement::Seek { span, .. }
        | Statement::Swap { span, .. } => span.clone(),

        Statement::UseUnsafe { span, .. } => span.clone(),
        Statement::DefDoUntil(d)         => d.span.clone(),
        Statement::RetNow { span, .. }   => span.clone(),
        Statement::RetLate { span, .. }  => span.clone(),
        Statement::AttemptBlock { span, .. } => span.clone(),
        Statement::TryBlock { span, .. } => span.clone(),
        Statement::ModuleDecl { span, .. } | Statement::FromBlock { span, .. } => span.clone(),
    };

    
    // Recursion guard: prevent stack overflow by returning a controlled error
    // when recursion depth exceeds RECURSION_LIMIT.
    let _rec_guard = match enter_recursion_guard_with_threshold(200) {
        Ok(g) => g,
        Err(trip) => {
            let snippet = format!("{:?}", stmt);
            let diag = error_handler::stack_guard_triggered(
                trip.depth,
                Some(&span),
                &snippet,
                Some(&exec.traceback),
            );
            let mut params = std::collections::HashMap::new();
            params.insert("depth", trip.depth.to_string());
            params.insert("expr", snippet.clone());
            params.insert(
                "span",
                format!(
                    "{}:{}-{}:{}",
                    span.start_line, span.start_col, span.end_line, span.end_col
                ),
            );
            let _ = error_handler::persist_diagnostic(
                &trip.backtrace,
                trip.recent_events,
                error_handler::diagnostic_context(
                    "STACK_OVERFLOW",
                    &params,
                    Some(&span),
                    Some(&snippet),
                    Some(&exec.traceback),
                ),
            );
            return Err(exec.span_err(&span, diag));
        }
    };




    // Trace frames are managed by the Executor wrapper to ensure a single
    // push/pop per dispatched statement.  Do not push here to avoid double
    // pushing frames when `execute_statement` performs the push.

    let stmt_kind = match stmt {
        _ => "Other",
    };
    let _ = stmt_kind;

    match stmt {
                // ── Assignment ────────────────────────────────────────────────────────
        Statement::Assignment { target, value, span: _ } => {
            let v = eval_expr(exec, value)?;
            let v_to_store = match v {
                crate::interpreter::environment::Value::List(_)
                | crate::interpreter::environment::Value::Dict(_) => {
                    let id = exec.gc.allocate(v);
                    crate::interpreter::environment::Value::Heap(id)
                }
                other => other,
            };
            scoped::scope_assign(&mut exec.env, &target.name, v_to_store).map_err(|e| anyhow::anyhow!(e))?;
            Ok(None)
        }

        // ── Constant declaration ──────────────────────────────────────────────
        Statement::ConstAssignment { target, value, span: _ } => {
            if exec.env.is_const(&target.name) {
                return Err(anyhow::anyhow!("cannot reassign constant '{}'", target.name));
            }
            let v = eval_expr(exec, value)?;
            let v_to_store = match v {
                crate::interpreter::environment::Value::List(_)
                | crate::interpreter::environment::Value::Dict(_) => {
                    let id = exec.gc.allocate(v);
                    crate::interpreter::environment::Value::Heap(id)
                }
                other => other,
            };
            scoped::scope_assign(&mut exec.env, &target.name, v_to_store).map_err(|e| anyhow::anyhow!(e))?;
            exec.env.mark_const(&target.name);
            Ok(None)
        }

        // ── Multi-label assignment ────────────────────────────────────────────
        Statement::MultiAssignment { targets, value, span: _ } => {
            let v = eval_expr(exec, value)?;
            let v_to_store = match v {
                crate::interpreter::environment::Value::List(_)
                | crate::interpreter::environment::Value::Dict(_) => {
                    let id = exec.gc.allocate(v);
                    crate::interpreter::environment::Value::Heap(id)
                }
                other => other,
            };
            for t in targets {
                scoped::scope_assign(&mut exec.env, &t.name, v_to_store.clone()).map_err(|e| anyhow::anyhow!(e))?;
            }
            Ok(None)
        }

        // ── Function definition ───────────────────────────────────────────────
        Statement::FunctionDef { name, params, body, span: _ } => {
            // Register in the global functions table (for named-call resolution).
            exec.functions.insert(name.name.clone(), (params.clone(), body.clone()));

            // Also bind a callable Value::Lambda into the current lexical scope so
            // the function can be passed as a value. If we're at top-level, bind
            // globally; otherwise bind in the current (innermost) scope.
            let lambda_val = Value::Lambda(params.clone(), body.clone(), exec.env.capture_scope());
            if exec.env.get_scopes().len() <= 1 {
                exec.env.set_global(name.name.clone(), lambda_val);
            } else {
                ex_frame::set_local(&mut exec.env, name.name.clone(), lambda_val);
            }

            Ok(None)
        }
        // Module declaration — creates a namespace with exported symbols
        Statement::ModuleDecl { name, exports, body, span: _ } => {
            // Create a new scope for the module body
            exec.env.push_scope(ScopeKind::Function); // module body

            // Execute all statements in the module body
            for stmt in body.iter() {
                // Use eval_stmt (the correct function name in this file)
                let _ = eval_stmt(exec, stmt)?;
            }

            // Collect exported symbols into a list of (name, value) pairs
            // We'll store this as a specially-formatted list
            let mut module_exports: Vec<Value> = Vec::new();
            
            for export_name in exports.iter() {
                if let Some(val) = exec.env.get(&export_name.name) {
                    // Store as [name_string, value] pairs in the list
                    module_exports.push(Value::List(vec![
                        Value::String(export_name.name.clone()),
                        val,
                    ]));
                } else {
                    ex_frame::pop_scope(&mut exec.env, "module loading error", &mut exec.diagnostics);
                    return Err(anyhow::anyhow!(
                        "Module '{}' exports '{}' but it is not defined",
                        name.name, export_name.name
                    ));
                }
            }

            // Pop the module scope
            ex_frame::pop_scope(&mut exec.env, "module loading cleanup", &mut exec.diagnostics);

            // Create a module value as a List containing exports
            // Format: [["export1", value1], ["export2", value2], ...]
            let module_val = Value::List(module_exports);

            // Bind the module name in the current scope
            if exec.env.get_scopes().len() <= 1 {
                exec.env.set_global(name.name.clone(), module_val);
            } else {
                ex_frame::set_local(&mut exec.env, name.name.clone(), module_val);
            }

            // Register the module path for USE/FROM resolution
            exec.module_registry.register(name.name.clone(), format!("inline:{}", name.name));

            Ok(None)
        }

        // FROM/USE import block — placeholder for lazy import registration
        Statement::FromBlock { imports, span: _ } => {
            // Register modules and create lazy bindings in the current scope.
            for group in imports.iter() {
                let module_name = group.module.name.clone();
                // Try to resolve path now and register; if resolution fails, still register name (lazy)
                match exec.resolve_module_path(&module_name) {
                    Ok(p) => exec.module_registry.register(module_name.clone(), p),
                    Err(_) => exec.module_registry.register(module_name.clone(), String::new()),
                }
                for u in group.uses.iter() {
                    let sym = u.name.name.clone();
                    let alias = u.alias.as_ref().map(|a| a.name.clone());
                    let bind_name = alias.clone().unwrap_or_else(|| sym.clone());
                    let li = Value::LazyImport { module: module_name.clone(), name: sym.clone(), alias: alias.clone() };
                    ex_frame::set_local(&mut exec.env, bind_name.clone(), li);
                }
            }
            Ok(None)
        }
// ── Counted DO block ──────────────────────────────────────────────────
        Statement::DoBlock { targets, alias, repeats, duration_ms, body, span } => {
            // ── Timed loop: `DO x FOR Nms END` ──────────────────────────────
            if let Some(dur_expr) = duration_ms {
                let ms = match eval_expr(exec, dur_expr)? {
                    Value::Number(n) => n as u64,
                    other => return Err(anyhow::anyhow!("DO...FOR <ms>: expected a number of milliseconds, got {:?}", other)),
                };
                let deadline = std::time::Instant::now() + std::time::Duration::from_millis(ms);

                if targets.is_empty() {
                    // Anonymous timed body loop
                    while std::time::Instant::now() < deadline {
                        for s in body.iter() {
                            exec.execute_statement(s)?;
                            if exec.control_flow.is_some() { break; }
                            let _ = exec.collect_garbage();
                        }
                        match exec.control_flow {
                            Some(crate::interpreter::executor::ControlFlowSignal::Break) => { exec.control_flow = None; break; }
                            Some(crate::interpreter::executor::ControlFlowSignal::Continue) => { exec.control_flow = None; }
                            Some(_) => break,
                            None => {}
                        }
                    }
                } else {
                    // Target dispatch timed loop: call each target repeatedly until deadline
                    for target_id in targets.iter() {
                        let fn_body: Option<Vec<Statement>> = exec.functions
                            .get(&target_id.name)
                            .map(|(_, b)| b.clone())
                            .or_else(|| exec.env.get(&target_id.name).and_then(|v| {
                                if let Value::Lambda(_, s, _) = exec.deref(v) { Some(s) } else { None }
                            }));
                        if let Some(stmts) = fn_body {
                            while std::time::Instant::now() < deadline {
                                ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                                if let Some(a) = alias {
                                    ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(target_id.name.clone()));
                                }
                                for s in stmts.iter() {
                                    exec.execute_statement(s)?;
                                    if exec.control_flow.is_some() { break; }
                                    let _ = exec.collect_garbage();
                                }
                                ex_frame::pop_scope(&mut exec.env, "DO timed target", &mut exec.diagnostics);
                                match exec.control_flow {
                                    Some(crate::interpreter::executor::ControlFlowSignal::Break) => { exec.control_flow = None; break; }
                                    Some(crate::interpreter::executor::ControlFlowSignal::Continue) => { exec.control_flow = None; }
                                    Some(_) => break,
                                    None => {}
                                }
                            }
                        }
                    }
                }
                return Ok(None);
            }
            let counts = exec.resolve_repeat_counts(targets.len(), repeats.as_ref(), span)?;

            for (i, target_id) in targets.iter().enumerate() {
                let repeat_count = counts[i];

                let resolved_target_val = exec
                    .env
                    .get(&target_id.name)
                    .map(|v| exec.deref(v));

                // Lambda stored globally
                let global_lambda: Option<Vec<Statement>> = match &resolved_target_val {
                    Some(Value::Lambda(_, s, _)) => Some(s.clone()),
                    _ => None,
                };

                if let Some(stmts) = global_lambda {
                    for _ in 0..repeat_count {
                        // Push a call-site frame so errors inside this stored
                        // lambda show the call origin in the traceback.
                        exec.push_frame(crate::parser::ast::Span::dummy(), format!("call {}", target_id.name));
                        ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                        if let Some(a) = alias {
                            ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(target_id.name.clone()));
                        }
                        for s in stmts.iter() {
                            exec.execute_statement(s)?;
                            if exec.control_flow.is_some() { break; }
                            let _ = exec.collect_garbage();
                        }
                        ex_frame::pop_scope(&mut exec.env, "DO lambda", &mut exec.diagnostics);
                        exec.pop_frame();
                    }
                    continue;
                }

                if let Some(Value::List(items)) = resolved_target_val.clone() {
                    for _ in 0..repeat_count {
                        for item in items.iter() {
                            exec.execute_value_as_callable(item, alias, &target_id.name)?;
                            let _ = exec.collect_garbage();
                        }
                    }
                    continue;
                }

                let fn_lkp: Option<(Vec<crate::parser::Identifier>, Vec<Statement>)> =
                    exec.functions.get(&target_id.name).cloned();
                if let Some((func_params, func_body)) = fn_lkp {
                    for _ in 0..repeat_count {
                        ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                        if let Some(a) = alias {
                            ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(target_id.name.clone()));
                        }
                        let _ = func_params;
                        let func_body: &Vec<Statement> = &func_body;
                        for s in func_body.iter() {
                            exec.execute_statement(s)?;
                            if exec.control_flow.is_some() { break; }
                            let _ = exec.collect_garbage();
                        }
                        ex_frame::pop_scope(&mut exec.env, "DO fn", &mut exec.diagnostics);
                    }
                    continue;
                }
                else {
                    // Fallback: check environment for a callable lambda value.
                    if let Some(env_val) = exec.env.get(&target_id.name) {
                        if let Value::Lambda(_, stmts, lambda_caps) = env_val {
                                exec.push_frame(crate::parser::ast::Span::dummy(), format!("call {}", target_id.name));
                                ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                            // Inject captures
                            for (k, v) in &lambda_caps {
                                ex_frame::set_local(&mut exec.env, k.clone(), v.clone());
                            }
                            let mut last = Value::None;
                            for s in stmts.iter() {
                                if let Some(v) = exec.execute_statement(s)? { last = v; }
                                if let Some(crate::interpreter::executor::ControlFlowSignal::Return(ret_val)) = exec.control_flow.take() {
                                    let ret_concrete = exec.deref_return(ret_val);
                                        ex_frame::pop_scope(&mut exec.env, "RET.NOW in env-lambda", &mut exec.diagnostics);
                                        exec.pop_frame();
                                    return Ok(Some(ret_concrete));
                                }
                                let _ = exec.collect_garbage();
                            }
                                ex_frame::pop_scope(&mut exec.env, "call env-lambda", &mut exec.diagnostics);
                                exec.pop_frame();
                            return Ok(Some(last));
                        }
                    }
                }


                // Treat as logical thread name
                let tid = exec.env.define_thread(Some(target_id.name.clone()), 1.0);
                for _ in 0..repeat_count {
                    ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                    if let Some(a) = alias {
                        ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(target_id.name.clone()));
                    }
                    ex_frame::set_local(&mut exec.env, "_thread_id", Value::Number(tid as f64));
                    for s in body.iter() {
                        exec.execute_statement(s)?;
                        if exec.control_flow.is_some() { break; }
                        let _ = exec.collect_garbage();
                    }
                    ex_frame::pop_scope(&mut exec.env, "DO thread body", &mut exec.diagnostics);
                }
                exec.env.remove_thread(tid);
            }

            Ok(None)
        }

        // ── WHILE loop ────────────────────────────────────────────────────────
        Statement::WhileBlock { targets, alias, condition, body, scope_modifier, span } => {
            let limit = if exec.while_limit == 0 { usize::MAX } else { exec.while_limit };

            if targets.is_empty() {
                if body.is_empty() {
                    ex_frame::pop_frame(&mut exec.traceback);
                    return Ok(None);
                }
                // Determine scope behavior:
                // - Default (None): no new scope pushed; variables survive the loop.
                // - UnbindScope: push Block scope; variables die at end.
                // - BindScope: push Block scope but hoist vars to function scope on exit.
                let push_block = scope_modifier.is_some();
                if push_block {
                    ex_frame::push_scope(&mut exec.env, ScopeKind::Block);
                }
                if let Some(a) = alias {
                    ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(String::new()));
                }
                let mut iterations: usize = 0;
                loop {
                    if iterations >= limit {
                        if push_block { ex_frame::pop_scope(&mut exec.env, "WHILE limit", &mut exec.diagnostics); }
                        return Err(exec.span_err(span, format!("WHILE loop exceeded iteration limit ({})", limit)));
                    }
                    let cond_val = eval_expr(exec, condition)?;
                    if !exec.value_is_truthy(&cond_val) { break; }
                    for s in body.iter() {
                        exec.execute_statement(s)?;
                        if exec.control_flow.is_some() { break; }
                        let _ = exec.collect_garbage();
                    }
                    // Break exits the while loop; Continue restarts from the condition check;
                    // Return/Killed must propagate upward.
                    match exec.control_flow {
                        Some(crate::interpreter::executor::ControlFlowSignal::Break) => {
                            exec.control_flow = None;
                            break;
                        }
                        Some(crate::interpreter::executor::ControlFlowSignal::Continue) => {
                            exec.control_flow = None;
                            // fall through to re-evaluate condition
                        }
                        Some(_) => break, // Return / Killed — propagate
                        None => {}
                    }
                    iterations += 1;
                }
                if push_block {
                    if scope_modifier.as_ref() == Some(&ScopeModifier::BindScope) {
                        exec.env.pop_scope_hoist().ok();
                    } else {
                        ex_frame::pop_scope(&mut exec.env, &format!("WHILE iter {}", iterations), &mut exec.diagnostics);
                    }
                }
                ex_frame::pop_frame(&mut exec.traceback);
                return Ok(None);
            }

            for target_id in targets.iter() {
                let lambda_stmts: Option<Vec<Statement>> = exec
                    .env
                    .get(&target_id.name)
                    .and_then(|v| if let Value::Lambda(_, s, _) = v { Some(s) } else { None })
                    .or_else(|| exec.functions.get(&target_id.name).map(|r| r.1.clone()));

                let tid = exec.env.define_thread(Some(target_id.name.clone()), 1.0);
                let mut iterations: usize = 0;

                if let Some(exec_body) = lambda_stmts {
                    loop {
                        if iterations >= limit {
                            return Err(exec.span_err(span, format!("WHILE loop for '{}' exceeded iteration limit ({})", target_id.name, limit)));
                        }
                        if let Some(a) = alias {
                            ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                            ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(target_id.name.clone()));
                            ex_frame::set_local(&mut exec.env, "_thread_id", Value::Number(tid as f64));
                        }
                        let exec_body: &Vec<Statement> = &exec_body;
                        for s in exec_body.iter() {
                            exec.execute_statement(s)?;
                            if exec.control_flow.is_some() { break; }
                            let _ = exec.collect_garbage();
                        }
                        if alias.is_some() {
                            ex_frame::pop_scope(&mut exec.env, "WHILE lambda", &mut exec.diagnostics);
                        }
                        if exec.control_flow.is_some() { break; }
                        iterations += 1;
                        let cond_val = eval_expr(exec, condition)?;
                        if !exec.value_is_truthy(&cond_val) { break; }
                    }
                } else {
                    loop {
                        if iterations >= limit {
                            return Err(exec.span_err(span, format!("WHILE loop for '{}' exceeded iteration limit ({})", target_id.name, limit)));
                        }
                        let cond_val = eval_expr(exec, condition)?;
                        if !exec.value_is_truthy(&cond_val) { break; }
                        if let Some(a) = alias {
                            ex_frame::push_scope(&mut exec.env, ScopeKind::Block);
                            ex_frame::set_local(&mut exec.env, a.name.clone(), Value::String(target_id.name.clone()));
                            ex_frame::set_local(&mut exec.env, "_thread_id", Value::Number(tid as f64));
                        }
                        for s in body.iter() {
                            exec.execute_statement(s)?;
                            if exec.control_flow.is_some() { break; }
                            let _ = exec.collect_garbage();
                        }
                        if alias.is_some() {
                            ex_frame::pop_scope(&mut exec.env, "WHILE body", &mut exec.diagnostics);
                        }
                        iterations += 1;
                    }
                }

                exec.env.remove_thread(tid);
            }

            Ok(None)
        }

        // ── FOR x IN iterable ─────────────────────────────────────────────────
        Statement::ForIn { var, iterable, body, scope_modifier, span } => {
            let iter_val_raw = eval_expr(exec, iterable)?;
            let iter_val = exec.deref(iter_val_raw);
            let elements: Vec<Value> = match iter_val {
                Value::List(items) => items,
                Value::String(s)   => s.chars().map(|c| Value::String(c.to_string())).collect(),
                Value::Number(n)   => (0..n as i64).map(|i| Value::Number(i as f64)).collect(),
                other => {
                    return Err(exec.span_err(span, format!("FOR IN: cannot iterate over {}", match &other {
                        Value::Bool(_)   => "bool",
                        Value::None      => "none",
                        Value::List(_)   => "list",
                        Value::Lambda(_, _, _) => "lambda",
                        _                => "unknown",
                    })));
                }
            };

            // Scope behavior: always push a Block scope for the loop variable binding.
            // - Default (None): loop var is scoped to the block; other new vars go to parent.
            //   We still push a scope so the loop variable (var.name) is removed on exit.
            // - UnbindScope: same as default (Block scope).
            // - BindScope: push Block scope but hoist all vars to function scope on exit.
            ex_frame::push_scope(&mut exec.env, ScopeKind::Block);
            for element in elements {
                ex_frame::set_local(&mut exec.env, var.name.clone(), element);
                for s in body.iter() {
                    exec.execute_statement(s)?;
                    if exec.control_flow.is_some() { break; }
                    let _ = exec.collect_garbage();
                }
                // Break exits the for loop; Continue moves to the next element;
                // Return/Killed must propagate upward.
                match exec.control_flow {
                    Some(crate::interpreter::executor::ControlFlowSignal::Break) => {
                        exec.control_flow = None;
                        break;
                    }
                    Some(crate::interpreter::executor::ControlFlowSignal::Continue) => {
                        exec.control_flow = None;
                        continue;
                    }
                    Some(_) => break, // Return / Killed — propagate
                    None => {}
                }
            }
            if scope_modifier.as_ref() == Some(&ScopeModifier::BindScope) {
                exec.env.pop_scope_hoist().ok();
            } else {
                ex_frame::pop_scope(&mut exec.env, "FOR IN", &mut exec.diagnostics);
            }

            ex_frame::pop_frame(&mut exec.traceback);
            Ok(None)
        }

        Statement::PriorityOverride { higher, lower, span: _ } => {
            exec.priorities.add_edge(&higher.name, &lower.name);
            Ok(None)
        }

        Statement::Constraint { left, relation, right, constraint, span: _ } => {
            let left_s       = expr_to_simple(left);
            let right_s      = expr_to_simple(right);
            let constraint_s = expr_to_simple(constraint);
            let rel_enum     = relation.as_ref().and_then(|rt| Relation::from_str(&rt.text));
            exec.constraints.add_constraint(ConstraintExpr::new(left_s, rel_enum, right_s, constraint_s));
            Ok(None)
        }

        Statement::Print { expr, span: _ } => {
            let v = eval_expr(exec, expr)?;
            exec.do_print(&v);
            Ok(Some(v))
        }

        Statement::If { conditions, then_body, else_body, scope_modifier, span: _ } => {
            let any_true = conditions.iter().try_fold(false, |acc, cond| {
                let val = eval_expr(exec, cond)?;
                Ok::<bool, anyhow::Error>(acc || exec.value_is_truthy(&val))
            })?;

            let active_body = if any_true {
                Some(then_body.as_slice())
            } else {
                else_body.as_deref()
            };
            if let Some(stmts) = active_body {
                // Scope behavior:
                // - Default (None): no new scope; variables are visible after the block.
                // - UnbindScope: push Block scope; variables die at end.
                // - BindScope: push Block scope but hoist vars to function scope on exit.
                let push_block = scope_modifier.is_some();
                if push_block {
                    crate::interpreter::ex_frame::push_scope(&mut exec.env, ScopeKind::Block);
                }
                let result = (|| {
                    for stmt in stmts {
                        exec.execute_statement(stmt)?;
                        if exec.control_flow.is_some() { break; }
                        let _ = exec.collect_garbage();
                    }
                    Ok::<(), anyhow::Error>(())
                })();
                if push_block {
                    if scope_modifier.as_ref() == Some(&ScopeModifier::BindScope) {
                        exec.env.pop_scope_hoist().ok();
                    } else {
                        crate::interpreter::ex_frame::pop_scope(&mut exec.env, "IF block cleanup", &mut exec.diagnostics);
                    }
                }
                result?;
            }
            Ok(None)
        }

        Statement::End { .. } => Ok(None),

        Statement::Break { span: _ } => {
            exec.control_flow = Some(crate::interpreter::executor::ControlFlowSignal::Break);
            Ok(None)
        }

        Statement::Continue { span: _ } => {
            exec.control_flow = Some(crate::interpreter::executor::ControlFlowSignal::Continue);
            Ok(None)
        }

        Statement::ExprStmt { expr, span: _ } => {
            if !matches!(expr, Expr::Raw(_, _)) {
                let v = eval_expr(exec, expr)?;
                return Ok(Some(v));
            }
            Ok(None)
        }

        Statement::Other { kind, payload, span: _ } => {
            if kind == "reserved_keyword_error" {
                let msg = payload.as_deref().unwrap_or("reserved keyword used as variable name");
                return Err(anyhow!("{}", msg));
            }
            exec.diagnostics.push(format!("Unhandled statement kind: {}", kind));
            if let Some(p) = payload {
                exec.diagnostics.push(format!("  payload: {}", p));
            }
            Ok(None)
        }

        // ── RET.NOW(): expr ───────────────────────────────────────────────────
        Statement::RetNow { value, span: _ } => {
            let v = eval_expr(exec, value)?;
            let v_concrete = exec.deref(v);
            // If this executor is a pipeline stage, forward the value downstream.
            if let Some(ref tx) = exec.pipeline_tx {
                let _ = tx.send(v_concrete.clone());
            }
            exec.control_flow = Some(crate::interpreter::executor::ControlFlowSignal::Return(v_concrete));
            Ok(None)
        }

        // ── RET.LATE(condition): expr ─────────────────────────────────────────
        Statement::RetLate { value, condition, span: _ } => {
            use std::time::{SystemTime, UNIX_EPOCH};

            let snapshot = eval_expr(exec, value)?;

            let trigger = match condition {
                RetLateCondition::AfterMs(ms_expr) => {
                    let ms_val = eval_expr(exec, ms_expr)?;
                    let ms = match &ms_val {
                        Value::Number(n) => *n as u64,
                        _ => 0,
                    };
                    // RET.LATE(0ms) — warn and treat as RET.NOW
                    if ms == 0 {
                        eprintln!("Warning: RET.LATE(0ms) has no delay — treating as RET.NOW. Use RET.NOW for instant returns.");
                        exec.control_flow = Some(crate::interpreter::executor::ControlFlowSignal::Return(snapshot));
                        return Ok(None);
                    }
                    let now_ms = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|d| d.as_millis() as u64)
                        .unwrap_or(0);
                    crate::interpreter::environment::PendingTrigger::AtMs(now_ms + ms)
                }
                RetLateCondition::WhenCalled(fn_name) => {
                    crate::interpreter::environment::PendingTrigger::WhenCalled(fn_name.clone())
                }
            };

            let pending = Value::Pending(Box::new(snapshot), trigger);
            exec.ret_late_pending = Some(pending);
            Ok(None)
        }

        // ── ATTEMPT(err_var): try_body ELSE: else_body END ────────────────────
        Statement::AttemptBlock { err_var, try_body, else_body, span: _ } => {
            crate::interpreter::exceptions::execute_attempt_block(exec, err_var, try_body, else_body)
        }

        // ── TRY: try_body OTHERWISE: else_body END ────────────────────────────
        Statement::TryBlock { try_body, else_body, span: _ } => {
            crate::interpreter::exceptions::execute_try_block(exec, try_body, else_body)
        }

        Statement::ObjDecl { .. } | Statement::SpawnBlock { .. } | Statement::DefDoUntil(_) => {
            Ok(None)
        }

        // ── GOTO loop statements ──────────────────────────────────────────────
        Statement::LoopBlock { name, body, span: _ } => {
            // If we arrived here via a forward GOTO, clear that signal — it just means
            // "jump to this block", not "restart it".
            if let Some(crate::interpreter::executor::ControlFlowSignal::GotoLabel(ref lbl)) = exec.control_flow {
                if lbl == name {
                    exec.control_flow = None;
                }
            }
            // Run the body in a loop; GOTO <name> from INSIDE the body restarts from the top.
            loop {
                for stmt in body.iter() {
                    exec.execute_statement(stmt)?;
                    if exec.control_flow.is_some() { break; }
                    let _ = exec.collect_garbage();
                }
                match exec.control_flow.take() {
                    None => {}  // body completed with no control flow — fall through to break
                    Some(crate::interpreter::executor::ControlFlowSignal::GotoLabel(ref lbl)) if lbl == name => {
                        // GOTO from inside the body — restart from top
                        exec.control_flow = None;
                        continue;
                    }
                    Some(crate::interpreter::executor::ControlFlowSignal::Break) => break,
                    Some(other) => { exec.control_flow = Some(other); break; }
                }
                // Body ran to completion with no restart signal — single-pass exit.
                break;
            }
            Ok(None)
        }

        Statement::GotoBlock { name, body, span: _ } => {
            // GOTO ptr: ... END — set the active pointer context for the block body.
            let ptr_id = match exec.env.get(name) {
                Some(Value::Pointer(id)) => id,
                Some(other) => return Err(anyhow::anyhow!(
                    "GOTO block: '{}' is not a pointer (got {:?})", name, other
                )),
                None => return Err(anyhow::anyhow!(
                    "GOTO block: undefined variable '{}'", name
                )),
            };
            if exec.pointer_context.current().is_some() {
                return Err(anyhow::anyhow!(
                    "GOTO block: nested pointer contexts not allowed (already inside a GOTO block)"
                ));
            }
            exec.pointer_context.push_context(ptr_id);
            let result = (|| {
                for stmt in body.iter() {
                    exec.execute_statement(stmt)?;
                    if exec.control_flow.is_some() { break; }
                    let _ = exec.collect_garbage();
                }
                Ok::<(), anyhow::Error>(())
            })();
            exec.pointer_context.pop_context();
            result?;
            Ok(None)
        }

        Statement::GotoLabel { label, span: _ } => {
            exec.control_flow = Some(crate::interpreter::executor::ControlFlowSignal::GotoLabel(label.clone()));
            Ok(None)
        }

        Statement::Pull { dtype, explicit_ptr, args, target, span: _ } => {
            use crate::runtime::pointer::ops::DataType;

            // Resolve pointer: explicit arg takes priority, else use active GOTO context
            let ptr_id = if let Some(ptr_expr) = explicit_ptr {
                match eval_expr(exec, ptr_expr)? {
                    Value::Pointer(id) => id,
                    other => return Err(anyhow::anyhow!("PULL: explicit pointer is not a pointer value (got {:?})", other)),
                }
            } else {
                exec.pointer_context.current()
                    .ok_or_else(|| anyhow::anyhow!("error[E072]: PULL requires a pointer argument or an active GOTO block"))?
            };

            // Evaluate args before getting mutable registry lock
            let arg_vals: Vec<Value> = args.iter()
                .map(|a| eval_expr(exec, a))
                .collect::<Result<Vec<_>>>()?;
            
            let data_type = DataType::from_str(dtype)
                .ok_or_else(|| anyhow::anyhow!("PULL: unknown data type '{}'", dtype))?;
            
            let mut registry = exec.pointer_registry.write().unwrap();
            let ptr = registry.lookup_mut(ptr_id)
                .ok_or_else(|| anyhow::anyhow!("error[E073]: Pointer {} not found in registry", ptr_id))?;
            
            if !ptr.alive {
                return Err(anyhow::anyhow!("error[E070]: Use after free: pointer {} was accessed after being freed", ptr_id));
            }
            
            let pulled_value = crate::runtime::pointer::ops::pull(ptr, data_type, &arg_vals)
                .map_err(|e| anyhow::anyhow!("PULL error: {}", e))?;
            
            drop(registry);
            
            if let Some(tgt) = target {
                exec.env.assign(&tgt.name, pulled_value);
            }
            Ok(None)
        }

        Statement::Push { dtype, explicit_ptr, value, args, span: _ } => {
            use crate::runtime::pointer::ops::DataType;

            // Resolve pointer: explicit arg takes priority, else use active GOTO context
            let ptr_id = if let Some(ptr_expr) = explicit_ptr {
                match eval_expr(exec, ptr_expr)? {
                    Value::Pointer(id) => id,
                    other => return Err(anyhow::anyhow!("PUSH: explicit pointer is not a pointer value (got {:?})", other)),
                }
            } else {
                exec.pointer_context.current()
                    .ok_or_else(|| anyhow::anyhow!("error[E072]: PUSH requires a pointer argument or an active GOTO block"))?
            };

            let raw_val = eval_expr(exec, value)?;
            let val = exec.deref(raw_val);
            
            let arg_vals: Vec<Value> = args.iter()
                .map(|a| eval_expr(exec, a))
                .collect::<Result<Vec<_>>>()?;
            
            let data_type = DataType::from_str(dtype)
                .ok_or_else(|| anyhow::anyhow!("PUSH: unknown data type '{}'", dtype))?;
            
            let mut registry = exec.pointer_registry.write().unwrap();
            let ptr = registry.lookup_mut(ptr_id)
                .ok_or_else(|| anyhow::anyhow!("error[E073]: Pointer {} not found in registry", ptr_id))?;
            
            if !ptr.alive {
                return Err(anyhow::anyhow!("error[E070]: Use after free: pointer {} was accessed after being freed", ptr_id));
            }
            
            crate::runtime::pointer::ops::push(ptr, data_type, val, &arg_vals)
                .map_err(|e| anyhow::anyhow!("PUSH error: {}", e))?;
            Ok(None)
        }

        Statement::Alloc { target, kind, args, metadata: _, span: _ } => {
            use crate::runtime::pointer::PointerKind;
            
            let kind_upper = kind.to_uppercase();
            let ptr_kind = match kind_upper.as_str() {
                "MEM" => PointerKind::Mem,
                "FILE" => PointerKind::File,
                "DEV" => PointerKind::Dev,
                "NET" => PointerKind::Net,
                _ => return Err(anyhow::anyhow!("ALLOC: unknown pointer kind '{}'", kind)),
            };
            
            // Evaluate args
            let arg_vals: Vec<Value> = args.iter()
                .map(|a| eval_expr(exec, a))
                .collect::<Result<Vec<_>>>()?;
            
            let ptr_id = {
                let mut registry = exec.pointer_registry.write().unwrap();
                match ptr_kind {
                    PointerKind::Mem => {
                        let size = arg_vals.get(0)
                            .and_then(|v| if let Value::Number(n) = v { Some(*n as usize) } else { None })
                            .unwrap_or(1024);
                        registry.register_mem(size)
                    }
                    PointerKind::File => {
                        let path = arg_vals.get(0)
                            .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                            .unwrap_or_default();
                        let mode = arg_vals.get(1)
                            .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                            .unwrap_or_else(|| "rw".to_string());
                        registry.register_file(path, mode)
                    }
                    PointerKind::Dev => {
                        let device_id = arg_vals.get(0)
                            .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                            .unwrap_or_default();
                        let device_type = arg_vals.get(1)
                            .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                            .unwrap_or_else(|| "generic".to_string());
                        registry.register_device(device_id, device_type)
                    }
                    PointerKind::Net => {
                        let host = arg_vals.get(0)
                            .and_then(|v| if let Value::String(s) = v { Some(s.clone()) } else { None })
                            .unwrap_or_default();
                        let port = arg_vals.get(1)
                            .and_then(|v| if let Value::Number(n) = v { Some(*n as u16) } else { None })
                            .unwrap_or(80);
                        registry.register_network(host, port)
                    }
                }
            };
            
            // Track allocation in GC (v1.4.4)
            exec.pointer_gc_tracker.track_allocation(ptr_id);
            
            exec.env.assign(&target.name, Value::Pointer(ptr_id));
            Ok(None)
        }

        Statement::Free { pointer_expr, span: _ } => {
            let ptr_val = eval_expr(exec, pointer_expr)?;
            if let Value::Pointer(ptr_id) = ptr_val {
                // Check if pointer exists
                let exists = exec.pointer_registry.read().unwrap().lookup(ptr_id).is_some();
                if !exists {
                    return Err(anyhow::anyhow!("error[E073]: Pointer {} not found in registry", ptr_id));
                }
                
                // Notify GC of free (v1.4.4)
                exec.pointer_gc_tracker.unroot(ptr_id);
                exec.pointer_registry.write().unwrap().kill(ptr_id);
                Ok(None)
            } else {
                Err(anyhow::anyhow!("error[E074]: Invalid pointer type: FREE requires a pointer value, got {:?}", ptr_val))
            }
        }

        Statement::Info { pointer_expr, target, span: _ } => {
            let ptr_val = eval_expr(exec, pointer_expr)?;
            if let Value::Pointer(ptr_id) = ptr_val {
                let registry = exec.pointer_registry.read().unwrap();
                let ptr = registry.lookup(ptr_id);
                
                if ptr.is_none() {
                    return Err(anyhow::anyhow!("error[E073]: Pointer {} not found in registry", ptr_id));
                }
                
                let info = ptr.map(|p| p.info()).unwrap_or(Value::None);
                drop(registry);
                
                if let Some(tgt) = target {
                    exec.env.assign(&tgt.name, info);
                }
                Ok(None)
            } else {
                Err(anyhow::anyhow!("error[E074]: Invalid pointer type: INFO requires a pointer value, got {:?}", ptr_val))
            }
        }

        Statement::Seek { pointer_expr, offset_expr, span: _ } => {
            let ptr_val = eval_expr(exec, pointer_expr)?;
            let offset_val = eval_expr(exec, offset_expr)?;
            
            if let Value::Pointer(ptr_id) = ptr_val {
                let offset = match offset_val {
                    Value::Number(n) => n as usize,
                    _ => return Err(anyhow::anyhow!("SEEK: offset must be a number, got {:?}", offset_val)),
                };
                
                let mut registry = exec.pointer_registry.write().unwrap();
                match registry.lookup_mut(ptr_id) {
                    None => return Err(anyhow::anyhow!("error[E073]: Pointer {} not found in registry", ptr_id)),
                    Some(ptr) if !ptr.alive => return Err(anyhow::anyhow!(
                        "error[E070]: Use after free: pointer {} was accessed after being freed", ptr_id
                    )),
                    Some(ptr) => {
                        ptr.seek(offset).map_err(|e| anyhow::anyhow!("SEEK error: {}", e))?;
                    }
                }
                Ok(None)
            } else {
                Err(anyhow::anyhow!("error[E074]: Invalid pointer type: SEEK requires a pointer value, got {:?}", ptr_val))
            }
        }

        Statement::Swap { var1, var2, span: _ } => {
            // Get values of both variables
            let val1 = exec.env.get(&var1.name)
                .ok_or_else(|| anyhow::anyhow!("error[E2001]: undefined variable '{}' in SWAP", var1.name))?;
            let val2 = exec.env.get(&var2.name)
                .ok_or_else(|| anyhow::anyhow!("error[E2001]: undefined variable '{}' in SWAP", var2.name))?;
            
            // Swap them
            exec.env.assign(&var1.name, val2);
            exec.env.assign(&var2.name, val1);
            
            Ok(None)
        }

        Statement::UseUnsafe { write_access, .. } => {
            use crate::runtime::family::UnsafePermission;
            let perm = if *write_access { UnsafePermission::Write } else { UnsafePermission::Read };
            exec.family_permission = perm;
            exec.family_registry = crate::runtime::family::FamilyRegistry::new(perm);
            Ok(None)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Expression evaluation
// ─────────────────────────────────────────────────────────────────────────────

/// Recursively evaluate an AST expression and return the resulting `Value`.
///
/// This is the main expression evaluator, extracted verbatim from
/// `Executor::eval_expr`.  All operator and builtin dispatch sub-functions
/// called from here remain on `Executor` (they are `pub` there) so that the
/// test suite and the old call sites in `executor.rs` continue to work.

// Helper: execute a user-defined function body in a fresh scope and return the Value.
#[allow(dead_code)]
fn call_user_function(
    exec: &mut crate::interpreter::executor::Executor,
    func_params: &Vec<crate::parser::Identifier>,
    func_body: &Vec<crate::parser::Statement>,
    argvals: &Vec<crate::interpreter::Value>,
) -> Result<crate::interpreter::Value> {
    // Push a new local scope for the function.
    ex_frame::push_scope(&mut exec.env, ScopeKind::Function);

    // Bind parameters
    for (i, param) in func_params.iter().enumerate() {
        let val = argvals.get(i).cloned().unwrap_or(crate::interpreter::Value::None);
        // If a list is passed by value, allocate it on the heap so the
        // callee receives a shared, mutable handle.
        let val_to_bind = match val {
            crate::interpreter::environment::Value::List(_) => {
                let id = exec.gc.allocate(val);
                crate::interpreter::environment::Value::Heap(id)
            }
            other => other,
        };
        ex_frame::set_local(&mut exec.env, param.name.clone(), val_to_bind);
    }

    // Execute the function body statements using the executor's statement runner.
    let mut last = crate::interpreter::Value::None;
    for s in func_body.iter() {
        if let Some(v) = exec.execute_statement(s)? {
            last = v;
        }
        if let Some(crate::interpreter::executor::ControlFlowSignal::Return(ret_val)) = exec.control_flow.take() {
            let ret_concrete = exec.deref_return(ret_val);
            ex_frame::pop_scope(&mut exec.env, "RET.NOW in fn", &mut exec.diagnostics);
            return Ok(ret_concrete);
        }
        let _ = exec.collect_garbage();
    }

    // If there is a pending late return value, return it as-is (caller uses resolve()).
    if let Some(p) = exec.ret_late_pending.take() {
        last = p;
    }

    ex_frame::pop_scope(&mut exec.env, "fn call", &mut exec.diagnostics);
    Ok(last)
}

// ─────────────────────────────────────────────────────────────────────────────
// String interpolation helper
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluate `"... {expr} ..."` string interpolation.
///
/// Rules:
/// - `{expr}` — any Pasta expression; evaluated and stringified.
/// - `{{`      — literal `{`.
/// - `}}`      — literal `}`.
/// - `\{`      — also accepted as a literal `{` (backslash-escape form).
/// - Lone `}` (not part of `}}`) is passed through unchanged.
///
/// Nested braces inside the expression (e.g. `{func(a, b)}`) are handled
/// by tracking brace depth so `{my_list[0]}` and similar work correctly.
fn interpolate_string(exec: &mut Executor, s: &str) -> Result<Value> {
    // Fast path — no interpolation needed.
    if !s.contains('{') && !s.contains("}}") {
        return Ok(Value::String(s.to_string()));
    }

    let mut result = String::with_capacity(s.len() + 32);
    let chars: Vec<char> = s.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        if ch == '{' {
            // `{{` → literal `{`
            if i + 1 < len && chars[i + 1] == '{' {
                result.push('{');
                i += 2;
                continue;
            }
            // Start of an interpolation expression.
            i += 1; // skip the opening `{`
            let mut expr_text = String::new();
            let mut depth: usize = 1;

            while i < len && depth > 0 {
                match chars[i] {
                    '{' => { depth += 1; expr_text.push('{'); }
                    '}' => {
                        depth -= 1;
                        if depth > 0 { expr_text.push('}'); }
                    }
                    c => expr_text.push(c),
                }
                i += 1;
            }

            let expr_text = expr_text.trim().to_string();
            if expr_text.is_empty() {
                // `{}` — leave as-is
                result.push_str("{}");
            } else {
                let tokens = crate::lexer::lexer::Lexer::new(&expr_text).lex();
                let mut parser = crate::parser::parser::Parser::new(tokens);
                let expr = parser.parse_single_expr();
                let val = eval_expr(exec, &expr)?;
                let val = exec.deref(val);
                result.push_str(&Executor::value_to_string(&val));
            }
        } else if ch == '}' {
            // `}}` → literal `}`
            if i + 1 < len && chars[i + 1] == '}' {
                result.push('}');
                i += 2;
            } else {
                // Lone `}` — pass through unchanged.
                result.push('}');
                i += 1;
            }
        } else {
            result.push(ch);
            i += 1;
        }
    }

    Ok(Value::String(result))
}

pub fn eval_expr(exec: &mut Executor, expr: &Expr) -> Result<Value> {
    // Recursion guard: prevent stack overflow by returning a controlled error
    // when recursion depth exceeds RECURSION_LIMIT.
    // Recursion guard (fallback): do not reference span/exec here.
    let _rec_guard = match enter_recursion_guard_with_threshold(200) {
        Ok(g) => g,
        Err(trip) => {
            let snippet = format!("{:?}", expr);
            let tb = crate::interpreter::errors::ambient_traceback_clone();
            let diag = error_handler::stack_guard_triggered(trip.depth, None, &snippet, Some(&tb));
            let mut params = std::collections::HashMap::new();
            params.insert("depth", trip.depth.to_string());
            params.insert("expr", snippet.clone());
            let _ = error_handler::persist_diagnostic(
                &trip.backtrace,
                trip.recent_events,
                error_handler::diagnostic_context(
                    "STACK_OVERFLOW",
                    &params,
                    None,
                    Some(&snippet),
                    Some(&tb),
                ),
            );
            return Err(anyhow::anyhow!(diag));
        }
    };
    match expr {
        Expr::Number(n, _)  => Ok(Value::Number(*n)),
        Expr::String(s, _)  => {
            // If the string contains `{`, attempt interpolation.
            // Strings with no `{` are returned unchanged (fast path inside).
            if s.contains('{') || s.contains("}}") {
                interpolate_string(exec, s)
            } else {
                Ok(Value::String(s.clone()))
            }
        }
        Expr::Bool(b, _)    => Ok(Value::Bool(*b)),
        Expr::None(_)       => Ok(Value::None),

        Expr::Identifier(id) => {
            // Magic runtime identifiers
            match id.name.to_uppercase().as_str() {
                "RAND" => {
                    let rng = crate::runtime::Rng::new();
                    return Ok(Value::Bool(rng.next_u64() & 1 == 1));
                }
                _ => {}
            }
            if let Some(v) = exec.env.get(&id.name) {
                // If this is a lazy import binding, trigger module load and replace binding.
                if let Value::LazyImport { module, name: orig_name, alias: _ } = v {
                    // Attempt to load module and fetch export
                    let actual = exec.get_module_export(&module, &orig_name)?;
                    // If the binding name (possibly an alias) differs from the original
                    // function name, also register the function params under the alias.
                    if id.name != orig_name {
                        if let Some(entry) = exec.functions.get(&orig_name).cloned() {
                            exec.functions.insert(id.name.clone(), entry);
                        }
                    }
                    // Replace binding in current (innermost) scope so subsequent lookups are fast.
                    ex_frame::set_local(&mut exec.env, id.name.clone(), actual.clone());
                    return Ok(actual);
                }
                return Ok(v);
            }
            // If not found in the environment, check the named functions
            // table — allow function names to be referenced as first-class
            // values (so `apply_twice(inc, 5)` works when `inc` is a DEF).
            if let Some((params, body)) = exec.functions.get(&id.name).cloned()
                .or_else(|| exec.functions.get(&id.name.to_lowercase()).cloned()) {
                // Return an in-memory Lambda value (caller will bind it into
                // the callee scope or treat it as a value).
                return Ok(Value::Lambda(params, body, exec.env.capture_scope()));
            }
            let rerr = RuntimeError::new(
                RuntimeErrorKind::UndefinedVariable(id.name.clone()),
                id.span.clone(),
            );
            return Err(anyhow!(rerr));
        }

        Expr::Binary { op, left, right, .. } => {
            // PipeArrow (|>) must be intercepted here to access &mut Executor for calls
            if *op == BinaryOp::PipeArrow {
                // `arg |> func` — left is the argument, right is the function.
                let arg_val  = eval_expr(exec, left)?;
                let func_val = eval_expr(exec, right)?;
                return exec.call_value(func_val, vec![arg_val]);
            }
            let lv = eval_expr(exec, left)?;
            let rv = eval_expr(exec, right)?;
            exec.eval_binary(op, lv, rv)
        }

        Expr::Call { callee, args, .. } => {
                // Special-case RET.NOW used as an expression: trigger early return.
                if let Expr::Identifier(id) = &**callee {
                    let name_lc = id.name.to_lowercase();
                    if name_lc.starts_with("ret.now") || name_lc == "ret.now" {
                        // Evaluate first argument if present, else None sentinel.
                        let ret_val = if args.len() >= 1 {
                            eval_expr(exec, &args[0])?
                        } else {
                            Value::None
                        };
                        let ret_concrete = exec.deref_return(ret_val);
                        exec.control_flow = Some(crate::interpreter::executor::ControlFlowSignal::Return(ret_concrete.clone()));
                        return Ok(ret_concrete);
                    }
                }

            
            
            // Defensive early-return: if recursion depth is already high, return a controlled error
            // instead of continuing into a potential eval_expr <-> eval_stmt recursion loop.
            // This is a temporary safeguard to produce a diagnostic and avoid stack exhaustion.
            {
                let current_depth = RECURSION_DEPTH.with(|d| d.get());
                if current_depth > 50 {
                    let snippet = short_expr_summary(expr);
                    return Err(anyhow::anyhow!(
                        "recursion guard: deep recursion detected (depth = {}) while evaluating expression: {}",
                        current_depth,
                        snippet
                    ));
                }
            }
    if let Expr::Identifier(id) = &**callee {
                let name = id.name.to_lowercase();
                let mut argvals = Vec::with_capacity(args.len());
                for a in args {
                    argvals.push(eval_expr(exec, a)?);
                }
                
                // 0. Check for LazyImport FIRST and resolve it before other lookups
                if let Some(env_val) = exec.env.get(&id.name) {
                    if let Value::LazyImport { module: lz_mod, name: lz_sym, alias: _ } = exec.deref(env_val) {
                        // Trigger lazy module load
                        let actual = exec.get_module_export(&lz_mod, &lz_sym)?;
                        // Rebind so future calls are direct
                        ex_frame::set_local(&mut exec.env, id.name.clone(), actual.clone());
                        // Copy function params from original name to alias so named params work
                        if id.name != lz_sym {
                            if let Some(entry) = exec.functions.get(&lz_sym).cloned() {
                                exec.functions.insert(id.name.clone(), entry);
                            }
                        }
                        // If actual is a Lambda, execute it with proper named parameters
                        if let Value::Lambda(_, stmts, _lambda_caps) = exec.deref(actual) {
                            ex_frame::push_frame(&mut exec.traceback, id.span.clone(), format!("call {}", id.name));
                            ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                            // Look up function params by original symbol name
                            if let Some((func_params, _)) = exec.functions.get(&lz_sym).cloned() {
                                for (i, param) in func_params.iter().enumerate() {
                                    let val = argvals.get(i).cloned().unwrap_or(Value::None);
                                    ex_frame::set_local(&mut exec.env, param.name.clone(), val);
                                }
                            } else {
                                // Fallback to positional args
                                for (i, av) in argvals.iter().enumerate() {
                                    ex_frame::set_local(&mut exec.env, format!("__arg_{}__", i), av.clone());
                                }
                            }
                            let mut last = Value::None;
                            for s in stmts.iter() {
                                if let Some(v) = exec.execute_statement(s)? { last = v; }
                                if let Some(crate::interpreter::executor::ControlFlowSignal::Return(ret_val)) = exec.control_flow.take() {
                                    let ret_concrete = exec.deref_return(ret_val);
                                    ex_frame::pop_scope(&mut exec.env, "RET.NOW in lazy import", &mut exec.diagnostics);
                                    ex_frame::pop_frame(&mut exec.traceback);
                                    return Ok(ret_concrete);
                                }
                            }
                            ex_frame::pop_scope(&mut exec.env, "lazy import call", &mut exec.diagnostics);
                            ex_frame::pop_frame(&mut exec.traceback);
                            return Ok(last);
                        }
                    }
                }
                
                // 1. Named functions with params (DEF name(params): …)
                let fn_lkp: Option<(Vec<crate::parser::Identifier>, Vec<Statement>)> =
                    exec.functions.get(&id.name).or_else(|| exec.functions.get(&name)).cloned();

                if let Some((func_params, func_body)) = fn_lkp {
                    // Record this function as having been called (for RET.LATE WHEN triggers).
                    exec.fired_events.insert(id.name.clone());
                    // Push a traceback frame for this call so runtime errors inside
                    // the callee show the call-site and function name.
                    ex_frame::push_frame(&mut exec.traceback, id.span.clone(), format!("call {}", id.name));
                    ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                    let func_params: &Vec<crate::parser::Identifier> = &func_params;
                    for (i, param) in func_params.iter().enumerate() {
                        let val = argvals.get(i).cloned().unwrap_or(Value::None);
                        let val_to_bind = match val {
                            crate::interpreter::environment::Value::List(_) => {
                                let id = exec.gc.allocate(val);
                                crate::interpreter::environment::Value::Heap(id)
                            }
                            other => other,
                        };
                        ex_frame::set_local(&mut exec.env, param.name.clone(), val_to_bind);
                    }
                    let mut last = Value::None;
                    let func_body: &Vec<Statement> = &func_body;
                    for s in func_body.iter() {
                        if let Some(v) = exec.execute_statement(s)? { last = v; }
                        if let Some(crate::interpreter::executor::ControlFlowSignal::Return(ret_val)) = exec.control_flow.take() {
                            let ret_concrete = exec.deref_return(ret_val);
                            ex_frame::pop_scope(&mut exec.env, "RET.NOW in fn", &mut exec.diagnostics);
                            ex_frame::pop_frame(&mut exec.traceback);
                            return Ok(ret_concrete);
                        }
                        let _ = exec.collect_garbage();
                    }
                    let pending = exec.ret_late_pending.take();
                    // Return the Pending handle as-is so the caller can resolve() it.
                    if let Some(p) = pending { last = p; }
                    ex_frame::pop_scope(&mut exec.env, "fn call", &mut exec.diagnostics);
                    ex_frame::pop_frame(&mut exec.traceback);
                    return Ok(last);
                }

                // 2. Lambda stored in environment (resolve LazyImport on first call)
                let callee_env_name = if let Expr::Identifier(id) = &**callee { id.name.clone() } else { name.clone() };
                let raw_env_val = exec.env.get(&callee_env_name)
                    .or_else(|| exec.env.get(&name))
                    .map(|v| exec.deref(v));
                let maybe_lambda = match raw_env_val {
                    Some(Value::LazyImport { module: lz_mod, name: lz_sym, alias: _ }) => {
                        // Trigger lazy module load; rebind so future calls are direct
                        let actual = exec.get_module_export(&lz_mod, &lz_sym)?;
                        // If the function has named params registered under the original name,
                        // also register them under the alias so parameter binding works.
                        if callee_env_name != lz_sym {
                            if let Some(entry) = exec.functions.get(&lz_sym).cloned() {
                                exec.functions.insert(callee_env_name.clone(), entry);
                            }
                        }
                        ex_frame::set_local(&mut exec.env, callee_env_name.clone(), actual.clone());
                        Some(exec.deref(actual))
                    }
                    other => other
                };
                // List label(n): 1-based indexing — `label(1)` returns first element.
                if let Some(Value::List(ref items)) = maybe_lambda {
                    if argvals.len() == 1 {
                        if let Value::Number(n) = &argvals[0] {
                            let idx = (*n as isize) - 1;
                            if idx >= 0 && (idx as usize) < items.len() {
                                return Ok(items[idx as usize].clone());
                            } else {
                                return Err(anyhow::anyhow!("list index {} out of range (label has {} items)", n, items.len()));
                            }
                        }
                    }
                }
                if let Some(Value::Lambda(lambda_params, stmts, lambda_captures)) = maybe_lambda {
                    // Push a traceback frame for this env-stored lambda invocation.
                    ex_frame::push_frame(&mut exec.traceback, id.span.clone(), format!("call {}", id.name));
                    ex_frame::push_scope(&mut exec.env, ScopeKind::Function);
                    // Inject captured variables first (params will shadow them if same name).
                    for (k, v) in &lambda_captures {
                        ex_frame::set_local(&mut exec.env, k.clone(), v.clone());
                    }
                    // If the lambda itself has parameters stored, use them.
                    // Otherwise fall back to looking up in the functions table.
                    let callee_name = if let Expr::Identifier(id) = &**callee { id.name.clone() } else { name.clone() };
                    let use_lambda_params = !lambda_params.is_empty();
                    let params_to_use: Vec<crate::parser::Identifier> = if use_lambda_params {
                        lambda_params.clone()
                    } else if let Some((func_params, _)) = exec.functions.get(&callee_name).cloned() {
                        func_params
                    } else {
                        Vec::new()
                    };
                    
                    if !params_to_use.is_empty() {
                        for (i, param) in params_to_use.iter().enumerate() {
                            let v = argvals.get(i).cloned().unwrap_or(Value::None);
                            let v_to_bind = match v {
                                crate::interpreter::environment::Value::List(_) => {
                                    let id = exec.gc.allocate(v);
                                    crate::interpreter::environment::Value::Heap(id)
                                }
                                other => other,
                            };
                            ex_frame::set_local(&mut exec.env, param.name.clone(), v_to_bind);
                        }
                    } else {
                        for (i, val) in argvals.iter().enumerate() {
                            let v = val.clone();
                            let v_to_bind = match v {
                                crate::interpreter::environment::Value::List(_) => {
                                    let id = exec.gc.allocate(v);
                                    crate::interpreter::environment::Value::Heap(id)
                                }
                                other => other,
                            };
                            ex_frame::set_local(&mut exec.env, format!("__arg_{}__", i), v_to_bind);
                        }
                    }
                    let mut last = Value::None;
                    for s in stmts.iter() {
                        if let Some(v) = exec.execute_statement(s)? { last = v; }
                        if let Some(crate::interpreter::executor::ControlFlowSignal::Return(ret_val)) = exec.control_flow.take() {
                            let ret_concrete = exec.deref_return(ret_val);
                            ex_frame::pop_scope(&mut exec.env, "RET.NOW in lambda", &mut exec.diagnostics);
                            ex_frame::pop_frame(&mut exec.traceback);
                            return Ok(ret_concrete);
                        }
                    }
                    
            
                ex_frame::pop_scope(&mut exec.env, "lambda call", &mut exec.diagnostics);
                    ex_frame::pop_frame(&mut exec.traceback);
                    return Ok(last);
                }

                // 3. Fall through to builtins
                exec.call_builtin(&name, argvals)
            } else {
                Err(anyhow!("Call to non-identifier callee not supported"))
            }
        }

        Expr::List { items, .. } => {
            let mut vals = Vec::with_capacity(items.len());
            for it in items {
                vals.push(eval_expr(exec, it)?);
            }
            let id = exec.gc.allocate(Value::List(vals));
            Ok(Value::Heap(id))
        }

        Expr::Dict { pairs, .. } => {
            let mut map = std::collections::HashMap::new();
            for (k, v) in pairs {
                let key_val = eval_expr(exec, k)?;
                let key_str = Executor::value_to_string(&key_val);
                let val = eval_expr(exec, v)?;
                map.insert(key_str, val);
            }
            let id = exec.gc.allocate(Value::Dict(map));
            Ok(Value::Heap(id))
        }

        Expr::Lambda(stmts, _) => {
            // Lambda expressions don't have named params; use empty vec
            let id = exec.gc.allocate(Value::Lambda(Vec::new(), stmts.clone(), exec.env.capture_scope()));
            Ok(Value::Heap(id))
        }

        Expr::Raw(_, _) => Ok(Value::None),

        Expr::TensorBuilder { expr, .. } => {
            // Evaluate the inner expression to get a List (or Number),
            // then build a tensor from it — without calling build_tensor
            // (which would call eval_expr again causing infinite recursion).
            let inner = eval_expr(exec, expr)?;
            let inner = exec.deref(inner);
            fn collect(exe: &crate::interpreter::executor::Executor, v: &crate::interpreter::environment::Value) -> anyhow::Result<(Vec<usize>, Vec<f64>)> {
                let v = exe.deref(v.clone());
                match &v {
                    crate::interpreter::environment::Value::Number(n) => Ok((Vec::new(), vec![*n])),
                    crate::interpreter::environment::Value::List(items) => {
                        if items.is_empty() { return Err(anyhow::anyhow!("Tensor rows cannot be empty")); }
                        let (first_shape, mut first_data) = collect(exe, &items[0])?;
                        let mut flat = Vec::new(); flat.append(&mut first_data);
                        for item in &items[1..] {
                            let (shape, mut data) = collect(exe, item)?;
                            if shape != first_shape { return Err(anyhow::anyhow!("Ragged tensor: inconsistent dimensions")); }
                            flat.append(&mut data);
                        }
                        let mut shape = Vec::with_capacity(1 + first_shape.len());
                        shape.push(items.len()); shape.extend(first_shape);
                        Ok((shape, flat))
                    }
                    other => Err(anyhow::anyhow!("Tensor element must be a number, got: {:?}", other)),
                }
            }
            let (shape, data) = collect(exec, &inner)?;
            if shape.is_empty() { return Err(anyhow::anyhow!("Cannot build tensor from scalar")); }
            Ok(crate::interpreter::environment::Value::Tensor(
                crate::interpreter::environment::RuntimeTensor::new(shape, "float32".to_string(), data)
            ))
        }

        Expr::Index { base, indices, .. } => {
            let base_raw = eval_expr(exec, base)?;
            let base_val = exec.deref(base_raw);
            if indices.is_empty() {
                return Err(anyhow!("Index expression requires at least one index"));
            }
            let idx_val = eval_expr(exec, &indices[0])?;
            match base_val {
                Value::Dict(map) => {
                    let key = exec.deref(idx_val);
                    let key_str = crate::interpreter::executor::Executor::value_to_string(&key);
                    map.get(&key_str).cloned()
                        .ok_or_else(|| anyhow!("dict key \"{}\" not found", key_str))
                }
                Value::List(items) => {
                    let idx = match idx_val {
                        Value::Number(n) => n as isize,
                        other => return Err(anyhow!("List index must be a number, got {:?}", other)),
                    };
                    let len = items.len() as isize;
                    let i = if idx < 0 { len + idx } else { idx };
                    if i < 0 || i >= len {
                        return Err(anyhow!("Index {} out of range (len {})", idx, len));
                    }
                    Ok(exec.deref(items[i as usize].clone()))
                }
                Value::String(s) => {
                    let idx = match idx_val {
                        Value::Number(n) => n as isize,
                        other => return Err(anyhow!("String index must be a number, got {:?}", other)),
                    };
                    let chars: Vec<char> = s.chars().collect();
                    let len = chars.len() as isize;
                    let i = if idx < 0 { len + idx } else { idx };
                    if i < 0 || i >= len {
                        return Err(anyhow!("String index {} out of range (len {})", idx, len));
                    }
                    Ok(Value::String(chars[i as usize].to_string()))
                }
                other => Err(anyhow!("Cannot index into {:?}", other)),
            }
        }

        Expr::ConstructorCall { .. } => Err(anyhow!("constructor calls not implemented")),
        Expr::Combine { .. }         => Err(anyhow!("combine operator not implemented")),
        Expr::Reassign { .. }        => Err(anyhow!("reassign operator not implemented")),

        // ── REF expression (v1.4.4) ───────────────────────────────────────────
        Expr::Ref { kind, target, metadata, span: _ } => {
            use crate::runtime::pointer::PointerKind;
            
            let kind_upper = kind.to_uppercase();
            let ptr_kind = match kind_upper.as_str() {
                "MEM" => PointerKind::Mem,
                "FILE" => PointerKind::File,
                "DEV" => PointerKind::Dev,
                "NET" => PointerKind::Net,
                _ => return Err(anyhow!("REF: unknown pointer kind '{}'", kind)),
            };
            
            // Evaluate the target expression
            let target_val = eval_expr(exec, target)?;
            
            // Evaluate metadata
            let meta_values: Vec<(String, Value)> = {
                let mut pairs = Vec::new();
                for (key, val_expr) in metadata {
                    let val = eval_expr(exec, val_expr)?;
                    pairs.push((key.clone(), val));
                }
                pairs
            };
            
            // Register pointer based on kind and target
            let ptr_id = {
                let mut registry = exec.pointer_registry.write().unwrap();
                match ptr_kind {
                    PointerKind::Mem => {
                        match target_val {
                            Value::List(items) => {
                                // Convert list to bytes
                                let bytes: Vec<u8> = items.iter().filter_map(|v| {
                                    if let Value::Number(n) = v { Some(*n as u8) } else { None }
                                }).collect();
                                registry.register_mem_with_data(bytes)
                            }
                            Value::String(s) => {
                                registry.register_mem_with_data(s.into_bytes())
                            }
                            Value::Number(n) => {
                                // Treat as memory size
                                registry.register_mem(n as usize)
                            }
                            _ => return Err(anyhow!("REF.MEM: invalid target type")),
                        }
                    }
                    PointerKind::File => {
                        if let Value::String(path) = target_val {
                            // Get mode from metadata or default to "rw"
                            let mode = meta_values.iter()
                                .find(|(k, _)| k == "mode")
                                .and_then(|(_, v)| if let Value::String(s) = v { Some(s.clone()) } else { None })
                                .unwrap_or_else(|| "rw".to_string());
                            registry.register_file(path, mode)
                        } else {
                            return Err(anyhow!("REF.FILE: target must be a file path string"));
                        }
                    }
                    PointerKind::Dev => {
                        if let Value::String(device) = target_val {
                            // Get device_type from metadata or default to "generic"
                            let device_type = meta_values.iter()
                                .find(|(k, _)| k == "type")
                                .and_then(|(_, v)| if let Value::String(s) = v { Some(s.clone()) } else { None })
                                .unwrap_or_else(|| "generic".to_string());
                            registry.register_device(device, device_type)
                        } else {
                            return Err(anyhow!("REF.DEV: target must be a device path string"));
                        }
                    }
                    PointerKind::Net => {
                        if let Value::String(endpoint) = target_val {
                            // Parse endpoint as "host:port" or just "host" (default port 80)
                            let parts: Vec<&str> = endpoint.split(':').collect();
                            let host = parts[0].to_string();
                            let port: u16 = parts.get(1).and_then(|p| p.parse().ok()).unwrap_or(80);
                            registry.register_network(host, port)
                        } else {
                            return Err(anyhow!("REF.NET: target must be a network endpoint string"));
                        }
                    }
                }
            };
            
            // Apply remaining metadata to the pointer
            if !meta_values.is_empty() {
                let mut registry = exec.pointer_registry.write().unwrap();
                if let Some(ptr) = registry.lookup_mut(ptr_id) {
                    for (key, value) in meta_values {
                        ptr.metadata.set(key, value);
                    }
                }
            }
            
            // Track allocation in GC (v1.4.4)
            exec.pointer_gc_tracker.track_allocation(ptr_id);
            
            Ok(Value::Pointer(ptr_id))
        }

        Expr::ObjFamNew { group, mutable, parent_a, parent_b, .. } => {
            let va = eval_expr(exec, parent_a)?;
            let vb = eval_expr(exec, parent_b)?;
            let pa_id = match va {
                Value::FamilyNode { id, .. } => id,
                Value::Number(n) => n as u64,
                _ => 0,
            };
            let pb_id = match vb {
                Value::FamilyNode { id, .. } => id,
                Value::Number(n) => n as u64,
                _ => 0,
            };
            use crate::runtime::family::{ObjGroup, NodeRole};
            let obj_group = match group.as_str() {
                "LST"  => ObjGroup::Lst,
                "DICT" => ObjGroup::Dict,
                "TNSR" => ObjGroup::Tnsr,
                "CSM"  => ObjGroup::Csm { primary: Box::new(ObjGroup::Nrml), extensions: vec![] },
                _      => ObjGroup::Nrml,
            };
            let id = exec.family_registry.create_node(
                pa_id, pb_id, NodeRole::Child, obj_group, None, None,
            ).map_err(|e| anyhow::anyhow!("{}", e))?;
            Ok(Value::FamilyNode { id, mutable: *mutable })
        },

        Expr::DoesParentExist { target, .. } => {
            let tv = eval_expr(exec, target)?;
            let fam_id = match tv {
                Value::FamilyNode { id, .. } => id,
                _ => return Err(anyhow::anyhow!("DOES_PARENT_EXIST requires a family node value")),
            };
            let exists = exec.family_registry.does_parent_exist(fam_id);
            Ok(Value::Bool(exists))
        },
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Private helpers used only inside this module
// ─────────────────────────────────────────────────────────────────────────────

/// Convert a parser `Expr` to a `semantics::ExprSimple` for the constraint engine.
///
/// Mirrors the private `Executor::expr_to_simple`.  Kept here so that the
/// `Statement::Constraint` arm above can call it without going through Executor.
fn expr_to_simple(e: &Expr) -> ExprSimple {
    match e {
        Expr::Identifier(id)    => ExprSimple::Identifier(id.name.clone()),
        Expr::Number(n, _)      => ExprSimple::Number(*n),
        Expr::String(s, _)      => ExprSimple::Raw(s.clone()),
        Expr::Bool(b, _)        => ExprSimple::Raw(b.to_string()),
        Expr::None(_)           => ExprSimple::Raw("none".to_string()),
        Expr::Binary { left, right, .. } => {
            let l = match &**left  { Expr::Identifier(id) => id.name.clone(), Expr::Number(n, _) => n.to_string(), o => format!("{:?}", o) };
            let r = match &**right { Expr::Identifier(id) => id.name.clone(), Expr::Number(n, _) => n.to_string(), o => format!("{:?}", o) };
            ExprSimple::Raw(format!("{} ? {}", l, r))
        }
        other => ExprSimple::Raw(format!("{:?}", other)),
    }
}


// short expression summary helper (best-effort)
fn short_expr_summary(expr: &crate::parser::Expr) -> String {
    match expr {
        crate::parser::Expr::Identifier(id) => format!("Ident({})", id.name),
        crate::parser::Expr::Call { callee, .. } => format!("Call({})", short_expr_summary(&*callee)),
        crate::parser::Expr::Lambda(_, _) => "Lambda".to_string(),
        crate::parser::Expr::Number(n, _) => format!("Number({})", n),
        crate::parser::Expr::String(s, _) => format!("String(len={})", s.len()),
        _ => "Expr".to_string(),
    }
}

/* SAFE SHIM: ensure executor can call take_recent_events() even if instrumentation is absent.
   This returns an empty Vec when no recent-events buffer is present. Remove or replace
   with the real implementation when you reintroduce the ring buffer. */
pub fn take_recent_events() -> Vec<String> {
    Vec::new()
}
