// Access the VERBOSE_DEBUG flag from main
pub fn is_verbose_debug() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    extern "Rust" {
        static VERBOSE_DEBUG: AtomicBool;
    }
    unsafe { VERBOSE_DEBUG.load(Ordering::Relaxed) }
}
use std::sync::atomic::Ordering;
// use crate::bin::pasta::VERBOSE_DEBUG; // Disabled to fix import error
// src/interpreter/executor.rs
// Executor with stdio and basic graphics builtins (feature-gated image support)

use std::collections::HashMap;
use std::fs;
use std::io::{self};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};
use anyhow::{anyhow, Result};

use crate::interpreter::environment::{Environment, Value, RuntimeTensor};
use crate::runtime::rng::Rng;
use crate::runtime::strainer::Strainer;
use crate::parser::ast::*;
use crate::lexer::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::interpreter::errors::{RuntimeError, RuntimeErrorKind, TraceFrame, Traceback};
use crate::interpreter::ai_network;
use crate::interpreter::shell::Shell;
use crate::semantics::{ConstraintEngine, PriorityGraph, Relation, ExprSimple, ConstraintExpr};

const DEFAULT_WHILE_LIMIT: usize = 1_000_000;

#[derive(Debug)]
pub struct Executor {
        /// If true, print diagnostics and header loading notes
        pub verbose: bool,
    pub env: Environment,
    pub priorities: PriorityGraph,
    pub constraints: ConstraintEngine,
    pub diagnostics: Vec<String>,

    pub gfx_canvases: HashMap<String, (usize, usize, Vec<u8>)>,
    pub next_canvas_id: usize,

    /// Per-executor RNG instance (prefers hardware RNG when available).
    pub rng: Rng,

    /// Maximum number of iterations allowed per target in a `WHILE` loop.
    /// Defaults to `DEFAULT_WHILE_LIMIT`. Set to 0 for unlimited (not recommended).
    pub while_limit: usize,

    /// Stored function definitions: name -> body statements
    pub functions: HashMap<String, Vec<Statement>>,
    /// The per-executor garbage collector.  Each meatball / executor will
    /// eventually get its own heap so that we can collect on a per-thread
    /// basis; for now we just embed a single `Strainer` here.
    pub gc: Strainer,

    /// Stack of runtime frames used for traceback generation.  We push a
    /// frame each time we enter a user-defined block or evaluate a
    /// statement; this allows error reports to include the call stack.
    pub traceback: Traceback,

    /// Shell instance for file system and system operations
    pub shell: Shell,
}

impl Executor {
        fn debug_trace(&self, msg: &str) {
            // VERBOSE_DEBUG removed for build compatibility
        }
    pub fn new() -> Self {
        let mut exe = Self {
            env: Environment::new(),
            priorities: PriorityGraph::new(),
            constraints: ConstraintEngine::new(),
            diagnostics: Vec::new(),
            gfx_canvases: HashMap::new(),
            next_canvas_id: 1,
            rng: Rng::new(),
            while_limit: DEFAULT_WHILE_LIMIT,
            functions: HashMap::new(),
            gc: Strainer::new(),
            traceback: Traceback::default(),
            shell: Shell::default(),
            verbose: false,
        };

        // Load standard headers if present
        exe.load_header_if_exists("src/stdio.ph");
        exe.load_header_if_exists("stdio.ph");
        exe.load_header_if_exists("src/pasta_G.ph");

        // Auto-load headers from headers/ directory (idempotent)
        exe.load_headers_from_dir("headers");

        // Legacy fallback
        exe.load_header_if_exists("pasta_G.ph");

        // Ensure headers dir again (safe no-op)
        exe.load_headers_from_dir("headers");

        exe
    }

    /// Push a new frame onto the traceback stack.  `ctx` should describe
    /// what is being executed (e.g. "statement", "function call").
    fn push_frame(&mut self, span: Span, ctx: impl Into<String>) {
        self.traceback.0.push(TraceFrame { span, context: ctx.into() });
    }

    /// Pop the most recent frame.  Safe to call even if the stack is empty.
    fn pop_frame(&mut self) {
        self.traceback.0.pop();
    }

    /// Override the per-target WHILE iteration cap. Pass `0` for unlimited.
    pub fn set_while_limit(&mut self, limit: usize) {
        self.while_limit = limit;
    }

    /// Trigger a garbage collection scan using the current environment as the
    /// root set.  Returns the number of objects reclaimed.  This is exposed
    /// publicly primarily for testing and for callers that want more control
    /// over when collections happen; `execute_program` already invokes this
    /// automatically after each statement.
    pub fn collect_garbage(&mut self) -> usize {
        let roots = self.env.all_values();
        self.gc.collect_with_roots(&roots)
    }

    fn load_header_if_exists(&mut self, path: &str) {
        let p = Path::new(path);
        if !p.exists() {
            return;
        }
        match fs::read_to_string(p) {
            Ok(src) => {
                match Lexer::new(&src).lex_result() {
                    Ok(tokens) => {
                        let mut parser = Parser::new(tokens);
                        let program = parser.parse();
                        if let Err(e) = self.execute_repl(&program) {
                            let msg = format!("Failed to execute header {}: {}", path, e);
                            if !msg.contains("Undefined variable") {
                                self.diagnostics.push(msg);
                            }
                        } else {
                            if self.verbose {
                                self.diagnostics.push(format!("Loaded header {}", path));
                            }
                        }
                    }
                    Err(e) => {
                        self.diagnostics.push(format!("Lex error loading {}: {}:{}", path, e.line, e.col));
                    }
                }
            }
            Err(e) => {
                self.diagnostics.push(format!("I/O error reading {}: {}", path, e));
            }
        }
    }

    /// Recursively follow a `Value::Heap` handle until a non-heap value is
    /// reached (or return `Value::None` if the handle is invalid).  This is
    /// used throughout evaluation to hide the indirection from GC objects.
    fn deref(&self, mut v: Value) -> Value {
        while let Value::Heap(id) = v {
            if let Some(inner) = self.gc.get(id) {
                v = inner.clone();
            } else {
                return Value::None;
            }
        }
        v
    }

    /// Load all `.ph` files from a directory (sorted).
    fn load_headers_from_dir(&mut self, dir: &str) {
        let p = Path::new(dir);
        if !p.exists() || !p.is_dir() {
            return;
        }
        let mut names: Vec<String> = Vec::new();
        if let Ok(entries) = fs::read_dir(p) {
            for e in entries.flatten() {
                if let Some(n) = e.file_name().to_str() {
                    if n.ends_with(".ph") {
                        names.push(n.to_string());
                    }
                }
            }
        }
        names.sort();
        for n in names {
            let path = format!("{}/{}", dir, n);
            self.load_header_if_exists(&path);
        }
    }

    pub fn execute_program(&mut self, program: &Program) -> Result<()> {
        for stmt in &program.statements {
            if let Err(e) = self.execute_statement(stmt) {
                if self.verbose && is_verbose_debug() {
                    for msg in &self.diagnostics {
                        println!("note: {}", msg);
                    }
                }
                return Err(e);
            }
            let _ = self.collect_garbage();
        }
        if let Err(e) = self.constraints.validate_all() {
            let msg = format!("Constraint validation failed: {}", e);
            if self.verbose && is_verbose_debug() {
                self.diagnostics.push(msg.clone());
                for msg in &self.diagnostics {
                    println!("note: {}", msg);
                }
            }
            return Err(anyhow!(msg));
        }
        Ok(())
    }

    pub fn execute_repl(&mut self, program: &Program) -> Result<()> {
        for stmt in &program.statements {
            self.execute_statement(stmt)?;
        }
        Ok(())
    }

    pub fn execute_statement(&mut self, stmt: &Statement) -> Result<Option<Value>> {
                    self.debug_trace(&format!("Executing statement: {:?}", stmt));
                if self.verbose && is_verbose_debug() {
                    println!("[DEBUG] Executing statement: {:?}", stmt);
                    println!("[DEBUG] Current environment: {:?}", self.env.get_scopes());
                }
        // record current span and push on traceback
        let span = match stmt {
            Statement::Assignment { span, .. }
            | Statement::FunctionDef { span, .. }
            | Statement::DoBlock { span, .. }
            | Statement::WhileBlock { span, .. }
            | Statement::PriorityOverride { span, .. }
            | Statement::Constraint { span, .. }
            | Statement::Print { span, .. }
            | Statement::If { span, .. }
            | Statement::End { span }
            | Statement::ExprStmt { span, .. }
            | Statement::Other { span, .. } => span.clone(),
        };
        self.push_frame(span.clone(), "statement");
        // Note: pop_frame is handled explicitly before every `return` in the
        // branches that may exit early.  This avoids borrow conflicts while
        // still ensuring the traceback stack is cleaned up.

        match stmt {
            Statement::Assignment { target, value, span: _ } => {
                                                                self.debug_trace(&format!("Assignment: {} = {:?}", &target.name, value));
                                                if self.verbose && is_verbose_debug() {
                                                    println!("[DEBUG] Environment after assignment:");
                                                    self.env.debug_print();
                                                }
                                if self.verbose && is_verbose_debug() {
                                    println!("[DEBUG] Assignment target: {}", &target.name);
                                    println!("[DEBUG] Assignment value: {:?}", value);
                                }
                let v = self.eval_expr(value)?;
                if self.verbose && is_verbose_debug() {
                    println!("[DIAG] Assigning {} = {:?}", &target.name, v);
                    println!("[DEBUG] DEF assignment: storing {} in global scope", &target.name);
                }
                self.env.set_global(&target.name, v.clone());
                Ok(None)
            }

            // Function definition: store the function body for later invocation
            Statement::FunctionDef { name, body, span: _ } => {
                                                self.debug_trace(&format!("FunctionDef: {} = {:?}", name.name, body));
                                if self.verbose && is_verbose_debug() {
                                    println!("[DEBUG] FunctionDef: {}", name.name);
                                    println!("[DEBUG] Function body: {:?}", body);
                                }
                // If the function body is a single WHILE block, store as a lambda in the environment
                if body.len() == 1 {
                    if let Statement::WhileBlock { .. } = &body[0] {
                        self.env.set_local(name.name.clone(), Value::Lambda(body.clone()));
                        return Ok(None);
                    }
                }
                let body_clone = body.clone();
                self.functions.insert(name.name.clone(), body_clone);
                Ok(None)
            }

            // ── Counted DO block ─────────────────────────────────────────────
            Statement::DoBlock { targets, alias, repeats, body, span } => {
                                                                                                if self.verbose && is_verbose_debug() {
                                                                                                    println!("[DEBUG] Global scope before DO:");
                                                                                                    if let Some(global) = self.env.get_scopes().get(0) {
                                                                                                        println!("[DEBUG] Global scope vars: {:?}", global.get_vars());
                                                                                                    }
                                                                                                }
                                                                                self.debug_trace(&format!("DoBlock: targets={:?} alias={:?} repeats={:?} body={:?}", targets, alias, repeats, body));
                                                                if self.verbose && is_verbose_debug() {
                                                                    println!("[DEBUG] Environment before resolving DO block targets:");
                                                                    self.env.debug_print();
                                                                }
                                                if self.verbose && is_verbose_debug() {
                                                    println!("[DEBUG] DoBlock targets: {:?}", targets);
                                                    println!("[DEBUG] DoBlock alias: {:?}", alias);
                                                    println!("[DEBUG] DoBlock repeats: {:?}", repeats);
                                                    println!("[DEBUG] DoBlock body: {:?}", body);
                                                }
                                if self.verbose && is_verbose_debug() {
                                    println!("[DIAG] Environment before DO: ");
                                    for (i, scope) in self.env.get_scopes().iter().enumerate() {
                                        println!("[DIAG]  Scope {}: {:?}", i, scope.get_vars());
                                    }
                                }
                let counts = self.resolve_repeat_counts(targets.len(), repeats.as_ref(), span)?;

                for (i, target_id) in targets.iter().enumerate() {
                    if self.verbose && is_verbose_debug() {
                        println!("[DIAG] DO block target: {}", target_id.name);
                        let global_val = self.env.get_scopes().get(0).and_then(|s| s.get_vars().get(&target_id.name));
                        if let Some(val) = global_val {
                            println!("[DIAG] Global scope value: {:?}", val);
                        }
                        if let Some(val) = self.env.get(&target_id.name) {
                            println!("[DIAG] Target value type: {:?}", val);
                        } else {
                            println!("[DIAG] Target value: <not found>");
                        }
                    }
                    let repeat_count = counts[i];

                    // Always check global scope first for lambda
                    let lambda_stmts_opt = {
                        let global_val = self.env.get_scopes().get(0).and_then(|s| s.get_vars().get(&target_id.name));
                        if let Some(Value::Lambda(stmts)) = global_val {
                            Some(stmts.clone())
                        } else {
                            None
                        }
                    };
                    if let Some(stmts) = lambda_stmts_opt {
                        if self.verbose && is_verbose_debug() {
                            println!("[DIAG] DO block: executing as lambda from global scope");
                        }
                        for _ in 0..repeat_count {
                            self.env.push_scope();
                            if let Some(a) = alias {
                                self.env.set_local(a.name.clone(), Value::String(target_id.name.clone()));
                            }
                            if self.verbose && is_verbose_debug() {
                                println!("[DIAG] Lambda stmts: {:?}", stmts);
                                if stmts.len() == 1 {
                                    println!("[DIAG] Lambda body stmt[0]: {:?}", stmts[0]);
                                }
                            }
                            for s in stmts.iter() {
                                self.execute_statement(s)?;
                                let _ = self.collect_garbage();
                            }
                            if let Err(e) = self.env.pop_scope() {
                                self.diagnostics.push(format!("Warning: pop_scope failed after lambda call: {}", e));
                            }
                        }
                        continue;
                    }
                    // Fallback: check all scopes for lambda
                    if let Some(Value::Lambda(stmts)) = self.env.get(&target_id.name) {
                        if self.verbose && is_verbose_debug() {
                            println!("[DIAG] DO block: executing as lambda from fallback scope");
                        }
                        for _ in 0..repeat_count {
                            self.env.push_scope();
                            if let Some(a) = alias {
                                self.env.set_local(a.name.clone(), Value::String(target_id.name.clone()));
                            }
                            if self.verbose && is_verbose_debug() {
                                println!("[DIAG] Lambda stmts: {:?}", stmts);
                                if stmts.len() == 1 {
                                    println!("[DIAG] Lambda body stmt[0]: {:?}", stmts[0]);
                                }
                            }
                            for s in stmts.iter() {
                                self.execute_statement(s)?;
                                let _ = self.collect_garbage();
                            }
                            if let Err(e) = self.env.pop_scope() {
                                self.diagnostics.push(format!("Warning: pop_scope failed after lambda call: {}", e));
                            }
                        }
                        continue;
                    }

                    if let Some(Value::List(items)) = self.env.get(&target_id.name) {
                                                if self.verbose && is_verbose_debug() {
                                                    println!("[DIAG] DO block: executing as list");
                                                }
                        for _ in 0..repeat_count {
                            for item in items.iter() {
                                self.execute_value_as_callable(item, alias, &target_id.name)?;
                                let _ = self.collect_garbage();
                            }
                        }
                        continue;
                    }

                    // Check if target is a defined function
                    if let Some(func_body) = self.functions.get(&target_id.name).cloned() {
                                                if self.verbose {
                                                    println!("[DIAG] DO block: executing as function");
                                                }
                        for _ in 0..repeat_count {
                            self.env.push_scope();
                            if let Some(a) = alias {
                                self.env.set_local(a.name.clone(), Value::String(target_id.name.clone()));
                            }
                            if self.verbose {
                                println!("[DIAG] Function body stmts: {:?}", func_body);
                            }
                            for s in func_body.iter() {
                                self.execute_statement(s)?;
                                let _ = self.collect_garbage();
                            }
                            if let Err(e) = self.env.pop_scope() {
                                self.diagnostics.push(format!("Warning: pop_scope failed after function call: {}", e));
                            }
                        }
                        continue;
                    }

                    // Otherwise treat as logical thread name
                                        if self.verbose {
                                            println!("[DIAG] DO block: executing as thread/other");
                                        }
                    let tid = self.env.define_thread(Some(target_id.name.clone()), 1.0);

                    for _ in 0..repeat_count {
                        self.env.push_scope();
                        if let Some(a) = alias {
                            self.env.set_local(a.name.clone(), Value::String(target_id.name.clone()));
                        }
                        self.env.set_local("_thread_id".to_string(), Value::Number(tid as f64));
                        for s in body.iter() {
                            self.execute_statement(s)?;
                            let _ = self.collect_garbage();
                        }
                        if let Err(e) = self.env.pop_scope() {
                            self.diagnostics.push(format!("Warning: failed to pop scope after DO body: {}", e));
                        }
                    }

                    self.env.remove_thread(tid);
                }

                Ok(None)
            }

            // ── WHILE loop ───────────────────────────────────────────────────
            Statement::WhileBlock { targets, alias, condition, body, span } => {
                                                self.debug_trace(&format!("WhileBlock: targets={:?} alias={:?} condition={:?} body={:?}", targets, alias, condition, body));
                                if self.verbose {
                                    println!("[DEBUG] WhileBlock targets: {:?}", targets);
                                    println!("[DEBUG] WhileBlock alias: {:?}", alias);
                                    println!("[DEBUG] WhileBlock condition: {:?}", condition);
                                    println!("[DEBUG] WhileBlock body: {:?}", body);
                                }
                let limit = if self.while_limit == 0 { usize::MAX } else { self.while_limit };
                if self.verbose {
                    println!("[DIAG] Entering WHILE block. Scopes: {}", self.env.get_scopes().len());
                    for (i, scope) in self.env.get_scopes().iter().enumerate() {
                        println!("[DIAG]  Scope {}: {:?}", i, scope.get_vars());
                    }
                }

                if targets.is_empty() {
                    let mut iterations: usize = 0;
                    loop {
                        if iterations >= limit {
                            self.pop_frame();
                            return Err(self.span_err(
                                span,
                                format!("WHILE loop exceeded iteration limit ({})", limit),
                            ));
                        }
                        if self.verbose {
                            // Print the value of the loop variable if it exists
                            if let Some(var_name) = body.iter().find_map(|s| {
                                if let Statement::Assignment { target, .. } = s {
                                    Some(&target.name)
                                } else {
                                    None
                                }
                            }) {
                                let val = self.env.get(var_name);
                                println!("[DIAG] [pre-cond] Loop variable {} = {:?}", var_name, val);
                            }
                            println!("[DIAG] [pre-cond] Scopes: {}", self.env.get_scopes().len());
                            for (i, scope) in self.env.get_scopes().iter().enumerate() {
                                println!("[DIAG]  Scope {}: {:?}", i, scope.get_vars());
                            }
                        }
                        let cond_val = self.eval_expr(condition)?;
                        if self.verbose {
                            println!("[DIAG] WHILE condition value: {:?}", cond_val);
                        }
                        if !self.value_is_truthy(&cond_val) {
                            break;
                        }
                        if let Some(a) = alias {
                            self.env.push_scope();
                            self.env.set_local(a.name.clone(), Value::String("".to_string()));
                        }
                        for s in body.iter() {
                            self.execute_statement(s)?;
                        }
                        if alias.is_some() {
                            if let Err(e) = self.env.pop_scope() {
                                self.diagnostics.push(format!("Warning: pop_scope failed after WHILE body iteration: {}", e));
                            }
                        }
                        if self.verbose {
                            println!("[DIAG] [post-iter] Scopes: {}", self.env.get_scopes().len());
                            for (i, scope) in self.env.get_scopes().iter().enumerate() {
                                println!("[DIAG]  Scope {}: {:?}", i, scope.get_vars());
                            }
                        }
                        iterations += 1;
                    }
                    self.pop_frame();
                    return Ok(None);
                }

                for target_id in targets.iter() {
                    let lambda_stmts: Option<Vec<Statement>> = self
                        .env
                        .get(&target_id.name)
                        .and_then(|v| if let Value::Lambda(s) = v { Some(s) } else { None })
                        .or_else(|| self.functions.get(&target_id.name).cloned());

                    let tid = self.env.define_thread(Some(target_id.name.clone()), 1.0);
                    let mut iterations: usize = 0;

                    if lambda_stmts.is_some() {
                        loop {
                            if iterations >= limit {
                                self.pop_frame();
                                return Err(self.span_err(
                                    span,
                                    format!("WHILE loop for '{}' exceeded iteration limit ({})", target_id.name, limit),
                                ));
                            }
                            if let Some(a) = alias {
                                self.env.push_scope();
                                self.env.set_local(a.name.clone(), Value::String(target_id.name.clone()));
                                self.env.set_local("_thread_id".to_string(), Value::Number(tid as f64));
                            }
                            let exec_body = lambda_stmts.as_ref().unwrap();
                            for s in exec_body.iter() {
                                self.execute_statement(s)?;
                                let _ = self.collect_garbage();
                            }
                            if alias.is_some() {
                                if let Err(e) = self.env.pop_scope() {
                                    self.diagnostics.push(format!("Warning: pop_scope failed after WHILE body iteration: {}", e));
                                }
                            }
                            iterations += 1;
                            let cond_val = self.eval_expr(condition)?;
                            if !self.value_is_truthy(&cond_val) {
                                break;
                            }
                        }
                    } else {
                        loop {
                            if iterations >= limit {
                                self.pop_frame();
                                return Err(self.span_err(
                                    span,
                                    format!("WHILE loop for '{}' exceeded iteration limit ({})", target_id.name, limit),
                                ));
                            }
                            let cond_val = self.eval_expr(condition)?;
                            if !self.value_is_truthy(&cond_val) {
                                break;
                            }
                            if let Some(a) = alias {
                                self.env.push_scope();
                                self.env.set_local(a.name.clone(), Value::String(target_id.name.clone()));
                                self.env.set_local("_thread_id".to_string(), Value::Number(tid as f64));
                            }
                            for s in body.iter() {
                                self.execute_statement(s)?;
                                let _ = self.collect_garbage();
                            }
                            if alias.is_some() {
                                if let Err(e) = self.env.pop_scope() {
                                    self.diagnostics.push(format!("Warning: pop_scope failed after WHILE body iteration: {}", e));
                                }
                            }
                            iterations += 1;
                        }
                    }

                    self.env.remove_thread(tid);
                }

                Ok(None)
            }

            Statement::PriorityOverride { higher, lower, span: _ } => {
                self.priorities.add_edge(&higher.name, &lower.name);
                Ok(None)
            }

            Statement::Constraint { left, relation, right, constraint, span: _ } => {
                let left_s = self.expr_to_simple(left);
                let right_s = self.expr_to_simple(right);
                let constraint_s = self.expr_to_simple(constraint);
                let rel_enum = relation.as_ref().and_then(|rt| Relation::from_str(&rt.text));
                self.constraints.add_constraint(ConstraintExpr::new(left_s, rel_enum, right_s, constraint_s));
                Ok(None)
            }

            Statement::ExprStmt { expr, span: _ } => {
                // Evaluate for side effects, discard result (do not print)
                match expr {
                    Expr::Raw(_, _) => Ok(None),
                    _ => {
                        let _ = self.eval_expr(expr)?;
                        Ok(None)
                    }
                }
            }

            Statement::Print { expr, span: _ } => {
                let v = self.eval_expr(expr)?;
                self.do_print(&v);
                Ok(Some(v))
            }

            Statement::If { conditions, then_body, else_body, span: _ } => {
                let any_true = conditions.iter().try_fold(false, |acc, cond| {
                    let val = self.eval_expr(cond)?;
                    Ok::<bool, anyhow::Error>(acc || self.value_is_truthy(&val))
                })?;

                if any_true {
                    for stmt in then_body.iter() {
                        self.execute_statement(stmt)?;
                        let _ = self.collect_garbage();
                    }
                } else if let Some(body) = else_body {
                    for stmt in body.iter() {
                        self.execute_statement(stmt)?;
                        let _ = self.collect_garbage();
                    }
                }

                Ok(None)
            }

            Statement::End { .. } => Ok(None),

            Statement::Other { kind, payload, span: _ } => {
                self.diagnostics.push(format!("Unhandled statement kind: {}", kind));
                if let Some(p) = payload { self.diagnostics.push(format!("  payload: {}", p)); }
                Ok(None)
            }
        }
    }

    fn execute_value_as_callable(
        &mut self,
        value: &Value,
        alias: &Option<Identifier>,
        name_hint: &str,
    ) -> Result<()> {
        match value {
            Value::Lambda(stmts) => {
                let stmts = stmts.clone();
                self.env.push_scope();
                if let Some(a) = alias {
                    self.env.set_local(a.name.clone(), Value::String(name_hint.to_string()));
                }
                for s in stmts.iter() {
                    self.execute_statement(s)?;
                    let _ = self.collect_garbage();
                }
                if let Err(e) = self.env.pop_scope() {
                    self.diagnostics.push(format!(
                        "Warning: pop_scope failed after list-item lambda call: {}", e
                    ));
                }
            }
            Value::List(items) => {
                let items = items.clone();
                for item in items.iter() {
                    self.execute_value_as_callable(item, alias, name_hint)?;
                    let _ = self.collect_garbage();
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        match expr {
            Expr::Number(n, _) => Ok(Value::Number(*n)),
            Expr::String(s, _) => {
                let id = self.gc.allocate(Value::String(s.clone()));
                Ok(Value::Heap(id))
            }
            Expr::Bool(b, _) => Ok(Value::Bool(*b)),
            Expr::Identifier(id) => {
                self.env.get(&id.name).ok_or_else(|| anyhow!("Undefined variable '{}'", id.name))
            }
            Expr::Binary { op, left, right, .. } => {
                let lv = self.eval_expr(left)?;
                let rv = self.eval_expr(right)?;
                self.eval_binary(op, lv, rv)
            }
            Expr::Call { callee, args, .. } => {
                if let Expr::Identifier(id) = &**callee {
                    let name = id.name.to_lowercase();
                    let mut argvals = Vec::new();
                    for a in args { argvals.push(self.eval_expr(a)?); }
                    self.call_builtin(&name, argvals)
                } else {
                    Err(anyhow!("Call to non-identifier callee not supported"))
                }
            }
            Expr::List { items, .. } => {
                let mut vals = Vec::new();
                for it in items { vals.push(self.eval_expr(it)?); }
                // heap-allocate the list so it participates in GC
                let id = self.gc.allocate(Value::List(vals));
                Ok(Value::Heap(id))
            }
            Expr::Lambda(stmts, _) => {
                let id = self.gc.allocate(Value::Lambda(stmts.clone()));
                Ok(Value::Heap(id))
            }
            // Expr::Raw should not produce a value for runtime evaluation; treat as None
            Expr::Raw(_, _) => Ok(Value::None),
            Expr::TensorBuilder { expr, .. } => {
                self.build_tensor(expr)
            }
            Expr::Index { .. } => {
                // Indexing not implemented yet - runtime error
                Err(anyhow!("Index expression not supported"))
            }
            // unreachable pattern removed to silence warning
        }
    }

    // ── Truthiness ────────────────────────────────────────────────────────────

    fn value_is_truthy(&self, v: &Value) -> bool {
        // always operate on the dereferenced value
        let v = self.deref(v.clone());
        match &v {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Tensor(t) => t.numel() > 0,
            Value::None => false,
            Value::Lambda(_) => true,
            Value::Heap(_) => false, // should not happen after deref
        }
    }

    // ── Builtins ──────────────────────────────────────────────────────────────

    fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
                // Python-style builtins
                if name == "type" {
                    if args.len() != 1 {
                        return Err(anyhow!("type expects 1 argument"));
                    }
                    let t = match &args[0] {
                        Value::Number(_) => "number",
                        Value::String(_) => "string",
                        Value::Bool(_) => "bool",
                        Value::List(_) => "list",
                        Value::Tensor(_) => "tensor",
                        Value::Lambda(_) => "lambda",
                        Value::None => "none",
                        Value::Heap(_) => "heap",
                    };
                    return Ok(Value::String(t.to_string()));
                }
                if name == "str" {
                    if args.len() != 1 {
                        return Err(anyhow!("str expects 1 argument"));
                    }
                    return Ok(Value::String(Executor::value_to_string(&args[0])));
                }
        // eagerly dereference any heap handles so the individual builtin
        // implementations never have to consider `Value::Heap` cases.
        let args: Vec<Value> = args.into_iter().map(|v| self.deref(v)).collect();

        match name {
            "__pasta_stdin_readline" | "stdin_readline" => {
                let mut buf = String::new();
                let n = io::stdin().read_line(&mut buf)?;
                if n == 0 {
                    Ok(Value::None)
                } else {
                    while buf.ends_with('\n') || buf.ends_with('\r') { buf.pop(); }
                    Ok(Value::String(buf))
                }
            }

            // --- header scaffolding stubs ---
            "__pasta_memory_alloc" => {
                if args.len() != 1 { return Err(anyhow!("memory_alloc expects 1 arg")); }
                match &args[0] {
                    Value::Number(n) => Ok(Value::String(format!("mem://{}", *n as usize))),
                    _ => Err(anyhow!("memory_alloc size must be number")),
                }
            }
            "__pasta_memory_free" => Ok(Value::None),
            "__pasta_sys_env" => {
                if args.len() != 1 { return Err(anyhow!("sys.env expects 1 arg")); }
                match &args[0] {
                    Value::String(k) => Ok(Value::String(std::env::var(k).unwrap_or_default())),
                    _ => Ok(Value::None),
                }
            }
            "__pasta_time_now_ms" => {
                let ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64;
                Ok(Value::Number(ms))
            }
            "__pasta_rand_int" => {
                // Backwards-compatible stub: small pseudo-random value
                let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().subsec_nanos();
                let v = (nanos as u32 % 0x7fff) as f64;
                Ok(Value::Number(v))
            }

            /* Random utilities (prefer hardware RNG, fallback to PRNG) */
            "rand.int" => {
                // rand.int() -> number
                // rand.int(max) -> integer in [0, max)
                // rand.int(min, max) -> integer in [min, max)
                match args.len() {
                    0 => {
                        let v = self.rng.next_u64();
                        Ok(Value::Number((v as i64 & 0x7FFF_FFFF) as f64))
                    }
                    1 => {
                        match &args[0] {
                            Value::Number(n) => {
                                let max = if *n <= 0.0 { 0 } else { *n as u64 };
                                if max == 0 { return Ok(Value::Number(0.0)); }
                                let v = self.rng.next_u64() % max;
                                Ok(Value::Number(v as f64))
                            }
                            _ => Err(anyhow!("rand.int: expected numeric argument")),
                        }
                    }
                    2 => {
                        match (&args[0], &args[1]) {
                            (Value::Number(a), Value::Number(b)) => {
                                let min = *a as i64;
                                let max = *b as i64;
                                if max <= min { return Ok(Value::Number(min as f64)); }
                                let range = (max - min) as u64;
                                let v = (self.rng.next_u64() % range) as i64 + min;
                                Ok(Value::Number(v as f64))
                            }
                            _ => Err(anyhow!("rand.int: expected two numeric arguments")),
                        }
                    }
                    _ => Err(anyhow!("rand.int: expected 0,1 or 2 arguments")),
                }
            }

            "rand.ls" => {
                // rand.ls([n]) -> list of n random floats in [0,1)
                let n = if args.is_empty() { 8usize } else {
                    match &args[0] { Value::Number(x) => *x as usize, _ => return Err(anyhow!("rand.ls: expected numeric length")), }
                };
                let mut out = Vec::with_capacity(n);
                for _ in 0..n {
                    let v = self.rng.next_u64();
                    let f = (v as f64) / (u64::MAX as f64);
                    out.push(Value::Number(f));
                }
                Ok(Value::List(out))
            }

            "tensor.rand" | "tensor_rand" => {
                // tensor.rand(shape) → random float tensor using LCG PRNG seeded from system time
                if args.len() != 1 { return Err(anyhow!("tensor.rand expects 1 argument (shape)")); }
                let shape = match &args[0] {
                    Value::List(dims) => {
                        let mut s = Vec::with_capacity(dims.len());
                        for d in dims {
                            match d {
                                Value::Number(n) => s.push(n.round() as usize),
                                _ => return Err(anyhow!("tensor.rand: shape dimensions must be numbers")),
                            }
                        }
                        s
                    }
                    Value::Number(n) => vec![n.round() as usize],
                    _ => return Err(anyhow!("tensor.rand: argument must be a list or number")),
                };
                let numel: usize = shape.iter().product();
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as u64;
                let mut state: u64 = seed ^ 0xdeadbeef_cafebabe;
                let data: Vec<f64> = (0..numel).map(|_| {
                    state = state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                    (state >> 33) as f64 / (u32::MAX as f64)
                }).collect();
                return Ok(Value::Tensor(RuntimeTensor::new(shape, "float32".to_string(), data)));
            }

            "tensor.eye" | "tensor_eye" => {
                // tensor.eye(n) → n×n identity matrix
                if args.len() != 1 { return Err(anyhow!("tensor.eye expects 1 argument (n)")); }
                let n = match &args[0] {
                    Value::Number(x) => x.round() as usize,
                    _ => return Err(anyhow!("tensor.eye argument must be a number")),
                };
                let mut data = vec![0.0f64; n * n];
                for i in 0..n { data[i * n + i] = 1.0; }
                return Ok(Value::Tensor(RuntimeTensor::new(vec![n, n], "float32".to_string(), data)));
            }

            "tensor.from_list" | "tensor_from_list" => {
                // tensor.from_list([1, 2, 3]) → 1D tensor
                if args.len() != 1 { return Err(anyhow!("tensor.from_list expects 1 argument")); }
                // we deliberately match all variants to keep the compiler happy
                return match &args[0] {
                    Value::List(items) => {
                        let mut data = Vec::with_capacity(items.len());
                        let mut has_float = false;
                        for item in items {
                            match item {
                                Value::Number(n) => {
                                    if n.fract() != 0.0 { has_float = true; }
                                    data.push(*n);
                                }
                                other => return Err(anyhow!("tensor.from_list: non-numeric element: {:?}", other)),
                            }
                        }
                        let dtype = if has_float { "float32" } else { "int32" }.to_string();
                        let len = data.len();
                        Ok(Value::Tensor(RuntimeTensor::new(vec![len], dtype, data)))
                    }
                    Value::Tensor(_) => Err(anyhow!("tensor.from_list: expected a list, got a tensor")),
                    _ => Err(anyhow!("tensor.from_list expects a list")),
                };
            }

            "tensor.shape" | "tensor_shape" => {
                if args.len() != 1 { return Err(anyhow!("tensor.shape expects 1 argument")); }
                return match &args[0] {
                    Value::Tensor(t) => {
                        let shape: Vec<Value> = t.shape.iter().map(|&s| Value::Number(s as f64)).collect();
                        Ok(Value::List(shape))
                    }
                    _ => Err(anyhow!("tensor.shape expects a tensor")),
                };
            }

            "tensor.dtype" | "tensor_dtype" => {
                if args.len() != 1 { return Err(anyhow!("tensor.dtype expects 1 argument")); }
                return match &args[0] {
                    Value::Tensor(t) => Ok(Value::String(t.dtype.clone())),
                    _ => Err(anyhow!("tensor.dtype expects a tensor")),
                };
            }

            // Additional tensor utilities
            "tensor.sum" | "tensor_sum" => {
                if args.len() != 1 { return Err(anyhow!("tensor.sum expects 1 argument")); }
                return match &args[0] {
                    Value::Tensor(t) => {
                        let total: f64 = t.data.iter().copied().sum();
                        Ok(Value::Number(total))
                    }
                    _ => Err(anyhow!("tensor.sum expects a tensor")),
                };
            }

            "tensor.mean" | "tensor_mean" => {
                if args.len() != 1 { return Err(anyhow!("tensor.mean expects 1 argument")); }
                return match &args[0] {
                    Value::Tensor(t) => {
                        let total: f64 = t.data.iter().copied().sum();
                        let cnt = t.data.len() as f64;
                        Ok(Value::Number(if cnt == 0.0 { 0.0 } else { total / cnt }))
                    }
                    _ => Err(anyhow!("tensor.mean expects a tensor")),
                };
            }

            "ai.relu" | "ai_relu" => {
                // ai.relu(tensor) → apply ReLU activation element-wise
                if args.len() != 1 { return Err(anyhow!("ai.relu expects 1 argument")); }
                match &args[0] {
                    Value::Tensor(t) => {
                        let activated = ai_network::AILayer::relu(&t.data);
                        return Ok(Value::Tensor(RuntimeTensor::new(t.shape.clone(), t.dtype.clone(), activated)));
                    }
                    _ => return Err(anyhow!("ai.relu expects a tensor")),
                }
            }

            "ai.softmax" | "ai_softmax" => {
                // ai.softmax(tensor) → apply softmax
                if args.len() != 1 { return Err(anyhow!("ai.softmax expects 1 argument")); }
                match &args[0] {
                    Value::Tensor(t) => {
                        let probs = ai_network::AILayer::softmax(&t.data);
                        return Ok(Value::Tensor(RuntimeTensor::new(t.shape.clone(), t.dtype.clone(), probs)));
                    }
                    _ => return Err(anyhow!("ai.softmax expects a tensor")),
                }
            }

            "ai.loss.mse" | "ai_loss_mse" => {
                // ai.loss.mse(prediction_tensor, target_tensor) → MSE loss value
                if args.len() != 2 { return Err(anyhow!("ai.loss.mse expects 2 arguments")); }
                match (&args[0], &args[1]) {
                    (Value::Tensor(pred), Value::Tensor(target)) => {
                        if pred.data.len() != target.data.len() {
                            return Err(anyhow!("MSE loss: tensors must have same size"));
                        }
                        let loss = ai_network::mse_loss(&pred.data, &target.data)?;
                        return Ok(Value::Number(loss));
                    }
                    _ => return Err(anyhow!("ai.loss.mse expects two tensors")),
                }
            }

            "ai.loss.crossentropy" | "ai_loss_crossentropy" => {
                // ai.loss.crossentropy(logits_tensor, target_class) → cross-entropy loss
                if args.len() != 2 { return Err(anyhow!("ai.loss.crossentropy expects 2 arguments")); }
                match (&args[0], &args[1]) {
                    (Value::Tensor(logits), Value::Number(target)) => {
                        let target_class = target.round() as usize;
                        let loss = ai_network::cross_entropy_loss(&logits.data, target_class)?;
                        return Ok(Value::Number(loss));
                    }
                    _ => return Err(anyhow!("ai.loss.crossentropy expects (logits_tensor, target_class_number)")),
                }
            }

            "ai.tensor_to_list" | "ai_tensor_to_list" => {
                // Convert a tensor to a list for inspection
                if args.len() != 1 { return Err(anyhow!("ai.tensor_to_list expects 1 argument")); }
                match &args[0] {
                    Value::Tensor(t) => {
                        let list: Vec<Value> = t.data.iter().map(|&v| Value::Number(v)).collect();
                        return Ok(Value::List(list));
                    }
                    _ => return Err(anyhow!("ai.tensor_to_list expects a tensor")),
                }
            }

            "ai.list_to_tensor" | "ai_list_to_tensor" => {
                // Convert a list to a tensor
                if args.len() != 1 { return Err(anyhow!("ai.list_to_tensor expects 1 argument")); }
                match &args[0] {
                    Value::List(items) => {
                        let mut data = Vec::with_capacity(items.len());
                        for item in items {
                            match item {
                                Value::Number(n) => data.push(n.clone()),
                                _ => return Err(anyhow!("ai.list_to_tensor: list must contain only numbers")),
                            }
                        }
                        let len = data.len();
                        return Ok(Value::Tensor(RuntimeTensor::new(vec![len], "float32".to_string(), data)));
                    }
                    _ => return Err(anyhow!("ai.list_to_tensor expects a list")),
                }
            }

            // ── Shell & File System ────────────────────────────────────────────

            "shell.pwd" | "shell_pwd" | "pwd" => {
                // Get current working directory
                return Ok(Value::String(self.shell.pwd()));
            }

            "shell.cd" | "shell_cd" | "cd" => {
                // Change directory
                if args.len() != 1 { return Err(anyhow!("cd expects 1 argument (directory)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.cd(path)
                            .map(Value::String)
                    }
                    _ => Err(anyhow!("cd: path must be a string")),
                }
            }

            "shell.ls" | "shell_ls" | "ls" => {
                // List directory contents
                let path = if args.len() > 1 {
                    return Err(anyhow!("ls expects 0 or 1 argument"));
                } else if args.len() == 1 {
                    match &args[0] {
                        Value::String(p) => Some(p.as_str()),
                        _ => return Err(anyhow!("ls: path must be a string")),
                    }
                } else {
                    None
                };

                self.shell.ls(path)
                    .map(Value::List)
            }

            "shell.ls_long" | "shell_ls_long" | "ls_long" => {
                // List directory with detailed info
                let path = if args.len() > 1 {
                    return Err(anyhow!("ls_long expects 0 or 1 argument"));
                } else if args.len() == 1 {
                    match &args[0] {
                        Value::String(p) => Some(p.as_str()),
                        _ => return Err(anyhow!("ls_long: path must be a string")),
                    }
                } else {
                    None
                };

                self.shell.ls_long(path)
                    .map(Value::List)
            }

            "shell.mkdir" | "shell_mkdir" | "mkdir" => {
                // Create directory
                if args.is_empty() || args.len() > 2 {
                    return Err(anyhow!("mkdir expects 1-2 arguments (path, [parents])"));
                }
                let path = match &args[0] {
                    Value::String(p) => p,
                    _ => return Err(anyhow!("mkdir: path must be a string")),
                };
                let parents = if args.len() == 2 {
                    match &args[1] {
                        Value::Bool(b) => b.clone(),
                        Value::Number(n) => n.clone() != 0.0,
                        _ => return Err(anyhow!("mkdir: parents must be a boolean")),
                    }
                } else {
                    false
                };

                self.shell.mkdir(path, parents)
                    .map(Value::String)
            }

            "shell.rm" | "shell_rm" | "rm" => {
                // Remove file
                if args.len() != 1 { return Err(anyhow!("rm expects 1 argument (file)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.rm(path)
                            .map(Value::String)
                    }
                    _ => Err(anyhow!("rm: path must be a string")),
                }
            }

            "shell.rmdir" | "shell_rmdir" | "rmdir" => {
                // Remove empty directory
                if args.len() != 1 { return Err(anyhow!("rmdir expects 1 argument (directory)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.rmdir(path)
                            .map(Value::String)
                    }
                    _ => Err(anyhow!("rmdir: path must be a string")),
                }
            }

            "shell.rmdir_r" | "shell_rmdir_recursive" | "rm_r" => {
                // Remove directory recursively
                if args.len() != 1 { return Err(anyhow!("rm_r expects 1 argument (path)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.rmdir_recursive(&path)
                            .map(Value::String)
                    }
                    _ => Err(anyhow!("rm_r: path must be a string")),
                }
            }

            "shell.touch" | "shell_touch" | "touch" => {
                // Create or update file
                if args.len() != 1 { return Err(anyhow!("touch expects 1 argument (file)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.touch(&path)
                            .map(Value::String)
                    }
                    _ => Err(anyhow!("touch: path must be a string")),
                }
            }

            "shell.cp" | "shell_cp" | "cp" => {
                // Copy file
                if args.len() != 2 { return Err(anyhow!("cp expects 2 arguments (from, to)")); }
                let from = match &args[0] {
                    Value::String(s) => s,
                    _ => return Err(anyhow!("cp: from must be a string")),
                };
                let to = match &args[1] {
                    Value::String(s) => s,
                    _ => return Err(anyhow!("cp: to must be a string")),
                };

                self.shell.cp(&from, &to)
                    .map(Value::String)
            }

            "shell.mv" | "shell_mv" | "mv" => {
                // Move/rename file
                if args.len() != 2 { return Err(anyhow!("mv expects 2 arguments (from, to)")); }
                let from = match &args[0] {
                    Value::String(s) => s,
                    _ => return Err(anyhow!("mv: from must be a string")),
                };
                let to = match &args[1] {
                    Value::String(s) => s,
                    _ => return Err(anyhow!("mv: to must be a string")),
                };

                self.shell.mv(&from, &to)
                    .map(Value::String)
            }

            "shell.exists" | "shell_exists" => {
                // Check if path exists
                if args.len() != 1 { return Err(anyhow!("exists expects 1 argument (path)")); }
                match &args[0] {
                    Value::String(path) => {
                        return Ok(Value::Bool(self.shell.exists(&path)));
                    }
                    _ => return Err(anyhow!("exists: path must be a string")),
                }
            }

            "shell.is_file" | "shell_is_file" => {
                // Check if path is a file
                if args.len() != 1 { return Err(anyhow!("is_file expects 1 argument (path)")); }
                match &args[0] {
                    Value::String(path) => {
                        return Ok(Value::Bool(self.shell.is_file(&path)));
                    }
                    _ => return Err(anyhow!("is_file: path must be a string")),
                }
            }

            "shell.is_dir" | "shell_is_dir" => {
                // Check if path is a directory
                if args.len() != 1 { return Err(anyhow!("is_dir expects 1 argument (path)")); }
                match &args[0] {
                    Value::String(path) => {
                        return Ok(Value::Bool(self.shell.is_dir(&path)));
                    }
                    _ => return Err(anyhow!("is_dir: path must be a string")),
                }
            }

            "shell.file_size" | "shell_file_size" => {
                // Get file size in bytes
                if args.len() != 1 { return Err(anyhow!("file_size expects 1 argument (path)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.file_size(&path)
                            .map(|size| Value::Number(size as f64))
                    }
                    _ => Err(anyhow!("file_size: path must be a string")),
                }
            }

            "shell.realpath" | "shell_realpath" => {
                // Get absolute path
                if args.len() != 1 { return Err(anyhow!("realpath expects 1 argument (path)")); }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.realpath(&path)
                            .map(Value::String)
                    }
                    _ => Err(anyhow!("realpath: path must be a string")),
                }
            }

            // ── Graphics ──────────────────────────────────────────────────────

            "__pasta_g_create_canvas" => {
                if args.len() != 2 { return Err(anyhow!("create_canvas expects width,height")); }
                let w = match &args[0] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("width must be number")) };
                let h = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("height must be number")) };
                let id = format!("canvas{}", self.next_canvas_id);
                self.next_canvas_id += 1;
                let buf = vec![0u8; w * h * 4];
                self.gfx_canvases.insert(id.clone(), (w, h, buf));
                return Ok(Value::String(id));
            }

            "__pasta_g_destroy_canvas" => {
                if args.len() != 1 { return Err(anyhow!("destroy_canvas expects canvas id")); }
                let cid = match &args[0] { Value::String(s) => s.clone(), _ => return Err(anyhow!("canvas id must be string")) };
                self.gfx_canvases.remove(&cid);
                return Ok(Value::None);
            }

            "__pasta_g_clear" => {
                if args.len() < 2 { return Err(anyhow!("clear expects canvas id and color")); }
                let cid = match &args[0] { Value::String(s) => s.clone(), _ => return Err(anyhow!("canvas id must be string")) };
                if let Some((_, _, buf)) = self.gfx_canvases.get_mut(&cid) {
                    let (r, g, b, a) = Executor::parse_color_arg(args.get(1));
                    for px in buf.chunks_exact_mut(4) {
                        px[0] = r; px[1] = g; px[2] = b; px[3] = a;
                    }
                }
                return Ok(Value::None);
            }

            "__pasta_g_set_pixel" => {
                if args.len() < 4 { return Err(anyhow!("set_pixel expects cid,x,y,color")); }
                let cid = match &args[0] { Value::String(s) => s.clone(), _ => return Err(anyhow!("canvas id must be string")) };
                let x = match &args[1] { Value::Number(n) => n.round() as isize, _ => return Err(anyhow!("x must be number")) };
                let y = match &args[2] { Value::Number(n) => n.round() as isize, _ => return Err(anyhow!("y must be number")) };
                let (r, g, b, a) = Executor::parse_color_arg(args.get(3));
                if let Some((w, h, buf)) = self.gfx_canvases.get_mut(&cid) {
                    if x >= 0 && y >= 0 && (x as usize) < *w && (y as usize) < *h {
                        let idx = (y as usize) * (*w) + (x as usize);
                        let off = idx * 4;
                        buf[off] = r; buf[off + 1] = g; buf[off + 2] = b; buf[off + 3] = a;
                    }
                }
                return Ok(Value::None);
            }

            "__pasta_g_save" => {
                if args.len() != 2 { return Err(anyhow!("save expects canvas_id and filename")); }
                let cid = match &args[0] { Value::String(s) => s.clone(), _ => return Err(anyhow!("canvas id must be string")) };
                let filename = match &args[1] { Value::String(s) => s.clone(), _ => return Err(anyhow!("filename must be string")) };
                if let Some((w, h, buf)) = self.gfx_canvases.get(&cid) {
                    #[cfg(feature = "image")]
                    {
                        match image::save_buffer(&filename, &buf[..], *w as u32, *h as u32, image::ColorType::Rgba8) {
                            Ok(_) => return Ok(Value::Bool(true)),
                            Err(e) => {
                                self.diagnostics.push(format!("gfx save error: {}", e));
                                return Ok(Value::Bool(false));
                            }
                        }
                    }
                    #[cfg(not(feature = "image"))]
                    {
                        let _ = (w, h, buf, filename);
                        self.diagnostics.push("gfx save requested but 'image' feature not enabled".to_string());
                        return Ok(Value::Bool(false));
                    }
                } else {
                    return Ok(Value::Bool(false));
                }
            }

            "__pasta_g_show" => Ok(Value::None),


            // ================================================================
            // STDLIB BUILTINS — string_*, list_*, math_*, type checks, utils
            // All functions used by stdlib_examples.pa and stdlib.pa modules.
            // ================================================================

            // ── string_length / string_len / string_lenght ──────────────────
            "string_length" | "string_len" | "string_lenght" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_length expects 1 argument"));
                }
                match &args[0] {
                    Value::String(s) => return Ok(Value::Number(s.chars().count() as f64)),
                    Value::List(l)   => return Ok(Value::Number(l.len() as f64)),
                    Value::None      => return Ok(Value::Number(0.0)),
                    other => return Err(anyhow!("string_length: expected string, got {:?}", other)),
                }
            }

            // ── string_concat ────────────────────────────────────────────────
            "string_concat" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_concat expects 2 arguments"));
                }
                let a = Executor::value_to_string(&args[0]);
                let b = Executor::value_to_string(&args[1]);
                Ok(Value::String(a + &b))
            }

            // ── string_concat_with_sep ───────────────────────────────────────
            "string_concat_with_sep" => {
                if args.len() != 3 {
                    return Err(anyhow!("string_concat_with_sep expects 3 arguments (s1, s2, sep)"));
                }
                let a   = Executor::value_to_string(&args[0]);
                let b   = Executor::value_to_string(&args[1]);
                let sep = Executor::value_to_string(&args[2]);
                Ok(Value::String(format!("{}{}{}", a, sep, b)))
            }

            // ── string_repeat ────────────────────────────────────────────────
            "string_repeat" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_repeat expects 2 arguments (s, count)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let n = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("string_repeat: count must be a number")),
                };
                return Ok(Value::String(s.repeat(n)));
            }

            // ── string_pad_left ──────────────────────────────────────────────
            "string_pad_left" => {
                if args.len() != 3 {
                    return Err(anyhow!("string_pad_left expects 3 arguments (s, width, pad_char)"));
                }
                let s     = Executor::value_to_string(&args[0]);
                let width = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("string_pad_left: width must be a number")) };
                let pad   = Executor::value_to_string(&args[2]);
                let pad_ch = pad.chars().next().unwrap_or(' ');
                if s.chars().count() >= width {
                    return Ok(Value::String(s));
                } else {
                    let needed = width - s.chars().count();
                    let padding: String = std::iter::repeat(pad_ch).take(needed).collect();
                    return Ok(Value::String(format!("{}{}", padding, s)));
                }
            }

            // ── string_pad_right ─────────────────────────────────────────────
            "string_pad_right" => {
                if args.len() != 3 {
                    return Err(anyhow!("string_pad_right expects 3 arguments (s, width, pad_char)"));
                }
                let s     = Executor::value_to_string(&args[0]);
                let width = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("string_pad_right: width must be a number")) };
                let pad   = Executor::value_to_string(&args[2]);
                let pad_ch = pad.chars().next().unwrap_or(' ');
                if s.chars().count() >= width {
                    return Ok(Value::String(s));
                } else {
                    let needed = width - s.chars().count();
                    let padding: String = std::iter::repeat(pad_ch).take(needed).collect();
                    return Ok(Value::String(format!("{}{}", s, padding)));
                }
            }

            // ── string_upper ─────────────────────────────────────────────────
            "string_upper" | "upper" => {
                if args.len() != 1 { return Err(anyhow!("string_upper expects 1 argument")); }
                return Ok(Value::String(Executor::value_to_string(&args[0]).to_uppercase()));
            }

            // ── string_lower ─────────────────────────────────────────────────
            "string_lower" | "lower" => {
                if args.len() != 1 { return Err(anyhow!("string_lower expects 1 argument")); }
                return Ok(Value::String(Executor::value_to_string(&args[0]).to_lowercase()));
            }

            // ── string_reverse ───────────────────────────────────────────────
            "string_reverse" => {
                if args.len() != 1 { return Err(anyhow!("string_reverse expects 1 argument")); }
                return Ok(Value::String(Executor::value_to_string(&args[0]).chars().rev().collect()));
            }

            // ── string_trim ──────────────────────────────────────────────────
            "string_trim" | "trim" => {
                if args.len() != 1 { return Err(anyhow!("string_trim expects 1 argument")); }
                return Ok(Value::String(Executor::value_to_string(&args[0]).trim().to_string()));
            }

            // ── string_starts_with ───────────────────────────────────────────
            "string_starts_with" | "starts_with" => {
                if args.len() != 2 { return Err(anyhow!("string_starts_with expects 2 arguments")); }
                let s   = Executor::value_to_string(&args[0]);
                let pre = Executor::value_to_string(&args[1]);
                return Ok(Value::Bool(s.starts_with(&*pre)));
            }

            // ── string_ends_with ─────────────────────────────────────────────
            "string_ends_with" | "ends_with" => {
                if args.len() != 2 { return Err(anyhow!("string_ends_with expects 2 arguments")); }
                let s   = Executor::value_to_string(&args[0]);
                let suf = Executor::value_to_string(&args[1]);
                return Ok(Value::Bool(s.ends_with(&*suf)));
            }

            // ── string_contains ──────────────────────────────────────────────
            "string_contains" => {
                if args.len() != 2 { return Err(anyhow!("string_contains expects 2 arguments")); }
                let s   = Executor::value_to_string(&args[0]);
                let sub = Executor::value_to_string(&args[1]);
                return Ok(Value::Bool(s.contains(&*sub)));
            }

            // ── string_replace ───────────────────────────────────────────────
            "string_replace" | "replace" => {
                if args.len() != 3 { return Err(anyhow!("string_replace expects 3 arguments (s, old, new)")); }
                let s   = Executor::value_to_string(&args[0]);
                let old = Executor::value_to_string(&args[1]);
                let new = Executor::value_to_string(&args[2]);
                return Ok(Value::String(s.replace(&*old, &new)));
            }

            // ── string_split ─────────────────────────────────────────────────
            "string_split" | "split" => {
                if args.len() != 2 { return Err(anyhow!("string_split expects 2 arguments (s, delimiter)")); }
                let s   = Executor::value_to_string(&args[0]);
                let del = Executor::value_to_string(&args[1]);
                let parts: Vec<Value> = s.split(&*del)
                    .map(|p: &str| Value::String(p.to_string()))
                    .collect();
                return Ok(Value::List(parts));
            }

            // ── string_join ──────────────────────────────────────────────────
            "string_join" | "join" => {
                if args.len() != 2 { return Err(anyhow!("string_join expects 2 arguments (list, delimiter)")); }
                let del = Executor::value_to_string(&args[1]);
                return match &args[0] {
                    Value::List(items) => {
                        let parts: Vec<String> = items.iter().map(|v| Executor::value_to_string(v)).collect();
                        Ok(Value::String(parts.join(&del)))
                    }
                    _ => Err(anyhow!("string_join: first argument must be a list")),
                };
            }

            // ── string_to_chars ──────────────────────────────────────────────
            "string_to_chars" => {
                if args.len() != 1 { return Err(anyhow!("string_to_chars expects 1 argument")); }
                let s = Executor::value_to_string(&args[0]);
                let chars: Vec<Value> = s.chars().map(|c: char| Value::String(c.to_string())).collect();
                return Ok(Value::List(chars));
            }

            // ── chars_to_string ──────────────────────────────────────────────
            "chars_to_string" => {
                if args.len() != 1 { return Err(anyhow!("chars_to_string expects 1 argument")); }
                return match &args[0] {
                    Value::List(items) => {
                        let s: String = items.iter().map(|v| Executor::value_to_string(v)).collect();
                        Ok(Value::String(s))
                    }
                    _ => Err(anyhow!("chars_to_string: expected a list")),
                };
            }

            // ================================================================
            // LIST BUILTINS
            // ================================================================

            // ── length (alias for len) ───────────────────────────────────────
            "length" => {
                if args.len() != 1 { return Err(anyhow!("length() takes exactly one argument")); }
                return match &args[0] {
                    Value::List(l)   => Ok(Value::Number(l.len() as f64)),
                    Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                    Value::Tensor(t) => Ok(Value::Number(t.numel() as f64)),
                    _ => Err(anyhow!("length() expects list, string, or tensor")),
                };
            }

            // ── list_first ───────────────────────────────────────────────────
            "list_first" => {
                if args.len() != 1 { return Err(anyhow!("list_first expects 1 argument")); }
                return match &args[0] {
                    Value::List(l) => Ok(l.first().cloned().unwrap_or(Value::None)),
                    _ => Err(anyhow!("list_first: expected a list")),
                };
            }

            // ── list_last ────────────────────────────────────────────────────
            "list_last" => {
                if args.len() != 1 { return Err(anyhow!("list_last expects 1 argument")); }
                return match &args[0] {
                    Value::List(l) => Ok(l.last().cloned().unwrap_or(Value::None)),
                    _ => Err(anyhow!("list_last: expected a list")),
                };
            }

            // ── list_take ────────────────────────────────────────────────────
            "list_take" => {
                if args.len() != 2 { return Err(anyhow!("list_take expects 2 arguments (list, count)")); }
                let n = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("list_take: count must be a number")) };
                return match &args[0] {
                    Value::List(l) => Ok(Value::List(l.iter().take(n).cloned().collect())),
                    _ => Err(anyhow!("list_take: expected a list")),
                };
            }

            // ── list_drop ────────────────────────────────────────────────────
            "list_drop" => {
                if args.len() != 2 { return Err(anyhow!("list_drop expects 2 arguments (list, count)")); }
                let n = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("list_drop: count must be a number")) };
                return match &args[0] {
                    Value::List(l) => Ok(Value::List(l.iter().skip(n).cloned().collect())),
                    _ => Err(anyhow!("list_drop: expected a list")),
                };
            }

            // ── list_slice ───────────────────────────────────────────────────
            "list_slice" => {
                if args.len() != 3 { return Err(anyhow!("list_slice expects 3 arguments (list, start, end)")); }
                let start = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("list_slice: start must be a number")) };
                let end   = match &args[2] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("list_slice: end must be a number")) };
                return match &args[0] {
                    Value::List(l) => {
                        let clamped_end = end.min(l.len());
                        let clamped_start = start.min(clamped_end);
                        Ok(Value::List(l[clamped_start..clamped_end].to_vec()))
                    }
                    _ => Err(anyhow!("list_slice: expected a list")),
                };
            }

            // ── list_reverse ─────────────────────────────────────────────────
            "list_reverse" => {
                if args.len() != 1 { return Err(anyhow!("list_reverse expects 1 argument")); }
                return match &args[0] {
                    Value::List(l) => {
                        let mut rev = l.clone();
                        rev.reverse();
                        Ok(Value::List(rev))
                    }
                    _ => Err(anyhow!("list_reverse: expected a list")),
                };
            }

            // ── list_concat ──────────────────────────────────────────────────
            "list_concat" => {
                if args.len() != 2 { return Err(anyhow!("list_concat expects 2 arguments")); }
                return match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let mut out = a.clone();
                        out.extend(b.iter().cloned());
                        Ok(Value::List(out))
                    }
                    _ => Err(anyhow!("list_concat: both arguments must be lists")),
                };
            }

            // ── list_flatten ─────────────────────────────────────────────────
            "list_flatten" => {
                if args.len() != 1 { return Err(anyhow!("list_flatten expects 1 argument")); }
                return match &args[0] {
                    Value::List(outer) => {
                        let mut flat: Vec<Value> = Vec::new();
                        for item in outer {
                            match item {
                                Value::List(inner) => flat.extend(inner.iter().cloned()),
                                other => flat.push(other.clone()),
                            }
                        }
                        Ok(Value::List(flat))
                    }
                    _ => Err(anyhow!("list_flatten: expected a list")),
                };
            }

            // ── list_sum ─────────────────────────────────────────────────────
            "list_sum" => {
                if args.len() != 1 { return Err(anyhow!("list_sum expects 1 argument")); }
                return match &args[0] {
                    Value::List(l) => {
                        let mut total = 0.0f64;
                        for v in l {
                            match &v {
                                Value::Number(n) => total += n,
                                Value::Bool(b) => if *b { total += 1.0; },
                                _ => (),
                            }
                        }
                        Ok(Value::Number(total))
                    }
                    _ => Err(anyhow!("list_sum: expected a list")),
                };
            }

            // ── list_average ─────────────────────────────────────────────────
            "list_average" => {
                if args.len() != 1 { return Err(anyhow!("list_average expects 1 argument")); }
                return match &args[0] {
                    Value::List(l) => {
                        if l.is_empty() { return Ok(Value::Number(0.0)); }
                        let mut total = 0.0f64;
                        for v in l {
                            match v {
                                Value::Number(n) => total += n.clone(),
                                _ => return Err(anyhow!("list_average: all elements must be numbers")),
                            }
                        }
                        Ok(Value::Number(total / l.len() as f64))
                    }
                    _ => Err(anyhow!("list_average: expected a list")),
                };
            }

            "not_null" => {
                if args.len() != 1 { return Err(anyhow!("not_null expects 1 argument")); }
                Ok(Value::Bool(!matches!(&args[0], Value::None)))
            }

            "identity" => {
                if args.len() != 1 { return Err(anyhow!("identity expects 1 argument")); }
                Ok(args[0].clone())
            }

            "bool" => {
                if args.len() != 1 { return Err(anyhow!("bool() takes exactly one argument")); }
                Ok(Value::Bool(self.value_is_truthy(&args[0])))
            }

            // ================================================================
            // VALIDATION BUILTINS
            // ================================================================

            "validate_not_empty" => {
                if args.len() != 2 { return Err(anyhow!("validate_not_empty expects 2 arguments (value, name)")); }
                let is_empty = match &args[0] {
                    Value::List(l)   => l.is_empty(),
                    Value::String(s) => s.is_empty(),
                    Value::None      => true,
                    _ => false,
                };
                Ok(Value::Bool(!is_empty))
            }

            "validate_is_number" => {
                if args.len() != 2 { return Err(anyhow!("validate_is_number expects 2 arguments (value, name)")); }
                Ok(Value::Bool(matches!(&args[0], Value::Number(_))))
            }

            "validate_is_string" => {
                if args.len() != 2 { return Err(anyhow!("validate_is_string expects 2 arguments (value, name)")); }
                Ok(Value::Bool(matches!(&args[0], Value::String(_))))
            }

            "validate_range" => {
                if args.len() != 4 { return Err(anyhow!("validate_range expects 4 arguments (value, min, max, name)")); }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Number(v), Value::Number(lo), Value::Number(hi)) => Ok(Value::Bool(*v >= *lo && *v <= *hi)),
                    _ => Err(anyhow!("validate_range: first 3 arguments must be numbers")),
                }
            }

            "validate_length" => {
                if args.len() != 4 { return Err(anyhow!("validate_length expects 4 arguments (value, min_len, max_len, name)")); }
                let actual = match &args[0] {
                    Value::List(l)   => l.len() as f64,
                    Value::String(s) => s.chars().count() as f64,
                    _ => return Err(anyhow!("validate_length: first argument must be list or string")),
                };
                match (&args[1], &args[2]) {
                    (Value::Number(lo), Value::Number(hi)) => Ok(Value::Bool(actual >= *lo && actual <= *hi)),
                    _ => Err(anyhow!("validate_length: min_len and max_len must be numbers")),
                }
            }

            // ================================================================
            // COLLECTION BUILTINS
            // ================================================================

            "collection_empty" => Ok(Value::List(vec![])),

            "collection_single" => {
                if args.len() != 1 { return Err(anyhow!("collection_single expects 1 argument")); }
                Ok(Value::List(vec![args[0].clone()]))
            }

            "collection_pair" => {
                if args.len() != 2 { return Err(anyhow!("collection_pair expects 2 arguments")); }
                Ok(Value::List(vec![args[0].clone(), args[1].clone()]))
            }

            "collection_triple" => {
                if args.len() != 3 { return Err(anyhow!("collection_triple expects 3 arguments")); }
                Ok(Value::List(vec![args[0].clone(), args[1].clone(), args[2].clone()]))
            }

            "collection_fill" => {
                if args.len() != 2 { return Err(anyhow!("collection_fill expects 2 arguments (value, size)")); }
                let n = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("collection_fill: size must be a number")) };
                    // No change needed, *n as usize is correct for Value::Number(n)
                Ok(Value::List(std::iter::repeat(args[0].clone()).take(n).collect()))
            }

            "collection_merge" => {
                if args.len() != 2 { return Err(anyhow!("collection_merge expects 2 arguments")); }
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let mut out = a.clone();
                        out.extend(b.iter().cloned());
                        Ok(Value::List(out))
                    }
                    _ => Err(anyhow!("collection_merge: both arguments must be lists")),
                }
            }

            "collection_merge_unique" => {
                if args.len() != 2 { return Err(anyhow!("collection_merge_unique expects 2 arguments")); }
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let mut out = a.clone();
                        for v in b { if !out.contains(v) { out.push(v.clone()); } }
                        Ok(Value::List(out))
                    }
                    _ => Err(anyhow!("collection_merge_unique: both arguments must be lists")),
                }
            }

            "collection_zip" => {
                if args.len() != 2 { return Err(anyhow!("collection_zip expects 2 arguments")); }
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let pairs: Vec<Value> = a.iter().zip(b.iter())
                            .map(|(x, y)| Value::List(vec![x.clone(), y.clone()]))
                            .collect();
                        Ok(Value::List(pairs))
                    }
                    _ => Err(anyhow!("collection_zip: both arguments must be lists")),
                }
            }

            // ================================================================
            // DIR / FILE STDLIB BUILTINS
            // ================================================================

            "dir_get_current" => Ok(Value::String(self.shell.pwd())),

            "dir_exists" => {
                if args.len() != 1 { return Err(anyhow!("dir_exists expects 1 argument")); }
                match &args[0] { Value::String(p) => Ok(Value::Bool(self.shell.is_dir(p))), _ => Err(anyhow!("dir_exists: path must be string")) }
            }

            "dir_create" => {
                if args.len() < 1 { return Err(anyhow!("dir_create expects 1 argument")); }
                let p = match &args[0] { Value::String(s) => s.clone(), _ => return Err(anyhow!("dir_create: path must be string")) };
                let parents = if args.len() > 1 { matches!(&args[1], Value::Bool(true)) } else { false };
                self.shell.mkdir(&p, parents).map(Value::String)
            }

            "dir_list" => {
                if args.len() != 1 { return Err(anyhow!("dir_list expects 1 argument")); }
                match &args[0] { Value::String(p) => self.shell.ls(Some(p)).map(Value::List), _ => Err(anyhow!("dir_list: path must be string")) }
            }

            "dir_delete" => {
                if args.len() != 1 { return Err(anyhow!("dir_delete expects 1 argument")); }
                match &args[0] { Value::String(p) => self.shell.rmdir(p).map(Value::String), _ => Err(anyhow!("dir_delete: path must be string")) }
            }

            "file_exists" => {
                if args.len() != 1 { return Err(anyhow!("file_exists expects 1 argument")); }
                match &args[0] { Value::String(p) => Ok(Value::Bool(self.shell.exists(p))), _ => Err(anyhow!("file_exists: path must be string")) }
            }

            "file_write" => {
                if args.len() != 2 { return Err(anyhow!("file_write expects 2 arguments (path, data)")); }
                let path = match &args[0] { Value::String(s) => s.clone(), _ => return Err(anyhow!("file_write: path must be string")) };
                let data = match &args[1] {
                    Value::String(s) => s.as_bytes().to_vec(),
                    Value::List(items) => {
                        items.iter().map(|v| match v { Value::Number(n) => Ok((n.round() as u32).min(255) as u8), _ => Err(anyhow!("file_write: list must contain numbers")) }).collect::<Result<Vec<_>>>()?
                    }
                    _ => return Err(anyhow!("file_write: data must be string or list")),
                };
                std::fs::write(&path, &data).map_err(|e| anyhow!("file_write: {}", e))?;
                Ok(Value::String(format!("Wrote {} bytes to {}", data.len(), path)))
            }

            "file_size" => {
                if args.len() != 1 { return Err(anyhow!("file_size expects 1 argument")); }
                match &args[0] { Value::String(p) => self.shell.file_size(p).map(|s| Value::Number(s as f64)), _ => Err(anyhow!("file_size: path must be string")) }
            }

            "file_delete" => {
                if args.len() != 1 { return Err(anyhow!("file_delete expects 1 argument")); }
                match &args[0] { Value::String(p) => self.shell.rm(p).map(Value::String), _ => Err(anyhow!("file_delete: path must be string")) }
            }

            "file_read" => {
                if args.len() != 1 { return Err(anyhow!("file_read expects 1 argument")); }
                match &args[0] {
                    Value::String(path) => {
                        match std::fs::read(path) {
                            Ok(bytes) => Ok(Value::List(bytes.iter().map(|&b| Value::Number(b as f64)).collect())),
                            Err(e) => Err(anyhow!("file_read: {}", e)),
                        }
                    }
                    _ => Err(anyhow!("file_read: path must be string")),
                }
            }

            // ================================================================
            // FORMATTING BUILTINS
            // ================================================================

            "format_number" => {
                if args.len() != 2 { return Err(anyhow!("format_number expects 2 arguments (number, decimals)")); }
                match (&args[0], &args[1]) {
                    (Value::Number(n), Value::Number(d)) => Ok(Value::String(format!("{:.prec$}", n, prec = *d as usize))),
                    _ => Err(anyhow!("format_number: expected (number, number)")),
                }
            }

            "format_currency" => {
                if args.len() != 2 { return Err(anyhow!("format_currency expects 2 arguments (amount, symbol)")); }
                let sym = Executor::value_to_string(&args[1]);
                match &args[0] {
                    Value::Number(n) => Ok(Value::String(format!("{}{:.2}", sym, n))),
                    _ => Err(anyhow!("format_currency: amount must be a number")),
                }
            }

            "format_percentage" => {
                if args.len() != 1 { return Err(anyhow!("format_percentage expects 1 argument")); }
                match &args[0] { Value::Number(n) => Ok(Value::String(format!("{:.1}%", n))), _ => Err(anyhow!("format_percentage: expected number")) }
            }

            "format_bytes" => {
                if args.len() != 1 { return Err(anyhow!("format_bytes expects 1 argument")); }
                match &args[0] {
                    Value::Number(n) => {
                        let b = n.round() as u64;
                        let s = if b < 1024 { format!("{} B", b) }
                                else if b < 1024 * 1024 { format!("{:.1} KB", b as f64 / 1024.0) }
                                else if b < 1024 * 1024 * 1024 { format!("{:.1} MB", b as f64 / (1024.0 * 1024.0)) }
                                else { format!("{:.1} GB", b as f64 / (1024.0 * 1024.0 * 1024.0)) };
                        Ok(Value::String(s))
                    }
                    _ => Err(anyhow!("format_bytes: expected number")),
                }
            }

            // ================================================================
            // TENSOR STDLIB ALIASES
            // ================================================================

            "tensor_create_zeros" => {
                if args.len() != 2 { return Err(anyhow!("tensor_create_zeros expects 2 arguments (rows, cols)")); }
                match (&args[0], &args[1]) {
                    (Value::Number(r), Value::Number(c)) => {
                        let (rows, cols) = (*r as usize, *c as usize);
                        Ok(Value::Tensor(RuntimeTensor::new(vec![rows, cols], "float32".to_string(), vec![0.0; rows * cols])))
                    }
                    _ => Err(anyhow!("tensor_create_zeros: expected two numbers")),
                }
            }

            "tensor_create_ones" => {
                if args.len() != 2 { return Err(anyhow!("tensor_create_ones expects 2 argumpents (rows, cols)")); }
                match (&args[0], &args[1]) {
                    (Value::Number(r), Value::Number(c)) => {
                        let (rows, cols) = (*r as usize, *c as usize);
                        Ok(Value::Tensor(RuntimeTensor::new(vec![rows, cols], "float32".to_string(), vec![1.0; rows * cols])))
                    }
                    _ => Err(anyhow!("tensor_create_ones: expected two numbers")),
                }
            }

            "tensor_create_identity" => {
                if args.len() != 1 { return Err(anyhow!("tensor_create_identity expects 1 argument (n)")); }
                match &args[0] {
                    Value::Number(n) => {
                        let n = n.round() as usize;
                            // No change needed, *n as usize is correct for Value::Number(n)
                        let mut data = vec![0.0f64; n * n];
                        for i in 0..n { data[i * n + i] = 1.0; }
                        Ok(Value::Tensor(RuntimeTensor::new(vec![n, n], "float32".to_string(), data)))
                    }
                    _ => Err(anyhow!("tensor_create_identity: expected number")),
                }
            }

            "tensor_from_list" => {
                if args.len() != 1 { return Err(anyhow!("tensor_from_list expects 1 argument")); }
                match &args[0] {
                    Value::List(items) => {
                        let data: Result<Vec<f64>> = items.iter().map(|v| match v { Value::Number(n) => Ok(n.clone()), _ => Err(anyhow!("tensor_from_list: list must contain numbers")) }).collect();
                        let data = data?;
                        let len = data.len();
                        Ok(Value::Tensor(RuntimeTensor::new(vec![len], "float32".to_string(), data)))
                    }
                    _ => Err(anyhow!("tensor_from_list: expected a list")),
                }
            }

            "tensor_get_shape" => {
                if args.len() != 1 { return Err(anyhow!("tensor_get_shape expects 1 argument")); }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::List(t.shape.iter().map(|&s| Value::Number(s as f64)).collect())),
                    _ => Err(anyhow!("tensor_get_shape: expected a tensor")),
                }
            }

            "tensor_get_dtype" => {
                if args.len() != 1 { return Err(anyhow!("tensor_get_dtype expects 1 argument")); }
                match &args[0] { Value::Tensor(t) => Ok(Value::String(t.dtype.clone())), _ => Err(anyhow!("tensor_get_dtype: expected a tensor")) }
            }

            "tensor_sum_all" => {
                if args.len() != 1 { return Err(anyhow!("tensor_sum_all expects 1 argument")); }
                match &args[0] { Value::Tensor(t) => Ok(Value::Number(t.data.iter().copied().sum())), _ => Err(anyhow!("tensor_sum_all: expected a tensor")) }
            }

            "tensor_mean_all" => {
                if args.len() != 1 { return Err(anyhow!("tensor_mean_all expects 1 argument")); }
                match &args[0] {
                    Value::Tensor(t) => {
                        let s: f64 = t.data.iter().copied().sum();
                        let n = t.data.len() as f64;
                        Ok(Value::Number(if n == 0.0 { 0.0 } else { s / n }))
                    }
                    _ => Err(anyhow!("tensor_mean_all: expected a tensor")),
                }
            }

            // ================================================================
            // LOGGING & DEBUG BUILTINS
            // ================================================================

            "log_info" => {
                if args.len() != 1 { return Err(anyhow!("log_info expects 1 argument")); }
                println!("[INFO] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }

            "log_warning" | "log_warn" => {
                if args.len() != 1 { return Err(anyhow!("log_warning expects 1 argument")); }
                println!("[WARN] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }

            "log_error" => {
                if args.len() != 1 { return Err(anyhow!("log_error expects 1 argument")); }
                println!("[ERROR] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }

            "log_debug" => {
                if args.len() != 1 { return Err(anyhow!("log_debug expects 1 argument")); }
                println!("[DEBUG] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }

            "debug_print_type" => {
                if args.len() != 1 { return Err(anyhow!("debug_print_type expects 1 argument")); }
                let t = match &args[0] {
                    Value::Number(_) => "number", Value::String(_) => "string", Value::Bool(_) => "bool",
                    Value::List(_) => "list", Value::Tensor(_) => "tensor", Value::Lambda(_) => "lambda",
                    Value::None => "none", Value::Heap(_) => "heap",
                };
                println!("[DEBUG] type({}) = {}", Executor::value_to_string(&args[0]), t);
                Ok(Value::None)
            }

            "debug_print_length" => {
                if args.len() != 1 { return Err(anyhow!("debug_print_length expects 1 argument")); }
                let len = match &args[0] {
                    Value::List(l)   => l.len(),
                    Value::String(s) => s.chars().count(),
                    _ => return Err(anyhow!("debug_print_length: expected list or string")),
                };
                println!("[DEBUG] length = {}", len);
                Ok(Value::None)
            }

            "debug_print_value" => {
                if args.len() != 2 { return Err(anyhow!("debug_print_value expects 2 arguments (name, value)")); }
                println!("[DEBUG] {} = {}", Executor::value_to_string(&args[0]), Executor::value_to_string(&args[1]));
                Ok(Value::None)
            }

            // ================================================================
            // FUNCTIONAL BUILTINS
            // ================================================================

            "pipe" => {
                if args.len() != 2 { return Err(anyhow!("pipe expects 2 arguments (value, [fns])")); }
                let fns = match &args[1] {
                    Value::List(l) => l.clone(),
                    _ => return Err(anyhow!("pipe: second argument must be a list of functions")),
                };
                let mut val = args[0].clone();
                for _f in fns {
                    // val = self.call_value_with_one_arg(&f, val)?;
                    // FIXME: call_value_with_one_arg is not implemented; placeholder for future implementation
                    return Err(anyhow!("call_value_with_one_arg is not implemented"));
                }
                Ok(val)
            }

            "partial" => {
                // partial(fn, arg) -> lambda that calls fn(arg, x)
                if args.len() != 2 { return Err(anyhow!("partial expects 2 arguments (fn, arg)")); }
                // We encode as a list [fn, fixed_arg] and handle it in call_value_with_one_arg
                Ok(Value::List(vec![
                    Value::String("__partial__".to_string()),
                    args[0].clone(),
                    args[1].clone(),
                ]))
            }

            // ================================================================
            // DATA PROCESSING BUILTINS
            // ================================================================

            "batch_split" => {
                if args.len() != 2 { return Err(anyhow!("batch_split expects 2 arguments (list, batch_size)")); }
                let batch_size = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("batch_split: batch_size must be a number")) };
                    // No change needed, *n as usize is correct for Value::Number(n)
                if batch_size == 0 { return Err(anyhow!("batch_split: batch_size must be > 0")); }
                match &args[0] {
                    Value::List(l) => {
                        let batches: Vec<Value> = l.chunks(batch_size)
                            .map(|chunk| Value::List(chunk.to_vec()))
                            .collect();
                        Ok(Value::List(batches))
                    }
                    _ => Err(anyhow!("batch_split: first argument must be a list")),
                }
            }

            "distinct_values" => {
                if args.len() != 1 { return Err(anyhow!("distinct_values expects 1 argument")); }
                match &args[0] {
                    Value::List(l) => {
                        let mut seen: Vec<Value> = Vec::new();
                        for v in l { if !seen.contains(v) { seen.push(v.clone()); } }
                        Ok(Value::List(seen))
                    }
                    _ => Err(anyhow!("distinct_values: expected a list")),
                }
            }

            // ================================================================
            // CONCURRENCY / PRIORITY DEMO BUILTINS
            // ================================================================

            "priority_critical_chain" => {
                self.priorities.add_edge("critical", "important");
                self.priorities.add_edge("important", "normal");
                Ok(Value::None)
            }

            "concurrent_task" => {
                if args.len() != 2 { return Err(anyhow!("concurrent_task expects 2 arguments (name, count)")); }
                let name  = Executor::value_to_string(&args[0]);
                let count = match &args[1] { Value::Number(n) => n.round() as usize, _ => return Err(anyhow!("concurrent_task: count must be a number")) };
                    // No change needed, *n as usize is correct for Value::Number(n)
                for i in 0..count {
                    println!("[Task {}] iteration {}", name, i);
                }
                Ok(Value::None)
            }

            other => Err(anyhow!("Unknown function '{}'", other)),
        } // end match name
    } // end fn call_builtin

    pub fn value_to_string(v: &Value) -> String {
        match v {
            Value::Number(n) => {
                if n.fract().abs() < f64::EPSILON { format!("{}", n.round() as i64) } else { format!("{}", n) }
            }
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(|i| Executor::value_to_string(i)).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Tensor(_t) => "<tensor>".to_string(),
            Value::Lambda(_) => "<lambda>".to_string(),
            Value::None => "None".to_string(),
            Value::Heap(_) => "<heap>".to_string(),
        }
    }

    /// Format a tensor for display.
    /// - 1D: tensor<dtype>[n][1, 2, 3]
    /// - 2D+: multi-line grid format:
    ///   tensor<float32>[2,3]
    ///   [[1, 2, 3],
    ///    [4, 5, 6]]
    fn tensor_to_string(&self, t: &RuntimeTensor) -> String {
        let shape_str = t.shape.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(",");
        let header = format!("tensor<{}>[{}]", t.dtype, shape_str);

        let fmt_val = |v: f64| -> String {
            if v.fract().abs() < f64::EPSILON { format!("{}", v as i64) } else { format!("{}", v) }
        };

        // Truncation threshold
        const MAX_SHOW: usize = 64;

        match t.rank() {
            0 => format!("{} scalar({})", header, if t.data.is_empty() { "?".to_string() } else { fmt_val(t.data[0]) }),
            1 => {
                let truncated = t.data.len() > MAX_SHOW;
                let items: Vec<String> = t.data.iter().take(MAX_SHOW).map(|&v| fmt_val(v)).collect();
                let body = if truncated {
                    format!("[{}, ...]", items.join(", "))
                } else {
                    format!("[{}]", items.join(", "))
                };
                format!("{}{}", header, body)
            }
            2 => {
                let rows = t.shape[0];
                let cols = t.shape[1];
                let max_rows = if rows > 8 { 4 } else { rows };
                let mut row_strs: Vec<String> = Vec::new();
                for r in 0..max_rows {
                    let start = r * cols;
                    let end = (start + cols).min(t.data.len());
                    let row_data = &t.data[start..end];
                    let max_cols = if cols > MAX_SHOW { MAX_SHOW } else { cols };
                    let items: Vec<String> = row_data.iter().take(max_cols).map(|&v| fmt_val(v)).collect();
                    let row_s = if cols > MAX_SHOW {
                        format!("[{}, ...]", items.join(", "))
                    } else {
                        format!("[{}]", items.join(", "))
                    };
                    row_strs.push(row_s);
                }
                let pad = " ".repeat(1);
                let joined = if rows > 8 {
                    let first_part = row_strs.join(",\n ");
                    format!("{},\n {} ...\n {}(showing {}/{} rows)",
                        first_part, pad, pad, max_rows, rows)
                } else {
                    row_strs.join(",\n ")
                };
                format!("{}\n[{}]", header, joined)
            }
            _ => {
                // Higher-rank: flat display with truncation
                let truncated = t.data.len() > MAX_SHOW;
                let items: Vec<String> = t.data.iter().take(MAX_SHOW).map(|&v| fmt_val(v)).collect();
                let body = if truncated {
                    format!("[{}, ...]", items.join(", "))
                } else {
                    format!("[{}]", items.join(", "))
                };
                format!("{}{}", header, body)
            }
        }
    }

    fn do_print(&self, v: &Value) {
        match v {
            Value::None => {}, // Suppress printing None in all cases
            Value::List(items) => {
                for item in items {
                    if !matches!(item, Value::None) {
                        println!("{}", Executor::value_to_string(item));
                    }
                }
            }
            other => println!("{}", Executor::value_to_string(other)),
        }
    }

    fn build_tensor(&mut self, expr: &Expr) -> Result<Value> {
        // evaluate the expression (mutable borrow) and immediately collapse
        // any heap references so we always start with a concrete value.
        let tmp = self.eval_expr(expr)?;
        let evaluated = self.deref(tmp);

        // recursive helper returns (shape_vector, flattened_data).  It
        // takes an explicit reference to the executor so that we can
        // dereference any heap handles encountered deeper in the
        // structure.
        fn collect(exe: &Executor, v: &Value) -> Result<(Vec<usize>, Vec<f64>)> {
            let v = exe.deref(v.clone());
            match &v {
                Value::Number(n) => Ok((Vec::new(), vec![n.clone()])),
                Value::List(items) => {
                    if items.is_empty() {
                        return Err(anyhow!("Tensor rows cannot be empty"));
                    }
                    // collect first element to determine subshape
                    let (first_shape, mut first_data) = collect(exe, &items[0])?;
                    let mut flat = Vec::new();
                    flat.append(&mut first_data);

                    for item in &items[1..] {
                        let (shape, mut data) = collect(exe, item)?;
                        if shape != first_shape {
                            return Err(anyhow!("Ragged tensor: inconsistent dimensions"));
                        }
                        flat.append(&mut data);
                    }

                    let mut shape = Vec::new();
                    shape.push(items.len());
                    shape.extend(first_shape);
                    Ok((shape, flat))
                }
                other => Err(anyhow!("Tensor element must be a number, got: {:?}", other)),
            }
        }

        let (shape, data) = collect(self, &evaluated)?;
        if shape.is_empty() {
            return Err(anyhow!("Cannot build tensor from scalar"));
        }
        let dtype = "float32".to_string();
        Ok(Value::Tensor(RuntimeTensor::new(shape, dtype, data)))
    }

    fn expr_to_simple(&self, e: &Expr) -> ExprSimple {
        match e {
            Expr::Identifier(id) => ExprSimple::Identifier(id.name.clone()),
            Expr::Number(n, _) => ExprSimple::Number(*n),
            Expr::String(s, _) => ExprSimple::Raw(s.clone()),
            Expr::Bool(b, _) => ExprSimple::Raw(b.to_string()),
            Expr::Binary { left, right, .. } => {
                let l = match &**left {
                    Expr::Identifier(id) => id.name.clone(),
                    Expr::Number(n, _) => n.to_string(),
                    o => format!("{:?}", o),
                };
                let r = match &**right {
                    Expr::Identifier(id) => id.name.clone(),
                    Expr::Number(n, _) => n.to_string(),
                    o => format!("{:?}", o),
                };
                ExprSimple::Raw(format!("{} ? {}", l, r))
            }
            other => ExprSimple::Raw(format!("{:?}", other)),
        }
    }

    fn eval_binary(&self, op: &BinaryOp, left: Value, right: Value) -> Result<Value> {
        use BinaryOp::*;
        // Before doing any pattern matching we want to collapse heap
        // references to their actual contents.  This keeps the match arms
        // simple (e.g. `(Value::String(a), Value::String(b))`) and avoids
        // needing to handle `Value::Heap` everywhere.
        let left = self.deref(left);
        let right = self.deref(right);
        match op {
            Add => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a.clone() + &b)),
                (Value::String(a), Value::Number(b)) => {
                    let bs = if b.fract().abs() < f64::EPSILON { format!("{}", b.round() as i64) } else { format!("{}", b) };
                    Ok(Value::String(a.clone() + &bs))
                }
                (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_elementwise(&a, &b, |x, y| x + y),
                (Value::Tensor(a), Value::Number(s)) => Executor::tensor_scalar(&a, s.clone(), |x, y| x + y),
                (Value::Number(s), Value::Tensor(b)) => Executor::tensor_scalar(&b, s.clone(), |x, y| y + x),
                _ => {
                    // fallback for List concat
                    match (left, right) {
                        (Value::List(mut a), Value::List(b)) => { a.extend(b); Ok(Value::List(a)) }
                        _ => Err(anyhow!("Unsupported operands for +")),
                    }
                }
            },
            Sub => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
                (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_elementwise(&a, &b, |x, y| x - y),
                (Value::Tensor(a), Value::Number(s)) => Executor::tensor_scalar(&a, s.clone(), |x, y| x - y),
                (Value::Number(s), Value::Tensor(b)) => Executor::tensor_scalar(&b, s.clone(), |x, y| y - x),
                _ => Err(anyhow!("Unsupported operands for -")),
            },
            Mul => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
                (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_elementwise(&a, &b, |x, y| x * y),
                (Value::Tensor(a), Value::Number(s)) => Executor::tensor_scalar(&a, s.clone(), |x, y| x * y),
                (Value::Number(s), Value::Tensor(b)) => Executor::tensor_scalar(&b, s.clone(), |x, y| y * x),
                _ => Err(anyhow!("Unsupported operands for *")),
            },
            Div => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b == 0.0 { Err(anyhow!("Division by zero")) } else { Ok(Value::Number(a / b)) }
                }
                (Value::Tensor(a), Value::Number(s)) => {
                    if *s == 0.0 { return Err(anyhow!("Tensor division by zero")); }
                    Executor::tensor_scalar(&a, *s, |x, y| x / y)
                }
                (Value::Number(s), Value::Tensor(b)) => {
                    if *s == 0.0 { return Err(anyhow!("Tensor division by zero")); }
                    Executor::tensor_scalar(&b, *s, |y, x| x / y)
                }
                (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_elementwise(&a, &b, |x, y| x / y),
                _ => Err(anyhow!("Unsupported operands for /")),
            },
            MatMul => match (&left, &right) {
                (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_matmul(&a, &b),
                _ => Err(anyhow!("@ (matmul) requires two tensors ")),
            },
            Eq  => Ok(Value::Bool(left == right)),
            Neq => Ok(Value::Bool(left != right)),
            Lt  => match (left, right) { (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a < b)),  _ => Err(anyhow!("< requires numbers ")) },
            Gt  => match (left, right) { (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a > b)),  _ => Err(anyhow!("> requires numbers ")) },
            Lte => match (left, right) { (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a <= b)), _ => Err(anyhow!("<= requires numbers ")) },
            Gte => match (left, right) { (Value::Number(a), Value::Number(b)) => Ok(Value::Bool(a >= b)), _ => Err(anyhow!(">= requires numbers ")) },
            Approx => {
                match (&left, &right) {
                    // Empty-string special case — must come BEFORE the general string arm
                    (Value::String(a), Value::String(b)) if a.is_empty() && b.is_empty() => {
                        Ok(Value::Bool(true))
                    }
                    (Value::Number(a), Value::Number(b)) => {
                        Ok(Value::Bool((a - b).abs() <= 0.0001))
                    }
                    (Value::String(a), Value::String(b)) => {
                        let match_ci = a.to_lowercase() == b.to_lowercase();
                        let match_lev = Executor::levenshtein(&a, &b) <= 1;
                        Ok(Value::Bool(match_ci || match_lev))
                    }
                    (Value::None, Value::None) => Ok(Value::Bool(true)),
                    _ => Ok(Value::Bool(false)),
                }
            }
            NotEq => {
                Ok(Value::Bool(left != right))
            }
            StrictEq => {
                let is_identical = match (&left, &right) {
                    (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
                    (Value::String(a), Value::String(b)) => a == b,
                    (Value::Bool(a), Value::Bool(b)) => a == b,
                    (Value::None, Value::None) => true,
                    _ => false,
                };
                Ok(Value::Bool(is_identical))
            }
            And => Ok(Value::Bool(self.value_is_truthy(&left) && self.value_is_truthy(&right))),
            Or  => Ok(Value::Bool(self.value_is_truthy(&left) || self.value_is_truthy(&right))),
            Not => {
                match right {
                    Value::Bool(b) => Ok(Value::Bool(!b)),
                    _ => Ok(Value::Bool(!self.value_is_truthy(&right))),
                }
            }
        }
    }

    // ── Tensor arithmetic helpers ─────────────────────────────────────────────

    pub fn tensor_elementwise(a: &RuntimeTensor, b: &RuntimeTensor, op: impl Fn(f64, f64) -> f64) -> Result<Value> {
        if a.shape != b.shape {
            return Err(anyhow!(
                "Shape mismatch for elementwise op: {:?} vs {:?}", a.shape, b.shape
            ));
        }
        let data: Vec<f64> = a.data.iter().zip(b.data.iter()).map(|(&x, &y)| op(x, y)).collect();
        let dtype = if a.dtype == "float32" || b.dtype == "float32" { "float32" } else { "int32" };
        Ok(Value::Tensor(RuntimeTensor::new(a.shape.clone(), dtype.to_string(), data)))
    }

    pub fn tensor_scalar(t: &RuntimeTensor, scalar: f64, op: impl Fn(f64, f64) -> f64) -> Result<Value> {
        let data: Vec<f64> = t.data.iter().map(|&x| op(x, scalar)).collect();
        let dtype = if t.dtype == "float32" || scalar.fract() != 0.0 { "float32" } else { "int32" };
        Ok(Value::Tensor(RuntimeTensor::new(t.shape.clone(), dtype.to_string(), data)))
    }

    pub fn tensor_matmul(a: &RuntimeTensor, b: &RuntimeTensor) -> Result<Value> {
        if a.rank() != 2 || b.rank() != 2 {
            return Err(anyhow!("@ (matmul) requires 2D tensors, got shapes {:?} and {:?}", a.shape, b.shape));
        }
        let (m, k1) = (a.shape[0], a.shape[1]);
        let (k2, n) = (b.shape[0], b.shape[1]);
        if k1 != k2 {
            return Err(anyhow!("matmul inner dimensions must match: {} vs {}", k1, k2));
        }
        let mut out = vec![0.0f64; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0;
                for k in 0..k1 {
                    sum += a.data[i * k1 + k] * b.data[k * n + j];
                }
                out[i * n + j] = sum;
            }
        }
        let dtype = if a.dtype == "float32" || b.dtype == "float32" { "float32" } else { "int32" };
        Ok(Value::Tensor(RuntimeTensor::new(vec![m, n], dtype.to_string(), out)))
    }

    // ── Levenshtein edit distance (for ≈ string comparison) ──────────────────

    pub fn levenshtein(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let (m, n) = (a.len(), b.len());
        // Early exit
        if m == 0 { return n; }
        if n == 0 { return m; }
        // Use two rolling rows to keep memory O(n)
        let mut prev: Vec<usize> = (0..=n).collect();
        let mut curr = vec![0usize; n + 1];
        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
                curr[j] = (prev[j] + 1)           // deletion
                    .min(curr[j - 1] + 1)          // insertion
                    .min(prev[j - 1] + cost);       // substitution
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }

    fn resolve_repeat_counts(&mut self, n_targets: usize, repeats_opt: Option<&Vec<Expr>>, span: &Span) -> Result<Vec<usize>> {
        if n_targets == 0 { return Ok(vec![]); }
        if repeats_opt.is_none() { return Ok(vec![1usize; n_targets]); }
        let repeats = repeats_opt.unwrap();

        if repeats.len() == 1 {
            let r_val = self.eval_expr(&repeats[0])?;
            let n = Executor::value_to_repeat_count(&r_val).map_err(|e| self.span_err(span, e))?;
            return Ok(vec![n; n_targets]);
        }

        if repeats.len() == n_targets {
            let mut counts = Vec::with_capacity(n_targets);
            for r_expr in repeats.iter() {
                let r_val = self.eval_expr(r_expr)?;
                let n = Executor::value_to_repeat_count(&r_val).map_err(|e| self.span_err(span, e))?;
                counts.push(n);
            }
            return Ok(counts);
        }

        Err(self.span_err(span, format!(
            "FOR repeat count list length {} does not match number of targets {}",
            repeats.len(), n_targets
        )))
    }

    fn value_to_repeat_count(v: &Value) -> Result<usize, String> {
        match v {
            Value::Number(n) => {
                if n.is_sign_negative() { Err("repeat count must be non-negative ".into()) }
                else { Ok(n.trunc() as usize) }
            }
            Value::String(s) => s.parse::<usize>().map_err(|_| "cannot parse repeat count from string ".into()),
            _ => Err("repeat count must be a number ".into()),
        }
    }

    // Removed duplicate parse_color_arg(...) definition
    pub fn parse_color_arg(arg: Option<&Value>) -> (u8, u8, u8, u8) {
        if let Some(Value::String(s)) = arg {
            let s = s.trim();
            if s.starts_with('#') && (s.len() == 7 || s.len() == 4) {
                if s.len() == 7 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&s[1..3], 16),
                        u8::from_str_radix(&s[3..5], 16),
                        u8::from_str_radix(&s[5..7], 16),
                    ) {
                        return (r, g, b, 255);
                    }
                } else {
                    let r = u8::from_str_radix(&s[1..2].repeat(2), 16).unwrap_or(0);
                    let g = u8::from_str_radix(&s[2..3].repeat(2), 16).unwrap_or(0);
                    let b = u8::from_str_radix(&s[3..4].repeat(2), 16).unwrap_or(0);
                    return (r, g, b, 255);
                }
            }

            if s.to_lowercase().starts_with("rgb(") && s.ends_with(')') {
                let inner = &s[4..s.len() - 1];
                let parts: Vec<&str> = inner.split(',').map(|p| p.trim()).collect();
                if parts.len() >= 3 {
                    let r = parts[0].parse::<u8>().unwrap_or(0);
                    let g = parts[1].parse::<u8>().unwrap_or(0);
                    let b = parts[2].parse::<u8>().unwrap_or(0);
                    return (r, g, b, 255);
                }
            }
        }
        (0, 0, 0, 255)
    }

    fn span_err<T: Into<String>>(&self, span: &Span, msg: T) -> anyhow::Error {
        // produce a RuntimeError and convert to anyhow for backwards
        // compatibility with code that returns `Result<_, anyhow::Error>`.
        // The message passed in is used as the human-readable description;
        // most callers simply forward the string they were already
        // constructing before the centralized error types existed.
        let mut err = RuntimeError::new(RuntimeErrorKind::SyntaxError, span.clone());
        err.message = msg.into();
        err = err.with_traceback(self.traceback.clone());
        anyhow!("{}", err)
    }

    /// Simple helper to lex & parse source code into a Program.
    pub fn parse(src: &str) -> crate::parser::ast::Program {
        let tokens = crate::lexer::lexer::Lexer::new(src).lex();
        crate::parser::parser::Parser::new(tokens).parse()
    }

    /// Convenience helper: execute a snippet and return the final environment.
    pub fn run(src: &str) -> Result<Environment> {
        let prog = Executor::parse(src);
        let mut ex = Executor::new();
        ex.execute_program(&prog)?;
        Ok(ex.env)
        }

    /// Enter the integrated shell. Blocks until the shell exits.
    /// Uses the executor itself (so the adapter can access `self.env`) to avoid
    /// double mutable borrows.
    pub fn enter_shell(&mut self) -> Result<(), String> {
        match crate::interpreter::shell_os::run_shell(self) {
            Ok(_) => Ok(()),
            Err(e) => Err(e),
         }
    }

    
} // end impl Executor


// All test modules moved to the end of the file
#[cfg(test)]
mod executor_error_tests {
    use super::*;
    #[test]
    fn parse_error_uses_central_messages() {
        // missing identifier after DEF should use the constant
        let tokens = Lexer::new("DEF    DO END ").lex();
        let mut parser = Parser::new(tokens);
        let (_prog, diags) = parser.parse_with_diagnostics();
        assert_eq!(diags.len(), 1);
        assert_eq!(diags[0].message, crate::interpreter::errors::messages::EXPECTED_IDENTIFIER_AFTER_DEF);
    }

    #[test]
    fn runtime_error_includes_traceback() {
        // use a DO-WHILE loop since standalone WHILE is not a statement in the
        // current grammar. The executor will treat this as a WhileBlock.
        let prog = parse("DO a WHILE 1:\n  x = 0\nEND ");
        let mut ex = Executor::new();
        ex.set_while_limit(1);
        let err = ex.execute_program(&prog).unwrap_err();
        let text = format!("{}", err);
        assert!(text.contains("exceeded iteration limit "), "got: {}", text);
        assert!(text.contains("Traceback:"));
    }
}

#[cfg(test)]
mod executor_tensor_tests {
    use super::*;
    #[test]
    fn exec_build_tensor_basic() {
        let env = run("x = BUILD TENSOR: [[1,2],[3,4]]\n ").unwrap();
        let v = env.get("x").expect("x should be defined ");
        match v {
            Value::Tensor(t) => {
                assert_eq!(t.shape, vec![2, 2]);
                assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0]);
                assert_eq!(t.dtype, "float32");
            }
            other => panic!("expected tensor, got {:?}", other),
        }
    }

    #[test]
    fn exec_build_tensor_float_dtype() {
        let env = run("x = BUILD TENSOR: [[1.0, 2],[3, 4]]\n ").unwrap();
        let v = env.get("x").unwrap();
        if let Value::Tensor(t) = v {
            assert_eq!(t.dtype, "float32");
        } else {
            panic!("not a tensor ");
        }
    }

    #[test]
    fn exec_build_tensor_ragged_error() {
        let mut ex = Executor::new();
        let prog = parse("x = BUILD TENSOR: [[1,2],[3]]\n ");
        let res = ex.execute_program(&prog);
        assert!(res.is_err(), "ragged tensor should error ");
    }

    #[test]
    fn exec_build_tensor_non_number_error() {
        let mut ex = Executor::new();
        let prog = parse("x = BUILD TENSOR: [[1,\"a\"],[3,4]]\n ");
        let res = ex.execute_program(&prog);
        assert!(res.is_err(), "non-number element should error ");
    }

    #[test]
    fn exec_build_tensor_multiline() {
        let src = "x = BUILD TENSOR:\n    [1, 2],\n    [3, 4]\n";
        let env = Executor::run(src).unwrap();
        let v = env.get("x").unwrap();
        if let Value::Tensor(t) = v {
            assert_eq!(t.shape, vec![2, 2]);
            assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0]);
        } else {
            panic!("not a tensor");
        }
    }
}

    // ...existing code...


// Move test functions outside impl block
#[cfg(test)]
mod executor_tests6 {
    use super::*;
    #[test]
    fn exec_build_tensor_3x2() {
        let src = "x = BUILD TENSOR: [[1,2],[3,4],[5,6]]\n";
        let env = Executor::run(src).unwrap();
        let v = env.get("x").unwrap();
        if let Value::Tensor(t) = v {
            assert_eq!(t.shape, vec![3, 2]);
            assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        } else {
            panic!("not a tensor");
        }
    }

    #[test]
    fn exec_tensor_via_expr() {
        let src = "PRINT BUILD TENSOR: [[1,2],[3,4]]\n";
        // Just ensure it doesn't error
        let prog = parse(src);
        let mut ex = Executor::new();
        let _ = ex.execute_program(&prog);
    }

    #[test]
    fn exec_tensor_stdlib_basic() {
        let env = run("x = tensor.zeros([2,3])\ny = tensor.ones(4)\nz = tensor.eye(3)\n").unwrap();
        if let Value::Tensor(t) = env.get("x").unwrap() {
            assert_eq!(t.shape, vec![2,3]);
            assert!(t.data.iter().all(|&v| v == 0.0));
        } else { panic!(); }
        if let Value::Tensor(t) = env.get("y").unwrap() {
            assert_eq!(t.shape, vec![4]);
            assert!(t.data.iter().all(|&v| v == 1.0));
        } else { panic!(); }
        if let Value::Tensor(t) = env.get("z").unwrap() {
            assert_eq!(t.shape, vec![3,3]);
            for i in 0..3 { for j in 0..3 { assert_eq!(t.data[i*3+j], if i==j {1.0} else {0.0}); }}
        } else { panic!(); }
    }

    #[test]
    fn exec_tensor_sum_mean() {
        let env = run("a = BUILD TENSOR: [1,2,3]\nb = tensor.sum(a)\nc = tensor.mean(a)\n").unwrap();
        assert_eq!(env.get("b"), Some(Value::Number(6.0)));
        assert_eq!(env.get("c"), Some(Value::Number(2.0)));
    }
}

// Move test functions outside impl block
#[cfg(test)]
mod executor_tests5 {
    use super::*;
    #[test]
    fn exec_tensor_reshape_transpose_flatten() {
        let src = "a = BUILD TENSOR: [[1,2,3],[4,5,6]]\n";
        let src = format!("{}b = tensor.reshape(a, [3,2])\n", src);
        let src = format!("{}c = tensor.transpose(a)\n", src);
        let src = format!("{}d = tensor.flatten(a)\n", src);
        let env = run(&src).unwrap();
        // reshape result should reorder data but same elements
        if let Value::Tensor(t) = env.get("b").unwrap() {
            assert_eq!(t.shape, vec![3,2]);
            assert_eq!(t.data, vec![1.0,2.0,3.0,4.0,5.0,6.0]);
        } else { panic!(); }
        if let Value::Tensor(t) = env.get("c").unwrap() {
            assert_eq!(t.shape, vec![3,2]);
            assert_eq!(t.data, vec![1.0,4.0,2.0,5.0,3.0,6.0]);
        } else { panic!(); }
        if let Value::Tensor(t) = env.get("d").unwrap() {
            assert_eq!(t.shape, vec![6]);
            assert_eq!(t.data, vec![1.0,2.0,3.0,4.0,5.0,6.0]);
        } else { panic!(); }
    }
}

// Move test functions outside impl block
#[cfg(test)]
mod executor_tests4 {
    use super::*;
    #[test]
    fn exec_tensor_elementwise_ops() {
        let env = run("a = BUILD TENSOR: [[1,2],[3,4]]\nb = BUILD TENSOR: [[5,6],[7,8]]\nc = a + b\nd = b - a\ne = a * b\n").unwrap();
        let c = env.get("c").unwrap();
        let d = env.get("d").unwrap();
        let e = env.get("e").unwrap();
        if let Value::Tensor(t) = c {
            assert_eq!(t.data, vec![6.0,8.0,10.0,12.0]);
        } else { panic!("c not tensor"); }
        if let Value::Tensor(t) = d {
            assert_eq!(t.data, vec![4.0,4.0,4.0,4.0]);
        } else { panic!("d not tensor"); }
        if let Value::Tensor(t) = e {
            assert_eq!(t.data, vec![5.0,12.0,21.0,32.0]);
        } else { panic!("e not tensor"); }
    }
}

// Move test functions outside impl block
#[cfg(test)]
mod executor_tests3 {
    use super::*;
    #[test]
    fn exec_tensor_scalar_ops() {
        let env = run("a = BUILD TENSOR: [[1,2],[3,4]]\nb = a + 1\nc = 2 + a\nd = a * 2\ne = 8 / a\n").unwrap();
        let b = env.get("b").unwrap();
        let c = env.get("c").unwrap();
        let d = env.get("d").unwrap();
        let e = env.get("e").unwrap();
        if let Value::Tensor(t) = b { assert_eq!(t.data, vec![2.0,3.0,4.0,5.0]); } else { panic!(); }
        if let Value::Tensor(t) = c { assert_eq!(t.data, vec![3.0,4.0,5.0,6.0]); } else { panic!(); }
        if let Value::Tensor(t) = d { assert_eq!(t.data, vec![2.0,4.0,6.0,8.0]); } else { panic!(); }
        if let Value::Tensor(t) = e { assert_eq!(t.data, vec![8.0,4.0,8.0/3.0,2.0]); } else { panic!(); }
    }
}


// Move test functions outside impl block
#[cfg(test)]
mod executor_tests2 {
    use super::*;
    #[test]
    fn exec_tensor_matmul_and_errors() {
        let env = Executor::run("a = BUILD TENSOR: [[1,2],[3,4]]\nb = BUILD TENSOR: [[5,6],[7,8]]\nc = a @ b\n").unwrap();
        if let Value::Tensor(t) = env.get("c").unwrap() {
            assert_eq!(t.shape, vec![2,2]);
            assert_eq!(t.data, vec![19.0,22.0,43.0,50.0]);
        } else { panic!(); }
        // mismatched shapes for elementwise
        let mut ex = Executor::new();
        let prog = Executor::parse("x = BUILD TENSOR: [[1,2]] + BUILD TENSOR: [[1,2],[3,4]]\n");
        assert!(ex.execute_program(&prog).is_err());
        // matmul dimension mismatch
        let mut ex2 = Executor::new();
        let prog2 = Executor::parse("x = BUILD TENSOR: [[1,2,3]] @ BUILD TENSOR: [[1,2],[3,4]]\n");
        assert!(ex2.execute_program(&prog2).is_err());
    }

    #[test]
    fn exec_build_tensor_high_dim() {
        // create a 3x2x2 tensor
        let src = "x = BUILD TENSOR: [[[1,2],[3,4]],[[5,6],[7,8]],[[9,10],[11,12]]]\n";
        let env = run(src).unwrap();
        if let Value::Tensor(t) = env.get("x").unwrap() {
            assert_eq!(t.shape, vec![3,2,2]);
            assert_eq!(t.data, vec![1.0,2.0,3.0,4.0,5.0,6.0,7.0,8.0,9.0,10.0,11.0,12.0]);
        } else { panic!("not a tensor"); }
    }
}

// Move test functions outside impl block
#[cfg(test)]
mod executor_tests {
    use super::*;
    #[test]
    fn exec_gc_basic_collection() {
        let mut ex = Executor::new();
        // allocate a list and take a snapshot of the heap size immediately
        ex.execute_program(&parse("a = [1]\n")).unwrap();
        let before = ex.gc.allocated_count();

        // overwrite the variable; the collector runs automatically and should
        // reclaim the list object, so the heap count must drop.
        ex.execute_program(&parse("a = 0\n")).unwrap();
        let after = ex.gc.allocated_count();
        assert!(after < before, "heap should shrink once 'a' is cleared");
    }

    #[test]
    fn exec_gc_retains_referenced() {
        let mut ex = Executor::new();
        // allocate a list and alias it
        ex.execute_program(&parse("a = [1]\nb = a\n")).unwrap();
        let before = ex.gc.allocated_count();

        // reassign `a` but `b` still refers to the list; after the implicit GC
        // the count should stay the same.
        ex.execute_program(&parse("a = 0\n")).unwrap();
        let after = ex.gc.allocated_count();

        assert_eq!(after, before, "object should remain reachable via b");
    }
}
