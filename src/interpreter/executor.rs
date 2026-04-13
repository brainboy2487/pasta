// ...existing code...
/// Access the VERBOSE_DEBUG flag from main
pub fn is_verbose_debug() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    extern "Rust" {
        static VERBOSE_DEBUG: AtomicBool;
    }
    unsafe { VERBOSE_DEBUG.load(Ordering::Relaxed) }
}

// src/interpreter/executor.rs
// Executor — startup/shutdown, high-level orchestration, wiring to subsystems.
// Statement and expression evaluation now live in ex_eval.rs.
// Frame/scope helpers now live in ex_frame.rs.

use anyhow::{anyhow, Result};
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::interpreter::environment::{Environment, RuntimeTensor, ScopeKind, Value};
use crate::interpreter::ex_eval;
use crate::interpreter::ex_frame;

//
// --- Object system helpers -------------------------------------------------
//

/// Metadata for a user-defined object family (class prototype).
#[derive(Debug, Clone)]
pub struct ObjectFamily {
    pub name: String,
    pub family_group: String,
    pub params: Vec<crate::parser::Identifier>,
    pub fields: Vec<crate::parser::FieldDecl>,
    pub mutation_table: Vec<crate::parser::MutEntry>,
    pub constructor: Option<crate::parser::Constructor>,
    pub methods: Vec<crate::parser::MethodDecl>,
}

/// A live object instance allocated by the executor.
#[derive(Debug, Clone)]
pub struct ObjectInstance {
    pub id: u64,
    pub family: String,
    pub fields: std::collections::HashMap<String, Value>,
    pub applied_mutations: std::collections::HashSet<String>,
}

impl ObjectInstance {
    pub fn new(id: u64, family: String, fields: std::collections::HashMap<String, Value>) -> Self {
        Self {
            id,
            family,
            fields,
            applied_mutations: std::collections::HashSet::new(),
        }
    }
}

use crate::interpreter::ai_network;
use crate::interpreter::errors::{RuntimeError, RuntimeErrorKind, Traceback};
use crate::interpreter::shell::Shell;
use crate::lexer::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::parser::*;
use crate::runtime::pointer::{PointerContext, SharedPointerRegistry};
use crate::runtime::rng::Rng;
use crate::runtime::strainer::Strainer;
use crate::runtime::ModuleRegistry;
use crate::semantics::{ConstraintEngine, ExprSimple, PriorityGraph};

const DEFAULT_WHILE_LIMIT: usize = 1_000_000;

// ── Control flow signal ───────────────────────────────────────────────────────

/// Control flow signals that unwind the statement execution loop.
#[derive(Debug, Clone)]
pub enum ControlFlowSignal {
    /// RET.NOW() fired — carry the return value up to the call site.
    Return(Value),
    /// BREAK — exit the innermost loop.
    Break,
    /// CONTINUE — skip to the next iteration of the innermost loop.
    Continue,
    /// Thread received a kill signal via :threads:kill — unwind immediately.
    Killed,
    /// GOTO <label> — restart the named LoopBlock from its top.
    GotoLabel(String),
}

// ── FPS system ────────────────────────────────────────────────────────────────

/// Per-frame timing state for the FPS module.  Activated by `FROM fps USE ...`.
pub struct FpsState {
    pub target_fps: f64,
    /// Fixed timestep returned by FPS_TICK / FPS_DELTA: always 1.0 / target_fps.
    pub fixed_delta: f64,
    pub frame_count: u64,
    pub paused: bool,
    pub init_time: std::time::Instant,
    /// Timestamp recorded at the start of each frame by FPS_BEGIN.
    pub frame_start: std::time::Instant,
    /// Real milliseconds the last completed frame took (FPS_END updates this).
    pub last_frame_time_ms: f64,
    /// Circular history of per-frame real durations in seconds (capped at 256).
    pub frame_times: std::collections::VecDeque<f64>,
    pub pause_start: Option<std::time::Instant>,
    /// Total wall time spent in a paused state (excluded from FPS_ELAPSED).
    pub paused_duration: std::time::Duration,
}

impl FpsState {
    pub fn new(target_fps: f64) -> Self {
        let now = std::time::Instant::now();
        let target_fps = if target_fps <= 0.0 { 60.0 } else { target_fps };
        Self {
            target_fps,
            fixed_delta: 1.0 / target_fps,
            frame_count: 0,
            paused: false,
            init_time: now,
            frame_start: now,
            last_frame_time_ms: 0.0,
            frame_times: std::collections::VecDeque::with_capacity(256),
            pause_start: None,
            paused_duration: std::time::Duration::ZERO,
        }
    }
}

// ── Executor struct ───────────────────────────────────────────────────────────

// ── Kill-channel TLS ─────────────────────────────────────────────────────────

use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};

/// Monotonically increasing counter used to assign unique IDs to script pipelines.
static PIPELINE_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Allocate the next unique pipeline ID.
fn next_pipeline_id() -> u64 {
    PIPELINE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

thread_local! {
    /// Receiver half of the kill channel for this PASTA script thread.
    /// Set by `set_kill_rx` before `Executor::run` is called.
    /// `execute_statement` polls this on every statement boundary.
    static KILL_RX: RefCell<Option<std::sync::mpsc::Receiver<()>>> =
        RefCell::new(None);
}

/// Store the kill-channel receiver for this thread.
/// Called by `spawn_script_thread` before launching `Executor::run`.
pub fn set_kill_rx(rx: std::sync::mpsc::Receiver<()>) {
    KILL_RX.with(|cell| *cell.borrow_mut() = Some(rx));
}

/// An open file handle — either a buffered reader or writer.
pub enum OpenFile {
    Reader(std::io::BufReader<std::fs::File>),
    Writer(std::io::BufWriter<std::fs::File>),
    Appender(std::io::BufWriter<std::fs::File>),
}

/// Core interpreter for the PASTA language.
///
/// Holds all mutable runtime state.  Statement/expression evaluation is
/// delegated to `ex_eval::eval_stmt` / `ex_eval::eval_expr`; frame/scope
/// helpers are in `ex_frame`.  `Executor` itself is the single orchestration
/// surface for external callers (REPL, batch runner, tests).
pub struct Executor {
    pub object_families: std::collections::HashMap<String, ObjectFamily>,
    pub objects: std::collections::HashMap<u64, ObjectInstance>,
    pub next_object_id: u64,

    pub verbose: bool,
    pub env: Environment,
    pub priorities: PriorityGraph,
    pub constraints: ConstraintEngine,
    pub diagnostics: Vec<String>,
    /// Metadata for windows/canvases: (width, height, is_open)
    pub gfx_handles: std::collections::HashMap<String, (usize, usize, bool)>,
    /// Live X11 windows keyed by the same handle string as canvases.
    /// Only populated when the x11 feature is enabled and a display is available.
    #[cfg(feature = "x11")]
    pub x11_windows:
        std::collections::HashMap<String, crate::stdlib::graphics::backend::x11::X11Window>,
    pub next_window_id: usize,
    pub next_canvas_id: usize,
    /// Primary canvas storage — 32-bit RGBA (0xFFRRGGBB per pixel)
    pub canvases: std::collections::HashMap<String, crate::stdlib::graphics::canvas::Canvas>,
    /// Per-window back buffers for double buffering.
    pub back_buffers: std::collections::HashMap<String, crate::stdlib::graphics::canvas::Canvas>,
    /// Per-handle grid cell sizing used by DRAW_GRID-style helpers.
    pub grid_configs: std::collections::HashMap<String, GridConfig>,
    /// Current drawing color used by SET_COLOR, canvas_fill_rect (5-arg), and all primitives
    pub current_color: u32,
    /// Active SET_DRAW_TARGET window handle (draw calls on this handle go to back buffer)
    pub draw_target: Option<String>,
    pub rng: Rng,
    pub while_limit: usize,
    pub functions: HashMap<String, (Vec<crate::parser::Identifier>, Vec<Statement>)>,
    pub gc: Strainer,
    pub traceback: Traceback,
    pub shell: Shell,
    pub control_flow: Option<ControlFlowSignal>,
    pub module_registry: ModuleRegistry,
    /// Cached loaded module exports: module_name -> (export_name -> Value)
    pub loaded_modules: std::collections::HashMap<String, std::collections::HashMap<String, Value>>,
    /// Pointer registry for MEM/FILE/DEV/NET pointers (v1.4.4)
    pub pointer_registry: SharedPointerRegistry,
    /// Current pointer context for GOTO/PULL/PUSH operations (v1.4.4)
    pub pointer_context: PointerContext,
    /// GC tracker for pointer lifetimes (v1.4.4)
    pub pointer_gc_tracker: crate::runtime::PointerGcTracker,

    // ── Extension System ──────────────────────────────────────────────────
    /// Registry of dynamically loadable builtin functions
    /// Maps function name to implementation function
    pub builtin_registry: std::collections::HashMap<String, fn(Vec<Value>) -> Result<Value>>,
    /// Loaded extensions to prevent double-loading
    pub loaded_extensions: std::collections::HashSet<String>,
    /// Registry of stdlib module names that can be satisfied by Rust-level builtins.
    /// Maps lowercase module name → list of function names it exports.
    pub stdlib_module_exports: std::collections::HashMap<String, Vec<String>>,
    /// Functions that have been called at least once (used for RET.LATE WHEN triggers).
    pub fired_events: std::collections::HashSet<String>,
    /// Pending deferred return value set by RET.LATE inside a function body.
    /// The function call site picks this up after the body finishes executing.
    pub ret_late_pending: Option<crate::interpreter::environment::Value>,
    /// Family object system registry
    pub family_registry: crate::runtime::family::FamilyRegistry,
    /// Unsafe permission level for family events
    pub family_permission: crate::runtime::family::UnsafePermission,
    /// If this executor is running inside a pipeline stage, values sent via
    /// RETURN are forwarded downstream through this channel sender.
    pub pipeline_tx: Option<std::sync::mpsc::SyncSender<Value>>,
    /// Open file handles keyed by handle string (e.g. "file://1")
    pub open_files: std::collections::HashMap<String, OpenFile>,
    /// Counter for generating unique file handle IDs
    pub next_file_id: usize,
    /// FPS timing system — None until `FROM fps USE` is executed.
    pub fps_state: Option<FpsState>,
}

#[derive(Clone, Copy)]
pub struct GridConfig {
    /// Width of one grid cell in pixels.
    pub cell_width: usize,
    /// Height of one grid cell in pixels.
    pub cell_height: usize,
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Format an f64 for display: integers print without a decimal point.
#[inline]
pub(crate) fn fmt_f64(n: f64) -> String {
    if n.fract().abs() < f64::EPSILON {
        format!("{}", n.round() as i64)
    } else {
        format!("{}", n)
    }
}

#[allow(unused_macros)]
macro_rules! vdbg {
    ($self:expr, $($arg:tt)*) => {
        if $self.verbose && is_verbose_debug() {
            println!($($arg)*);
        }
    };
}

// ── impl Executor ─────────────────────────────────────────────────────────────

impl Executor {
    /// Construct a new `Executor` with default settings and an empty environment.
    pub fn new() -> Self {
        let mut exe = Self {
            env: Environment::new(),
            priorities: PriorityGraph::new(),
            constraints: ConstraintEngine::new(),
            diagnostics: Vec::new(),
            next_canvas_id: 1,
            gfx_handles: std::collections::HashMap::new(),
            #[cfg(feature = "x11")]
            x11_windows: std::collections::HashMap::new(),
            next_window_id: 1,
            canvases: std::collections::HashMap::new(),
            back_buffers: std::collections::HashMap::new(),
            grid_configs: std::collections::HashMap::new(),
            current_color: 0xFF000000u32,
            draw_target: None,
            rng: Rng::new(),
            while_limit: DEFAULT_WHILE_LIMIT,
            functions: HashMap::new(),
            gc: Strainer::new(),
            traceback: Traceback::default(),
            shell: Shell::default(),
            verbose: false,
            control_flow: None,
            object_families: std::collections::HashMap::new(),
            objects: std::collections::HashMap::new(),
            next_object_id: 1,
            module_registry: ModuleRegistry::new_with_loader(None),
            loaded_modules: std::collections::HashMap::new(),
            pointer_registry: crate::runtime::pointer::PointerRegistry::new_shared(),
            pointer_context: PointerContext::new(),
            pointer_gc_tracker: crate::runtime::PointerGcTracker::new(),
            builtin_registry: std::collections::HashMap::new(),
            loaded_extensions: std::collections::HashSet::new(),
            stdlib_module_exports: std::collections::HashMap::new(),
            fired_events: std::collections::HashSet::new(),
            ret_late_pending: None,
            family_registry: crate::runtime::family::FamilyRegistry::new(
                crate::runtime::family::UnsafePermission::None,
            ),
            family_permission: crate::runtime::family::UnsafePermission::None,
            pipeline_tx: None,
            open_files: std::collections::HashMap::new(),
            next_file_id: 1,
            fps_state: None,
        };

        // Populate stdlib module registry
        {
            let gfx_exports: Vec<String> = vec![
                "window",
                "window_create",
                "create_window",
                "window_is_open",
                "window_close",
                "window_poll",
                "window_key",
                "window_open",
                "canvas",
                "canvas_create",
                "create_canvas",
                "canvas_set_pixel",
                "canvas_get_pixel",
                "canvas_present",
                "canvas_clear",
                "canvas_fill_rect",
                "canvas_draw_rect",
                "canvas_draw_line",
                "canvas_draw_circle",
                "canvas_fill_circle",
                "canvas_draw_ellipse",
                "canvas_fill_ellipse",
                "canvas_draw_triangle",
                "canvas_fill_triangle",
                "canvas_draw_polygon",
                "canvas_fill_polygon",
                "canvas_draw_arc",
                "canvas_blit",
                "canvas_width",
                "canvas_height",
                "canvas_save_ppm",
                "pixel",
                "blit",
                "close",
                "window_set_pixel",
                "window_fill",
                "window_save",
                "set_color",
                "present",
                "swap_buffer",
                "clear_buffer",
                "draw_to_buffer",
                "set_draw_target",
                "copy_to_buffer",
                "pop_buffer",
                "blend_to_buffer",
                "scroll_buffer",
                "tint_buffer",
                "resize_buffer",
                "draw_grid",
                "draw_to_grid",
                "draw_grid_batch",
                "draw_grid_runs",
                "color_rgb",
                "color_rgba",
                "color_hsv",
                "color_lerp",
                "color_rgb16",
                "color_from565",
                "color_to565",
                "color_wheel",
                "palette565_size",
                "graphics_cleanup",
                "gfx_memory_usage",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            exe.stdlib_module_exports
                .insert("graphics".to_string(), gfx_exports);
        }

        // FPS timing module exports
        {
            let fps_exports: Vec<String> = vec![
                "fps_init",
                "fps_begin",
                "fps_end",
                "fps_tick",
                "fps_sleep",
                "fps_delta",
                "fps_get",
                "fps_target",
                "fps_set_target",
                "fps_frame_count",
                "fps_avg",
                "fps_paused",
                "fps_pause",
                "fps_resume",
                "fps_elapsed",
                "fps_frame_time",
                "fps_behind",
            ]
            .iter()
            .map(|s| s.to_string())
            .collect();
            exe.stdlib_module_exports
                .insert("fps".to_string(), fps_exports);
        }

        // Register named color constants unconditionally so color() and COLOR_*
        // are available without loading the full graphics extension.
        crate::stdlib::graphics::draw::register_color_palette(&mut exe.env);

        exe
    }

    // ── Object system API ─────────────────────────────────────────────────────

    #[allow(dead_code)]
    fn register_object_family(&mut self, family: ObjectFamily) {
        self.object_families.insert(family.name.clone(), family);
    }

    /// Resolve a module name to a filesystem path using the search order:
    /// 1) ./<name>.pm                           (CWD)
    /// 2) ./modules/<name>.pm                   (local modules dir)
    /// 3) $PASTA_MODULE_PATH/<name>.pm          (colon-separated env-var override)
    /// 4) ./src/stdlib/<name>.pm               (development tree)
    /// 5) ./stdlib/<name>.pm                   (legacy / installed layout)
    pub fn resolve_module_path(&self, name: &str) -> Result<String> {
        use std::path::PathBuf;
        let cwd = std::env::current_dir()?;
        let mut candidates = vec![
            cwd.join(format!("{}.pm", name)),
            cwd.join("modules").join(format!("{}.pm", name)),
        ];
        // Honour PASTA_MODULE_PATH (colon-separated list of directories).
        if let Ok(env_paths) = std::env::var("PASTA_MODULE_PATH") {
            for dir in env_paths.split(':').filter(|d| !d.is_empty()) {
                candidates.push(PathBuf::from(dir).join(format!("{}.pm", name)));
            }
        }
        // Development fallback: src/stdlib/ inside the project tree.
        candidates.push(cwd.join("src").join("stdlib").join(format!("{}.pm", name)));
        // Legacy layout used by early installers.
        candidates.push(cwd.join("stdlib").join(format!("{}.pm", name)));

        for p in &candidates {
            if p.exists() {
                return Ok(p.to_string_lossy().to_string());
            }
        }
        Err(anyhow!(
            "ModuleNotFound: '{}' (searched: {})",
            name,
            candidates
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ))
    }

    /// Load the named module (lazy) and populate `loaded_modules` with its exports.
    pub fn load_module(&mut self, name: &str) -> Result<()> {
        // If already loaded, nothing to do.
        if self.loaded_modules.contains_key(name) {
            return Ok(());
        }

        // If registry reports Loading -> circular import
        if let Some(meta) = self.module_registry.get(name) {
            if meta.state == crate::runtime::module_registry::ModuleState::Loading {
                return Err(anyhow!(
                    "CircularImportError: circular import detected while loading '{}'",
                    name
                ));
            }
        }

        // Ensure registry has an entry and mark loading.
        let path = match self.resolve_module_path(name) {
            Ok(p) => {
                self.module_registry.register(name.to_string(), p.clone());
                p
            }
            Err(e) => {
                self.module_registry.mark_failed(name, e.to_string());
                return Err(anyhow!(
                    "ModuleNotFoundError: could not find module '{}'",
                    name
                ));
            }
        };

        self.module_registry.mark_loading(name);

        // Read and parse the file
        let src = std::fs::read_to_string(&path)
            .map_err(|e| anyhow!("failed to read module '{}': {}", name, e))?;
        let tokens = crate::lexer::lexer::Lexer::new(&src).lex();
        let mut parser = crate::parser::parser::Parser::new(tokens);
        let program = parser.parse();

        // Collect exports from any ModuleDecl in the program.
        // DEFs and other top-level statements may live outside the MOD block, so we
        // execute ALL top-level statements (skipping the ModuleDecl itself to avoid
        // double-scoping its body).
        let mut exports_list: Vec<String> = Vec::new();
        for s in program.statements.iter() {
            if let crate::parser::Statement::ModuleDecl { exports, .. } = s {
                exports_list.extend(exports.iter().map(|i| i.name.clone()));
            }
        }

        // Execute all top-level statements in a fresh isolated scope.
        // Skipping ModuleDecl avoids executing its body twice (once when we hit the
        // ModuleDecl statement and once as free-standing top-level DEFs).
        use crate::interpreter::ex_frame;
        ex_frame::push_scope(&mut self.env, ScopeKind::Function); // module body

        // Pre-register all function definitions so forward-calls work.
        self.register_functions_recursive(&program.statements);

        for stmt in program.statements.iter() {
            // Skip the ModuleDecl wrapper — its DEFs were pre-registered above and
            // any free-standing DEFs at the top level are executed separately.
            if matches!(stmt, crate::parser::Statement::ModuleDecl { .. }) {
                // Still execute internal DEFs that are ONLY inside the MOD block body
                // by drilling into the body.
                if let crate::parser::Statement::ModuleDecl { body, .. } = stmt {
                    for inner in body.iter() {
                        if let Err(e) = self.execute_statement(inner) {
                            self.module_registry.mark_failed(name, e.to_string());
                            let _ = ex_frame::pop_scope(
                                &mut self.env,
                                "module load",
                                &mut self.diagnostics,
                            );
                            return Err(anyhow!(
                                "ModuleLoadError: failed to execute module '{}': {}",
                                name,
                                e
                            ));
                        }
                    }
                }
                continue;
            }
            if let Err(e) = self.execute_statement(stmt) {
                self.module_registry.mark_failed(name, e.to_string());
                let _ = ex_frame::pop_scope(&mut self.env, "module load", &mut self.diagnostics);
                return Err(anyhow!(
                    "ModuleLoadError: failed to execute module '{}': {}",
                    name,
                    e
                ));
            }
        }

        // Collect exports.
        let mut exports_map: std::collections::HashMap<String, Value> =
            std::collections::HashMap::new();
        for export_name in exports_list.iter() {
            if let Some(v) = self.env.get(export_name) {
                exports_map.insert(export_name.clone(), v.clone());
            } else {
                self.module_registry.mark_failed(
                    name,
                    format!("module '{}' does not export '{}'", name, export_name),
                );
                let _ = ex_frame::pop_scope(
                    &mut self.env,
                    "module export check",
                    &mut self.diagnostics,
                );
                return Err(anyhow!(
                    "ExportNotFoundError: module '{}' does not export '{}'",
                    name,
                    export_name
                ));
            }
        }

        // Pop module scope (we keep exported values cached separately)
        let _ = ex_frame::pop_scope(&mut self.env, "module load complete", &mut self.diagnostics);

        // Cache exports and mark module loaded
        self.loaded_modules.insert(name.to_string(), exports_map);
        self.module_registry.set_exports(name, Vec::new());
        self.module_registry.mark_loaded(name);
        Ok(())
    }

    /// Get an exported value from a loaded module, loading it if necessary.
    pub fn get_module_export(&mut self, module: &str, symbol: &str) -> Result<Value> {
        // Check if this is a stdlib (Rust-level) module.
        if let Some(exports) = self.stdlib_module_exports.get(module).cloned() {
            if exports.iter().any(|e| e == symbol) {
                // Load the matching extension if not already loaded.
                let _ = self.load_extension(module);
                return Ok(Value::Builtin(symbol.to_string()));
            }
            return Err(anyhow!(
                "ExportNotFoundError: module '{}' does not export '{}'",
                module,
                symbol
            ));
        }
        if !self.loaded_modules.contains_key(module) {
            self.load_module(module)?;
        }
        if let Some(exports) = self.loaded_modules.get(module) {
            if let Some(v) = exports.get(symbol) {
                return Ok(v.clone());
            }
        }
        Err(anyhow!(
            "ExportNotFoundError: module '{}' does not export '{}'",
            module,
            symbol
        ))
    }

    #[allow(dead_code)]
    fn instantiate_object(
        &mut self,
        family_name: &str,
        overrides: Option<&std::collections::HashMap<String, Value>>,
    ) -> anyhow::Result<u64> {
        let fam = match self.object_families.get(family_name) {
            Some(f) => f.clone(),
            None => return Err(anyhow!("unknown object family '{}'", family_name)),
        };
        let mut fields_map = std::collections::HashMap::new();
        for f in &fam.fields {
            match self.eval_expr(&f.value) {
                Ok(v) => {
                    fields_map.insert(f.name.name.clone(), v);
                }
                Err(_) => {
                    fields_map.insert(f.name.name.clone(), Value::None);
                }
            }
        }
        if let Some(ov) = overrides {
            for (k, v) in ov {
                fields_map.insert(k.clone(), v.clone());
            }
        }
        let id = self.next_object_id;
        self.next_object_id = self.next_object_id.saturating_add(1);
        let inst = ObjectInstance::new(id, family_name.to_string(), fields_map);
        self.objects.insert(id, inst);
        Ok(id)
    }

    #[allow(dead_code)]
    fn apply_mutation(&mut self, object_id: u64, mutation_name: &str) -> anyhow::Result<()> {
        let inst_family = {
            let inst = match self.objects.get(&object_id) {
                Some(i) => i,
                None => return Err(anyhow!("object id {} not found", object_id)),
            };
            inst.family.clone()
        };
        let mut_entry = {
            let fam = match self.object_families.get(&inst_family) {
                Some(f) => f,
                None => return Err(anyhow!("family '{}' not registered", inst_family)),
            };
            match fam
                .mutation_table
                .iter()
                .find(|m| m.name.name == mutation_name)
            {
                Some(me) => me.clone(),
                None => {
                    return Err(anyhow!(
                        "mutation '{}' not found on family '{}'",
                        mutation_name,
                        fam.name
                    ))
                }
            }
        };
        let body_clone = mut_entry.body.clone();
        match body_clone {
            crate::parser::MutBody::Expr(expr) => {
                let _ = self.eval_expr(&expr)?;
            }
            crate::parser::MutBody::Block(stmts) => {
                for s in &stmts {
                    let _ = self.execute_statement(s)?;
                    if self.control_flow.is_some() {
                        break;
                    }
                }
            }
        }
        if let Some(inst) = self.objects.get_mut(&object_id) {
            inst.applied_mutations.insert(mutation_name.to_string());
        }
        Ok(())
    }

    // ── Traceback (thin wrappers used by execute_statement) ───────────────────

    pub fn push_frame(&mut self, span: Span, ctx: impl Into<String>) {
        ex_frame::push_frame(&mut self.traceback, span, ctx);
    }

    pub fn pop_frame(&mut self) {
        ex_frame::pop_frame(&mut self.traceback);
    }

    // ── Configuration ─────────────────────────────────────────────────────────

    pub fn set_while_limit(&mut self, limit: usize) {
        self.while_limit = limit;
    }

    // ── Extension System ──────────────────────────────────────────────────────

    /// Register a builtin function for dynamic loading
    pub fn register_builtin(&mut self, name: String, func: fn(Vec<Value>) -> Result<Value>) {
        self.builtin_registry.insert(name, func);
    }

    /// Load a named extension module (idempotent - won't double-load)
    pub fn load_extension(&mut self, extension_name: &str) -> Result<()> {
        if self.loaded_extensions.contains(extension_name) {
            return Ok(()); // Already loaded
        }

        match extension_name {
            "graphics" => {
                // Load graphics API functions
                crate::stdlib::graphics::register_graphics_api(self);
                // Register named color palette as global constants
                crate::stdlib::graphics::draw::register_color_palette(&mut self.env);
                self.loaded_extensions.insert(extension_name.to_string());
                Ok(())
            }
            "fps" => {
                // FPS module: no Rust-level registrations needed — all handled
                // in the call_builtin match block.  Just mark as loaded so the
                // overhead check short-circuits on future calls.
                self.loaded_extensions.insert(extension_name.to_string());
                Ok(())
            }
            _ => Err(anyhow!("Unknown extension: {}", extension_name)),
        }
    }

    // ── GC ────────────────────────────────────────────────────────────────────

    pub fn collect_garbage(&mut self) -> usize {
        let roots = self.env.all_values();
        self.gc.collect_with_roots(&roots)
    }

    /// Follow `Value::Heap` handles until a concrete value is reached.
    pub fn deref(&self, mut v: Value) -> Value {
        while let Value::Heap(id) = v {
            match self.gc.get(id) {
                Some(inner) => v = inner.clone(),
                None => return Value::None,
            }
        }
        v
    }

    /// Like `deref`, but stops at the first `Heap` reference pointing to a mutable
    /// container (Dict or List). This preserves heap handles so that callers can
    /// mutate the container in-place via `dict_set` / `list_append` etc.
    pub fn deref_return(&self, v: Value) -> Value {
        if let Value::Heap(id) = &v {
            // Peek at what the heap holds. If it's a mutable container, keep the
            // Heap reference so the caller can mutate it.
            if let Some(inner) = self.gc.get(*id) {
                if matches!(*inner, Value::Dict(_) | Value::List(_)) {
                    return v; // preserve Heap(id)
                }
            }
        }
        self.deref(v)
    }
    // ── Header loading ────────────────────────────────────────────────────────

    fn load_header_if_exists(&mut self, path: &str) {
        let p = Path::new(path);
        if !p.exists() {
            return;
        }
        match fs::read_to_string(p) {
            Ok(src) => match Lexer::new(&src).lex_result() {
                Ok(tokens) => {
                    let mut parser = Parser::new(tokens);
                    let (program, parse_diags) = parser.parse_with_diagnostics();
                    for d in &parse_diags {
                        eprintln!(
                            "Parse error at {}:{}: {}",
                            d.span.start_line, d.span.start_col, d.message
                        );
                    }
                    match self.execute_repl(&program) {
                        Ok(()) => {
                            if self.verbose {
                                self.diagnostics.push(format!("Loaded header {}", path));
                            }
                        }
                        Err(e) => {
                            let msg = format!("Failed to execute header {}: {}", path, e);
                            if !msg.contains("Undefined variable") {
                                self.diagnostics.push(msg);
                            }
                        }
                    }
                }
                Err(e) => {
                    self.diagnostics
                        .push(format!("Lex error loading {}: {}:{}", path, e.line, e.col));
                }
            },
            Err(e) => {
                self.diagnostics
                    .push(format!("I/O error reading {}: {}", path, e));
            }
        }
    }

    // ── Top-level execution ───────────────────────────────────────────────────

    fn register_functions_recursive(&mut self, stmts: &[Statement]) {
        for stmt in stmts {
            match stmt {
                Statement::FunctionDef {
                    name, params, body, ..
                } => {
                    self.functions
                        .insert(name.name.clone(), (params.clone(), body.clone()));
                    /* Inserted alias registration: if the function name contains a dot (e.g., "def.swap"),
                    also register the short name ("swap") so calls using either form resolve. */
                    if let Some(pos) = name.name.rfind('.') {
                        let short = name.name[pos + 1..].to_string();
                        if !self.functions.contains_key(&short) {
                            self.functions
                                .insert(short.clone(), (params.clone(), body.clone()));
                        }
                        if params.is_empty() {
                            self.env.set_global(
                                short,
                                Value::Lambda(params.clone(), body.clone(), Default::default()),
                            );
                        }
                    }

                    if params.is_empty() {
                        self.env.set_global(
                            name.name.clone(),
                            Value::Lambda(params.clone(), body.clone(), Default::default()),
                        );
                    }
                    self.register_functions_recursive(body);
                }
                Statement::DoBlock { body, .. } => self.register_functions_recursive(body),
                Statement::WhileBlock { body, .. } => self.register_functions_recursive(body),
                Statement::ForIn { body, .. } => self.register_functions_recursive(body),
                Statement::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    self.register_functions_recursive(then_body);
                    if let Some(eb) = else_body {
                        self.register_functions_recursive(eb);
                    }
                }
                Statement::DefDoUntil(d) => self.register_functions_recursive(&d.body),
                _ => {}
            }
        }
    }

    /// Execute a complete parsed `Program` (two-pass: register defs, then eval).
    pub fn execute_program(&mut self, program: &Program) -> Result<()> {
        self.register_functions_recursive(&program.statements);
        for stmt in &program.statements {
            if let Statement::FunctionDef {
                name,
                params,
                body,
                span: _,
            } = stmt
            {
                self.functions
                    .insert(name.name.clone(), (params.clone(), body.clone()));
                // Bind ALL defs as lambdas — not just zero-param ones.
                // Higher-order code (apply_twice(inc, 5)) needs `inc` as a Value::Lambda.
                self.env.set_global(
                    name.name.clone(),
                    Value::Lambda(params.clone(), body.clone(), Default::default()),
                );
            }
        }

        let stmts = &program.statements;
        let mut i = 0;
        while i < stmts.len() {
            let stmt = &stmts[i];
            // Do NOT skip FunctionDef — eval_stmt handles nested/local defs correctly.
            // DefDoUntil is still a no-op at eval time.
            if matches!(stmt, Statement::DefDoUntil(_)) {
                i += 1;
                continue;
            }
            if let Err(e) = self.execute_statement(stmt) {
                if self.verbose && is_verbose_debug() {
                    for msg in &self.diagnostics {
                        println!("note: {}", msg);
                    }
                }
                return Err(e);
            }
            let _ = self.collect_garbage();
            // Handle top-level control flow signals
            match self.control_flow.take() {
                None => {}
                Some(ControlFlowSignal::GotoLabel(ref label)) => {
                    // Forward GOTO: scan forward to find the matching LoopBlock and jump to it.
                    let label_clone = label.clone();
                    let target = stmts[i + 1..].iter().position(
                        |s| matches!(s, Statement::LoopBlock { name, .. } if name == &label_clone),
                    );
                    if let Some(offset) = target {
                        // Jump: skip to the LoopBlock (it will handle the signal on entry).
                        i = i + 1 + offset;
                        // Re-set the signal so the LoopBlock handler sees it.
                        self.control_flow = Some(ControlFlowSignal::GotoLabel(label_clone));
                        continue;
                    }
                    // Label not found ahead — clear and continue (label was already passed/handled)
                }
                Some(_) => {} // Break/Return/etc at top level — clear and continue
            }
            i += 1;
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

    /// Execute a program in REPL mode (no pre-pass, stops on control-flow signal).
    pub fn execute_repl(&mut self, program: &Program) -> Result<()> {
        /* PASTA_PIPELINE_SHORT_CIRCUIT */
        // --- pipeline short-circuit: if the program is a single top-level
        // expression that is a binary Pipe, spawn a detached pipeline and return.
        if program.statements.len() == 1 {
            use crate::parser::ast::{BinaryOp, Expr};
            if let crate::parser::ast::Statement::ExprStmt { expr, .. } = &program.statements[0] {
                if let Expr::Binary {
                    op, left, right, ..
                } = expr
                {
                    if *op == BinaryOp::Pipe {
                        // spawn detached pipeline and return immediately
                        let name_hint = "pipeline";
                        // expr is a &Box<Expr>, clone the boxed exprs so threads own them
                        let left_clone = left.clone();
                        let right_clone = right.clone();
                        // spawn pipeline (registers threads so :threads will show them)
                        if let Err(e) =
                            self.spawn_pipeline_detached(left_clone, right_clone, name_hint)
                        {
                            eprintln!("Failed to spawn pipeline: {}", e);
                        }
                        return Ok(());
                    }
                }
            }
        }

        for stmt in &program.statements {
            self.execute_statement(stmt)?;
            if self.control_flow.is_some() {
                break;
            }
        }
        Ok(())
    }

    // ── Statement execution ───────────────────────────────────────────────────

    /// Dispatch a single AST statement.
    ///
    /// Delegates to `ex_eval::eval_stmt` for all logic; this wrapper exists so
    /// that external call-sites (`execute_program`, `execute_repl`,
    /// `apply_mutation`, test helpers) continue to use a stable method name.
    pub fn execute_statement(&mut self, stmt: &Statement) -> Result<Option<Value>> {
        // Poll the kill channel (non-blocking). If a signal arrives, set the
        // Killed control-flow flag so all enclosing loops unwind immediately.
        let killed = KILL_RX.with(|cell| {
            if let Some(ref rx) = *cell.borrow() {
                rx.try_recv().is_ok()
            } else {
                false
            }
        });
        if killed {
            self.control_flow = Some(ControlFlowSignal::Killed);
            return Err(anyhow::anyhow!("thread killed"));
        }
        // Compute the span for this statement so the traceback frame can
        // show a meaningful location.  Mirrors the span selection logic
        // used by `ex_eval::eval_stmt`.
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

        ex_frame::push_frame(&mut self.traceback, span.clone(), "statement");
        let res = ex_eval::eval_stmt(self, stmt);
        ex_frame::pop_frame(&mut self.traceback);
        res
    }

    // ── run_stmt (public orchestration API described in the split spec) ───────

    /// High-level run-one-statement API.  Delegates to `execute_statement`.
    pub fn run_stmt(&mut self, stmt: &Statement) -> Result<()> {
        self.execute_statement(stmt)?;
        Ok(())
    }

    // ── Expression evaluation (public delegation to ex_eval) ─────────────────

    /// Evaluate an expression.  Delegates to `ex_eval::eval_expr`.
    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value> {
        ex_eval::eval_expr(self, expr)
    }

    // ── call_value (public orchestration API) ─────────────────────────────────

    /// Call a `Value::Lambda` or named function with the given arguments.
    ///
    /// Used by `ex_eval` for higher-order calls and exposed publicly for
    /// `int_api` / `ModuleEnvHandle` implementations.
    pub fn call_value(&mut self, val: Value, args: Vec<Value>) -> Result<Value> {
        let val = self.deref(val);
        match val {
            Value::Lambda(params, stmts, _captures) => {
                self.env.push_scope(ScopeKind::Function); // call_value lambda
                                                          // Bind by param name if available, otherwise positional
                if !params.is_empty() {
                    for (i, param) in params.iter().enumerate() {
                        let v = args.get(i).cloned().unwrap_or(Value::None);
                        self.env.set_local(param.name.clone(), v);
                    }
                } else {
                    for (i, v) in args.iter().enumerate() {
                        self.env.set_local(format!("__arg_{}__", i), v.clone());
                    }
                }
                let mut last = Value::None;
                for s in stmts.iter() {
                    if let Some(v) = self.execute_statement(s)? {
                        last = v;
                    }
                    if let Some(ControlFlowSignal::Return(ret_val)) = self.control_flow.take() {
                        let ret_concrete = self.deref(ret_val);
                        if let Err(e) = self.env.pop_scope() {
                            self.diagnostics
                                .push(format!("Warning: pop_scope in call_value: {}", e));
                        }
                        return Ok(ret_concrete);
                    }
                }
                if let Err(e) = self.env.pop_scope() {
                    self.diagnostics
                        .push(format!("Warning: pop_scope after call_value: {}", e));
                }
                if let Some(p) = self.ret_late_pending.take() {
                    last = p;
                }
                Ok(last)
            }
            Value::Builtin(name) => self.call_builtin(&name, args),
            other => Err(anyhow!("call_value: not callable: {:?}", other)),
        }
    }

    // ── execute_value_as_callable (used by DoBlock list dispatch) ─────────────

    pub fn execute_value_as_callable(
        &mut self,
        value: &Value,
        alias: &Option<Identifier>,
        name_hint: &str,
    ) -> Result<()> {
        match value {
            Value::Lambda(_, stmts, _) => {
                // Push a call-site frame so runtime errors inside env-stored
                // lambdas include the call origin in the traceback.
                self.push_frame(
                    crate::parser::ast::Span::dummy(),
                    format!("call {}", name_hint),
                );
                let stmts = stmts.clone();
                self.env.push_scope(ScopeKind::Function); // call_value env-lambda
                if let Some(a) = alias {
                    self.env
                        .set_local(a.name.clone(), Value::String(name_hint.to_string()));
                }
                for s in stmts.iter() {
                    self.execute_statement(s)?;
                    if self.control_flow.is_some() {
                        break;
                    }
                    let _ = self.collect_garbage();
                }
                if let Err(e) = self.env.pop_scope() {
                    self.diagnostics.push(format!(
                        "Warning: pop_scope after list-item lambda call: {}",
                        e
                    ));
                }
                self.pop_frame();
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

    // ── Truthiness ────────────────────────────────────────────────────────────

    pub fn value_is_truthy(&self, v: &Value) -> bool {
        let v = self.deref(v.clone());
        match &v {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty(),
            Value::List(l) => !l.is_empty(),
            Value::Tensor(t) => t.numel() > 0,
            Value::None => false,
            Value::Lambda(_, _, _) => true,
            Value::LazyImport { .. } => true,
            Value::Dict(m) => !m.is_empty(),
            Value::Heap(_) => true,
            Value::Pending(_, _) => true,
            Value::Pointer(_) => true,
            Value::FamilyNode { .. } => true,
            Value::Builtin(_) => true,
        }
    }

    // ── Builtins ──────────────────────────────────────────────────────────────

    fn get_canvas_for_draw(
        &mut self,
        handle: &str,
    ) -> Option<&mut crate::stdlib::graphics::canvas::Canvas> {
        if self.draw_target.as_deref() == Some(handle) {
            self.back_buffers.get_mut(handle)
        } else {
            self.canvases.get_mut(handle)
        }
    }

    fn resolve_grid_target(&self, explicit: Option<&str>) -> Result<String> {
        if let Some(handle) = explicit {
            return Ok(handle.to_string());
        }
        self.draw_target.clone().ok_or_else(|| {
            anyhow!("grid draw requires SET_DRAW_TARGET(window) or an explicit handle")
        })
    }

    fn grid_config_for(&self, handle: &str) -> Result<GridConfig> {
        self.grid_configs.get(handle).copied().ok_or_else(|| {
            anyhow!(
                "DRAW_GRID must be called before grid drawing on '{}'",
                handle
            )
        })
    }

    fn cleanup_graphics(&mut self) {
        #[cfg(feature = "x11")]
        {
            for (_, mut xwin) in self.x11_windows.drain() {
                xwin.close();
            }
        }
        self.canvases.clear();
        self.back_buffers.clear();
        self.gfx_handles.clear();
        self.grid_configs.clear();
        self.draw_target = None;
        self.next_window_id = 1;
        self.next_canvas_id = 1;
    }

    fn parse_grid_batch_cells(
        &self,
        cells: &[Value],
        default_color: u32,
    ) -> Result<Vec<(isize, isize, u32)>> {
        let mut out = Vec::with_capacity(cells.len());
        for entry in cells {
            let entry = self.deref(entry.clone());
            match entry {
                Value::List(items) => match items.as_slice() {
                    [Value::Number(x), Value::Number(y)] => {
                        out.push((*x as isize, *y as isize, default_color));
                    }
                    [Value::Number(x), Value::Number(y), Value::Number(color)] => {
                        out.push((*x as isize, *y as isize, *color as u32));
                    }
                    _ => {
                        return Err(anyhow!(
                            "DRAW_GRID_BATCH items must be [x, y] or [x, y, color]"
                        ))
                    }
                },
                Value::Dict(map) => {
                    let x = match map.get("x") {
                        Some(Value::Number(n)) => *n as isize,
                        _ => return Err(anyhow!("DRAW_GRID_BATCH dict items require numeric 'x'")),
                    };
                    let y = match map.get("y") {
                        Some(Value::Number(n)) => *n as isize,
                        _ => return Err(anyhow!("DRAW_GRID_BATCH dict items require numeric 'y'")),
                    };
                    let color = match map.get("color") {
                        Some(Value::Number(n)) => *n as u32,
                        Some(_) => {
                            return Err(anyhow!("DRAW_GRID_BATCH dict 'color' must be numeric"))
                        }
                        None => default_color,
                    };
                    out.push((x, y, color));
                }
                _ => return Err(anyhow!("DRAW_GRID_BATCH items must be lists or dicts")),
            }
        }
        Ok(out)
    }

    fn parse_grid_runs(
        &self,
        runs: &[Value],
        default_color: u32,
    ) -> Result<Vec<(isize, isize, usize, u32)>> {
        let mut out = Vec::with_capacity(runs.len());
        for entry in runs {
            let entry = self.deref(entry.clone());
            match entry {
                Value::List(items) => match items.as_slice() {
                    [Value::Number(x), Value::Number(y), Value::Number(len)] => {
                        let len = (*len).round() as isize;
                        if len <= 0 {
                            return Err(anyhow!("DRAW_GRID_RUNS run length must be > 0"));
                        }
                        out.push((*x as isize, *y as isize, len as usize, default_color));
                    }
                    [Value::Number(x), Value::Number(y), Value::Number(len), Value::Number(color)] =>
                    {
                        let len = (*len).round() as isize;
                        if len <= 0 {
                            return Err(anyhow!("DRAW_GRID_RUNS run length must be > 0"));
                        }
                        out.push((*x as isize, *y as isize, len as usize, *color as u32));
                    }
                    _ => {
                        return Err(anyhow!(
                            "DRAW_GRID_RUNS items must be [x, y, len] or [x, y, len, color]"
                        ))
                    }
                },
                Value::Dict(map) => {
                    let x = match map.get("x") {
                        Some(Value::Number(n)) => *n as isize,
                        _ => return Err(anyhow!("DRAW_GRID_RUNS dict items require numeric 'x'")),
                    };
                    let y = match map.get("y") {
                        Some(Value::Number(n)) => *n as isize,
                        _ => return Err(anyhow!("DRAW_GRID_RUNS dict items require numeric 'y'")),
                    };
                    let len = match map.get("len") {
                        Some(Value::Number(n)) => (*n).round() as isize,
                        _ => {
                            return Err(anyhow!("DRAW_GRID_RUNS dict items require numeric 'len'"))
                        }
                    };
                    if len <= 0 {
                        return Err(anyhow!("DRAW_GRID_RUNS run length must be > 0"));
                    }
                    let color = match map.get("color") {
                        Some(Value::Number(n)) => *n as u32,
                        Some(_) => {
                            return Err(anyhow!("DRAW_GRID_RUNS dict 'color' must be numeric"))
                        }
                        None => default_color,
                    };
                    out.push((x, y, len as usize, color));
                }
                _ => return Err(anyhow!("DRAW_GRID_RUNS items must be lists or dicts")),
            }
        }
        Ok(out)
    }

    pub fn call_builtin(&mut self, name: &str, args: Vec<Value>) -> Result<Value> {
        // Check extension registry first
        if let Some(&builtin_func) = self.builtin_registry.get(name) {
            return builtin_func(args);
        }

        // Python-style type introspection — deref heap handles so lists stored
        // via GC report "list" instead of "heap".
        if name == "type" {
            if args.len() != 1 {
                return Err(anyhow!("type expects 1 argument"));
            }
            let dereffed = self.deref(args.into_iter().next().unwrap());
            let t = match &dereffed {
                Value::Number(_) => "number",
                Value::String(_) => "string",
                Value::Bool(_) => "bool",
                Value::List(_) => "list",
                Value::Tensor(_) => "tensor",
                Value::Lambda(_, _, _) => "lambda",
                Value::LazyImport { .. } => "lazy",
                Value::None => "none",
                Value::Heap(_) => "heap",
                Value::Dict(_) => "dict",
                Value::Pending(_, _) => "pending",
                Value::Pointer(_) => "pointer",
                Value::FamilyNode { .. } => "family_node",
                Value::Builtin(_) => "builtin",
            };
            return Ok(Value::String(t.to_string()));
        }
        if name == "str"
            || name == "string"
            || name == "STRING"
            || name == "to_str"
            || name == "to_string"
        {
            if args.len() != 1 {
                return Err(anyhow!("str expects 1 argument"));
            }
            return Ok(Value::String(Executor::value_to_string(&args[0])));
        }

        // Mutable container builtins need raw Heap handles (before eager deref).
        match name {
            "list_append" | "LIST_APPEND" => {
                if args.len() != 2 {
                    return Err(anyhow!("list_append expects 2 arguments: list, value"));
                }
                let id = match &args[0] {
                    Value::Heap(id) => *id,
                    _ => return Err(anyhow!("list_append: first argument must be a list handle")),
                };
                self.gc
                    .append_to_list(id, args[1].clone())
                    .map_err(|e| anyhow!("list_append: {}", e))?;
                return Ok(Value::None);
            }
            "dict_set" | "DICT_SET" => {
                if args.len() != 3 {
                    return Err(anyhow!("dict_set expects 3 arguments: dict, key, value"));
                }
                let key = Executor::value_to_string(&self.deref(args[1].clone()));
                let val = self.deref(args[2].clone());
                let id = match &args[0] {
                    Value::Heap(id) => *id,
                    _ => return Err(anyhow!("dict_set: first argument must be a dict handle")),
                };
                let mut map = match self.gc.get(id) {
                    Some(Value::Dict(m)) => m.clone(),
                    _ => return Err(anyhow!("dict_set: handle does not point to a dict")),
                };
                map.insert(key, val);
                self.gc.set_or_allocate(id, Value::Dict(map));
                return Ok(Value::None);
            }
            "dict_delete" | "DICT_DELETE" => {
                if args.len() != 2 {
                    return Err(anyhow!("dict_delete expects 2 arguments: dict, key"));
                }
                let key = Executor::value_to_string(&self.deref(args[1].clone()));
                let id = match &args[0] {
                    Value::Heap(id) => *id,
                    _ => return Err(anyhow!("dict_delete: first argument must be a dict handle")),
                };
                let mut map = match self.gc.get(id) {
                    Some(Value::Dict(m)) => m.clone(),
                    _ => return Err(anyhow!("dict_delete: handle does not point to a dict")),
                };
                map.remove(&key);
                self.gc.set_or_allocate(id, Value::Dict(map));
                return Ok(Value::None);
            }
            _ => {}
        }

        // Eagerly deref heap handles so builtin arms never see `Value::Heap`.
        let args: Vec<Value> = args.into_iter().map(|v| self.deref(v)).collect();

        if name.eq_ignore_ascii_case("WINDOW_SAVE") {
            if args.len() != 2 {
                return Err(anyhow!("WINDOW_SAVE expects 2 args (window_handle, path)"));
            }
            match (&args[0], &args[1]) {
                (Value::String(h), Value::String(path)) => {
                    let canvas = match self.canvases.get(h) {
                        Some(c) => c,
                        None => return Err(anyhow!("WINDOW_SAVE: unknown window handle")),
                    };
                    canvas
                        .save_ppm(path)
                        .map_err(|e| anyhow!("WINDOW_SAVE: {}", e))?;
                    return Ok(Value::None);
                }
                _ => {
                    return Err(anyhow!(
                        "WINDOW_SAVE: expected (window_handle:string, path:string)"
                    ))
                }
            }
        }

        // Extension loading system
        if name.eq_ignore_ascii_case("LOAD_EXTENSION") {
            if args.len() != 1 {
                return Err(anyhow!(
                    "LOAD_EXTENSION requires 1 argument: extension_name"
                ));
            }
            match &args[0] {
                Value::String(ext_name) => {
                    self.load_extension(ext_name)?;
                    return Ok(Value::None);
                }
                _ => return Err(anyhow!("Extension name must be a string")),
            }
        }

        let _n = name.to_ascii_lowercase();
        match _n.as_str() {
            "window" | "window_create" => {
                // window(title, w, h) or window(title, w, h, buf_w, buf_h) -> handle
                if args.len() < 3 {
                    return Err(anyhow!(
                        "WINDOW expects at least 3 args (title, width, height)"
                    ));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(title), Value::Number(w), Value::Number(h)) => {
                        let width = *w as usize;
                        let height = *h as usize;
                        let buf_w = if args.len() >= 5 {
                            if let Value::Number(bw) = &args[3] {
                                *bw as usize
                            } else {
                                width
                            }
                        } else {
                            width
                        };
                        let buf_h = if args.len() >= 5 {
                            if let Value::Number(bh) = &args[4] {
                                *bh as usize
                            } else {
                                height
                            }
                        } else {
                            height
                        };
                        if width == 0 || height == 0 {
                            return Err(anyhow!("WINDOW: width and height must be > 0"));
                        }
                        let handle = format!("win://{}", self.next_window_id);
                        self.next_window_id = self.next_window_id.saturating_add(1);
                        self.canvases.insert(
                            handle.clone(),
                            crate::stdlib::graphics::canvas::Canvas::new(width, height),
                        );
                        self.back_buffers.insert(
                            handle.clone(),
                            crate::stdlib::graphics::canvas::Canvas::new(buf_w, buf_h),
                        );
                        self.gfx_handles
                            .insert(handle.clone(), (width, height, true));
                        #[cfg(feature = "x11")]
                        {
                            match crate::stdlib::graphics::backend::x11::X11Window::new(
                                title, width, height,
                            ) {
                                Ok(xwin) => {
                                    self.x11_windows.insert(handle.clone(), xwin);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[pasta/gfx] X11 unavailable ({}), using headless",
                                        e
                                    );
                                }
                            }
                        }
                        Ok(Value::String(handle))
                    }
                    _ => Err(anyhow!(
                        "WINDOW: expected (title:string, width:number, height:number)"
                    )),
                }
            }

            "create_window" => {
                // create_window(title, w, h) or create_window(title, w, h, buf_w, buf_h) -> window_handle
                if args.len() < 3 {
                    return Err(anyhow!(
                        "CREATE_WINDOW expects at least 3 args (title, width, height)"
                    ));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(title), Value::Number(w), Value::Number(h)) => {
                        let width = *w as usize;
                        let height = *h as usize;
                        let buf_w = if args.len() >= 5 {
                            if let Value::Number(bw) = &args[3] {
                                *bw as usize
                            } else {
                                width
                            }
                        } else {
                            width
                        };
                        let buf_h = if args.len() >= 5 {
                            if let Value::Number(bh) = &args[4] {
                                *bh as usize
                            } else {
                                height
                            }
                        } else {
                            height
                        };
                        if width == 0 || height == 0 {
                            return Err(anyhow!("CREATE_WINDOW: width and height must be > 0"));
                        }
                        let handle = format!("win://{}", self.next_window_id);
                        self.next_window_id = self.next_window_id.saturating_add(1);
                        self.canvases.insert(
                            handle.clone(),
                            crate::stdlib::graphics::canvas::Canvas::new(width, height),
                        );
                        self.back_buffers.insert(
                            handle.clone(),
                            crate::stdlib::graphics::canvas::Canvas::new(buf_w, buf_h),
                        );
                        self.gfx_handles
                            .insert(handle.clone(), (width, height, true));
                        #[cfg(feature = "x11")]
                        {
                            match crate::stdlib::graphics::backend::x11::X11Window::new(
                                title, width, height,
                            ) {
                                Ok(xwin) => {
                                    self.x11_windows.insert(handle.clone(), xwin);
                                }
                                Err(e) => {
                                    eprintln!(
                                        "[pasta/gfx] X11 unavailable ({}), using headless",
                                        e
                                    );
                                }
                            }
                        }
                        Ok(Value::String(handle))
                    }
                    _ => Err(anyhow!(
                        "CREATE_WINDOW: expected (title:string, width:number, height:number)"
                    )),
                }
            }

            "canvas" | "canvas_create" | "create_canvas" => {
                // canvas(w, h) -> canvas_handle:string
                if args.len() != 2 {
                    return Err(anyhow!("CANVAS expects 2 args (width, height)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(w), Value::Number(h)) => {
                        let width = *w as usize;
                        let height = *h as usize;
                        if width == 0 || height == 0 {
                            return Err(anyhow!("CANVAS: width and height must be > 0"));
                        }
                        let handle = format!("canvas://{}", self.next_window_id);
                        self.next_window_id = self.next_window_id.saturating_add(1);
                        self.canvases.insert(
                            handle.clone(),
                            crate::stdlib::graphics::canvas::Canvas::new(width, height),
                        );
                        self.gfx_handles
                            .insert(handle.clone(), (width, height, true));
                        Ok(Value::String(handle))
                    }
                    _ => Err(anyhow!("CANVAS: expected (width:number, height:number)")),
                }
            }

            "set_color" => {
                // SET_COLOR(packed_color) or SET_COLOR(r, g, b)
                match args.len() {
                    1 => match &args[0] {
                        Value::Number(n) => {
                            self.current_color = *n as u32;
                            Ok(Value::None)
                        }
                        _ => Err(anyhow!("SET_COLOR: expected a packed color number")),
                    },
                    3 => match (&args[0], &args[1], &args[2]) {
                        (Value::Number(rn), Value::Number(gn), Value::Number(bn)) => {
                            let r = (*rn as i32).clamp(0, 255) as u32;
                            let g = (*gn as i32).clamp(0, 255) as u32;
                            let b = (*bn as i32).clamp(0, 255) as u32;
                            self.current_color = 0xFF000000 | (r << 16) | (g << 8) | b;
                            Ok(Value::None)
                        }
                        _ => Err(anyhow!(
                            "SET_COLOR: expected (r:number, g:number, b:number)"
                        )),
                    },
                    _ => Err(anyhow!(
                        "SET_COLOR expects 1 arg (packed color) or 3 args (r, g, b)"
                    )),
                }
            }

            "draw_grid" => {
                let (explicit_handle, width_arg, height_arg) = match args.as_slice() {
        [Value::Number(w), Value::Number(h)] => (None, w, h),
        [Value::String(handle), Value::Number(w), Value::Number(h)] => (Some(handle.as_str()), w, h),
        _ => return Err(anyhow!("DRAW_GRID expects (cell_width, cell_height) or (handle, cell_width, cell_height)")),
    };

                let handle = self.resolve_grid_target(explicit_handle)?;
                let cell_width = (*width_arg).round() as isize;
                let cell_height = (*height_arg).round() as isize;
                if cell_width <= 0 || cell_height <= 0 {
                    return Err(anyhow!("DRAW_GRID cell dimensions must be > 0"));
                }
                if !self.gfx_handles.contains_key(&handle)
                    && !self.canvases.contains_key(&handle)
                    && !self.back_buffers.contains_key(&handle)
                {
                    return Err(anyhow!("DRAW_GRID: unknown handle '{}'", handle));
                }

                self.grid_configs.insert(
                    handle,
                    GridConfig {
                        cell_width: cell_width as usize,
                        cell_height: cell_height as usize,
                    },
                );
                Ok(Value::None)
            }

            "draw_to_grid" => {
                let (explicit_handle, x, y, color) = match args.as_slice() {
        [Value::Number(x), Value::Number(y)] => (None, *x as isize, *y as isize, self.current_color),
        [Value::Number(x), Value::Number(y), Value::Number(color)] => (None, *x as isize, *y as isize, *color as u32),
        [Value::String(handle), Value::Number(x), Value::Number(y)] => (Some(handle.as_str()), *x as isize, *y as isize, self.current_color),
        [Value::String(handle), Value::Number(x), Value::Number(y), Value::Number(color)] => (Some(handle.as_str()), *x as isize, *y as isize, *color as u32),
        _ => return Err(anyhow!("DRAW_TO_GRID expects (x, y), (x, y, color), (handle, x, y), or (handle, x, y, color)")),
    };

                let handle = self.resolve_grid_target(explicit_handle)?;
                let grid = self.grid_config_for(&handle)?;
                if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                    crate::stdlib::graphics::draw::fill_grid_cell(
                        canvas,
                        grid.cell_width,
                        grid.cell_height,
                        x,
                        y,
                        color,
                    );
                    Ok(Value::None)
                } else {
                    Err(anyhow!("DRAW_TO_GRID: unknown handle '{}'", handle))
                }
            }

            "draw_grid_batch" => {
                let (explicit_handle, cells_value) = match args.as_slice() {
                    [Value::List(cells)] => (None, cells),
                    [Value::String(handle), Value::List(cells)] => (Some(handle.as_str()), cells),
                    _ => {
                        return Err(anyhow!(
                            "DRAW_GRID_BATCH expects (cells) or (handle, cells)"
                        ))
                    }
                };

                let handle = self.resolve_grid_target(explicit_handle)?;
                let grid = self.grid_config_for(&handle)?;
                let cells = self.parse_grid_batch_cells(cells_value, self.current_color)?;
                if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                    crate::stdlib::graphics::draw::fill_grid_cells(
                        canvas,
                        grid.cell_width,
                        grid.cell_height,
                        &cells,
                    );
                    Ok(Value::None)
                } else {
                    Err(anyhow!("DRAW_GRID_BATCH: unknown handle '{}'", handle))
                }
            }

            "draw_grid_runs" => {
                let (explicit_handle, runs_value) = match args.as_slice() {
                    [Value::List(runs)] => (None, runs),
                    [Value::String(handle), Value::List(runs)] => (Some(handle.as_str()), runs),
                    _ => return Err(anyhow!("DRAW_GRID_RUNS expects (runs) or (handle, runs)")),
                };

                let handle = self.resolve_grid_target(explicit_handle)?;
                let grid = self.grid_config_for(&handle)?;
                let runs = self.parse_grid_runs(runs_value, self.current_color)?;
                if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                    crate::stdlib::graphics::draw::fill_grid_runs(
                        canvas,
                        grid.cell_width,
                        grid.cell_height,
                        &runs,
                    );
                    Ok(Value::None)
                } else {
                    Err(anyhow!("DRAW_GRID_RUNS: unknown handle '{}'", handle))
                }
            }

            "pixel" | "canvas_set_pixel" | "window_set_pixel" => {
                // pixel(canvas, x, y, r, g, b) -> none
                if args.len() != 6 {
                    return Err(anyhow!("PIXEL expects 6 args (canvas, x, y, r, g, b)"));
                }
                match (&args[0], &args[1], &args[2], &args[3], &args[4], &args[5]) {
        (Value::String(handle), Value::Number(xn), Value::Number(yn),
         Value::Number(rn), Value::Number(gn), Value::Number(bn)) => {
            let x = *xn as isize;
            let y = *yn as isize;
            let r = (*rn as i32).clamp(0, 255) as u32;
            let g = (*gn as i32).clamp(0, 255) as u32;
            let b = (*bn as i32).clamp(0, 255) as u32;
            let color = 0xFF000000 | (r << 16) | (g << 8) | b;
            let handle = handle.clone();
            if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                canvas.set_pixel(x, y, color);
                Ok(Value::None)
            } else {
                Err(anyhow!("PIXEL: unknown handle '{}'", handle))
            }
        }
        _ => Err(anyhow!("PIXEL: expected (canvas:string, x:number, y:number, r:number, g:number, b:number)")),
    }
            }

            "canvas_fill_rect" => {
                // 5-arg: (canvas, x1, y1, x2, y2) -> uses current_color
                // 8-arg: (canvas, x, y, w, h, r, g, b) -> backward compat
                match args.len() {
                    5 => match (&args[0], &args[1], &args[2], &args[3], &args[4]) {
                        (
                            Value::String(handle),
                            Value::Number(x1),
                            Value::Number(y1),
                            Value::Number(x2),
                            Value::Number(y2),
                        ) => {
                            let x = x1.min(*x2) as isize;
                            let y = y1.min(*y2) as isize;
                            let w = (x2 - x1).abs() as isize;
                            let h = (y2 - y1).abs() as isize;
                            let color = self.current_color;
                            let handle = handle.clone();
                            if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                                crate::stdlib::graphics::draw::fill_rect(canvas, x, y, w, h, color);
                                Ok(Value::None)
                            } else {
                                Err(anyhow!("canvas_fill_rect: unknown handle '{}'", handle))
                            }
                        }
                        _ => Err(anyhow!(
                            "canvas_fill_rect: expected (canvas:string, x1, y1, x2, y2)"
                        )),
                    },
                    8 => {
                        match (&args[0], &args[1], &args[2], &args[3], &args[4], &args[5], &args[6], &args[7]) {
                (Value::String(handle), Value::Number(xn), Value::Number(yn),
                 Value::Number(wn), Value::Number(hn),
                 Value::Number(rn), Value::Number(gn), Value::Number(bn)) => {
                    let x = *xn as isize;
                    let y = *yn as isize;
                    let w = (*wn as isize).max(0);
                    let h = (*hn as isize).max(0);
                    let r = (*rn as i32).clamp(0, 255) as u32;
                    let g = (*gn as i32).clamp(0, 255) as u32;
                    let b = (*bn as i32).clamp(0, 255) as u32;
                    let color = 0xFF000000 | (r << 16) | (g << 8) | b;
                    let handle = handle.clone();
                    if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                        crate::stdlib::graphics::draw::fill_rect(canvas, x, y, w, h, color);
                        Ok(Value::None)
                    } else {
                        Err(anyhow!("canvas_fill_rect: unknown handle '{}'", handle))
                    }
                }
                _ => Err(anyhow!("canvas_fill_rect: expected (canvas:string, x:number, y:number, w:number, h:number, r:number, g:number, b:number)")),
            }
                    }
                    _ => Err(anyhow!("canvas_fill_rect expects 5 or 8 args")),
                }
            }

            "canvas_draw_rect" => {
                // 5-arg: (canvas, x, y, w, h) uses current_color
                // 8-arg: (canvas, x, y, w, h, r, g, b) backward compat
                let (handle, x, y, w, h, color) = if args.len() == 5 {
                    match (&args[0], &args[1], &args[2], &args[3], &args[4]) {
                        (
                            Value::String(h),
                            Value::Number(x),
                            Value::Number(y),
                            Value::Number(w),
                            Value::Number(hh),
                        ) => (
                            h.clone(),
                            *x as isize,
                            *y as isize,
                            *w as isize,
                            *hh as isize,
                            self.current_color,
                        ),
                        _ => return Err(anyhow!("canvas_draw_rect: wrong types")),
                    }
                } else if args.len() == 8 {
                    match (
                        &args[0], &args[1], &args[2], &args[3], &args[4], &args[5], &args[6],
                        &args[7],
                    ) {
                        (
                            Value::String(h),
                            Value::Number(x),
                            Value::Number(y),
                            Value::Number(w),
                            Value::Number(hh),
                            Value::Number(r),
                            Value::Number(g),
                            Value::Number(b),
                        ) => {
                            let color = 0xFF000000
                                | ((*r as i32).clamp(0, 255) as u32) << 16
                                | ((*g as i32).clamp(0, 255) as u32) << 8
                                | (*b as i32).clamp(0, 255) as u32;
                            (
                                h.clone(),
                                *x as isize,
                                *y as isize,
                                *w as isize,
                                *hh as isize,
                                color,
                            )
                        }
                        _ => return Err(anyhow!("canvas_draw_rect: wrong types")),
                    }
                } else {
                    return Err(anyhow!("canvas_draw_rect expects 5 or 8 args"));
                };
                if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                    crate::stdlib::graphics::draw::draw_rect(canvas, x, y, w, h, color);
                    Ok(Value::None)
                } else {
                    Err(anyhow!("canvas_draw_rect: unknown handle '{}'", handle))
                }
            }

            "window_fill" | "canvas_fill" => {
                // window_fill(handle, x, y, w, h, r, g, b) -> none
                if args.len() != 8 {
                    return Err(anyhow!("window_fill expects 8 args (handle,x,y,w,h,r,g,b)"));
                }
                match (&args[0], &args[1], &args[2], &args[3], &args[4], &args[5], &args[6], &args[7]) {
        (Value::String(handle), Value::Number(xn), Value::Number(yn), Value::Number(wn), Value::Number(hn), Value::Number(rn), Value::Number(gn), Value::Number(bn)) => {
            let x = *xn as isize;
            let y = *yn as isize;
            let w = (*wn as isize).max(0);
            let h = (*hn as isize).max(0);
            let r = (*rn as i32).clamp(0, 255) as u32;
            let g = (*gn as i32).clamp(0, 255) as u32;
            let b = (*bn as i32).clamp(0, 255) as u32;
            let color = 0xFF000000 | (r << 16) | (g << 8) | b;
            let handle = handle.clone();
            if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                crate::stdlib::graphics::draw::fill_rect(canvas, x, y, w, h, color);
                Ok(Value::None)
            } else {
                Err(anyhow!("window_fill: unknown handle"))
            }
        }
        _ => Err(anyhow!("window_fill: expected (handle:string,x:number,y:number,w:number,h:number,r:number,g:number,b:number)")),
    }
            }

            "canvas_clear" => {
                // canvas_clear(canvas) -> uses current_color
                // canvas_clear(canvas, r, g, b) -> explicit color (backward compat)
                let handle = match args.first() {
                    Some(Value::String(s)) => s.clone(),
                    _ => {
                        return Err(anyhow!(
                            "canvas_clear: first argument must be a canvas handle"
                        ))
                    }
                };
                let color = if args.len() == 4 {
                    match (&args[1], &args[2], &args[3]) {
                        (Value::Number(r), Value::Number(g), Value::Number(b)) => {
                            let r = (*r as i32).clamp(0, 255) as u32;
                            let g = (*g as i32).clamp(0, 255) as u32;
                            let b = (*b as i32).clamp(0, 255) as u32;
                            0xFF000000 | (r << 16) | (g << 8) | b
                        }
                        _ => return Err(anyhow!("canvas_clear: expected (canvas, r, g, b)")),
                    }
                } else if args.len() == 1 {
                    self.current_color
                } else {
                    return Err(anyhow!("canvas_clear expects 1 or 4 args"));
                };
                if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                    canvas.clear(color);
                    Ok(Value::None)
                } else {
                    Err(anyhow!("canvas_clear: unknown handle '{}'", handle))
                }
            }

            "canvas_get_pixel" => {
                if args.len() != 3 {
                    return Err(anyhow!("canvas_get_pixel expects 3 args (canvas, x, y)"));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(handle), Value::Number(xn), Value::Number(yn)) => {
                        let x = *xn as isize;
                        let y = *yn as isize;
                        if let Some(canvas) = self.canvases.get(handle) {
                            Ok(Value::Number(canvas.get_pixel(x, y) as f64))
                        } else {
                            Ok(Value::Number(0.0))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_get_pixel: expected (canvas:string, x:number, y:number)"
                    )),
                }
            }

            "canvas_width" => {
                if args.len() != 1 {
                    return Err(anyhow!("canvas_width expects 1 arg (canvas)"));
                }
                match &args[0] {
                    Value::String(handle) => {
                        if let Some(canvas) = self.canvases.get(handle) {
                            Ok(Value::Number(canvas.width as f64))
                        } else if let Some((w, _, _)) = self.gfx_handles.get(handle) {
                            Ok(Value::Number(*w as f64))
                        } else {
                            Ok(Value::Number(0.0))
                        }
                    }
                    _ => Err(anyhow!("canvas_width: expected (canvas:string)")),
                }
            }

            "canvas_height" => {
                if args.len() != 1 {
                    return Err(anyhow!("canvas_height expects 1 arg (canvas)"));
                }
                match &args[0] {
                    Value::String(handle) => {
                        if let Some(canvas) = self.canvases.get(handle) {
                            Ok(Value::Number(canvas.height as f64))
                        } else if let Some((_, h, _)) = self.gfx_handles.get(handle) {
                            Ok(Value::Number(*h as f64))
                        } else {
                            Ok(Value::Number(0.0))
                        }
                    }
                    _ => Err(anyhow!("canvas_height: expected (canvas:string)")),
                }
            }

            "canvas_save_ppm" | "window_save" => {
                if args.len() != 2 {
                    return Err(anyhow!("canvas_save_ppm expects 2 args (canvas, path)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(handle), Value::String(path)) => {
                        if let Some(canvas) = self.canvases.get(handle) {
                            canvas
                                .save_ppm(path)
                                .map_err(|e| anyhow!("canvas_save_ppm: {}", e))?;
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_save_ppm: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_save_ppm: expected (canvas:string, path:string)"
                    )),
                }
            }

            "canvas_present" => {
                // canvas_present(window) -> none; presents window's own canvas to X11
                if args.len() != 1 {
                    return Err(anyhow!("canvas_present expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        #[cfg(feature = "x11")]
                        {
                            let canvas_clone = self.canvases.get(win_h).cloned();
                            if let Some(canvas) = canvas_clone {
                                if let Some(xwin) = self.x11_windows.get_mut(win_h) {
                                    match xwin.present(&canvas) {
                                        Ok(()) => {}
                                        Err(e) => {
                                            eprintln!("[pasta/gfx] canvas_present error: {}", e);
                                        }
                                    }
                                    let open = xwin.poll();
                                    if let Some(entry) = self.gfx_handles.get_mut(win_h) {
                                        entry.2 = open;
                                    }
                                }
                            }
                        }
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("canvas_present: expected (window:string)")),
                }
            }

            "blit" | "present" => {
                // BLIT(window, canvas) or PRESENT(window, canvas) -> none
                if args.len() != 2 {
                    return Err(anyhow!("BLIT expects 2 args (window, canvas)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(win_h), Value::String(canvas_h)) => {
                        #[cfg(feature = "x11")]
                        {
                            let canvas_clone = self.canvases.get(canvas_h).cloned();
                            if let Some(canvas) = canvas_clone {
                                if let Some(xwin) = self.x11_windows.get_mut(win_h) {
                                    match xwin.present(&canvas) {
                                        Ok(()) => {}
                                        Err(e) => {
                                            eprintln!("[pasta/gfx] BLIT error: {}", e);
                                        }
                                    }
                                    let open = xwin.poll();
                                    if let Some(entry) = self.gfx_handles.get_mut(win_h) {
                                        entry.2 = open;
                                    }
                                    if !open {
                                        return Ok(Value::None);
                                    }
                                }
                            }
                        }
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("BLIT: expected (window:string, canvas:string)")),
                }
            }

            "swap_buffer" => {
                // SWAP_BUFFER(window) or SWAP_BUFFER(window, src_x, src_y)
                if args.is_empty() {
                    return Err(anyhow!("SWAP_BUFFER expects at least 1 arg"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        let win_h = win_h.clone();
                        let src_x = if args.len() >= 3 {
                            if let Value::Number(n) = &args[1] {
                                *n as usize
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                        let src_y = if args.len() >= 3 {
                            if let Value::Number(n) = &args[2] {
                                *n as usize
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                        let (win_w, win_h_dim) = match self.gfx_handles.get(&win_h) {
                            Some((w, h, _)) => (*w, *h),
                            None => return Err(anyhow!("SWAP_BUFFER: unknown window handle")),
                        };
                        #[cfg(feature = "x11")]
                        {
                            if src_x == 0 && src_y == 0 {
                                if let Some(back) = self.back_buffers.get(&win_h) {
                                    if back.width == win_w && back.height == win_h_dim {
                                        if let Some(xwin) = self.x11_windows.get_mut(&win_h) {
                                            let _ = xwin.present(back);
                                            let open = xwin.poll();
                                            if let Some(entry) = self.gfx_handles.get_mut(&win_h) {
                                                entry.2 = open;
                                            }
                                            return Ok(Value::None);
                                        }
                                    }
                                }
                            }
                        }

                        // Fallback path for cropped copies, size mismatches, or headless mode.
                        let (canvases, back_buffers) = (&mut self.canvases, &self.back_buffers);
                        if let (Some(front), Some(back)) =
                            (canvases.get_mut(&win_h), back_buffers.get(&win_h))
                        {
                            front.copy_region_from(back, src_x, src_y, win_w, win_h_dim, 0, 0);
                        }

                        #[cfg(feature = "x11")]
                        {
                            if let Some(canvas) = self.canvases.get(&win_h) {
                                if let Some(xwin) = self.x11_windows.get_mut(&win_h) {
                                    let _ = xwin.present(canvas);
                                    let open = xwin.poll();
                                    if let Some(entry) = self.gfx_handles.get_mut(&win_h) {
                                        entry.2 = open;
                                    }
                                }
                            }
                        }
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("SWAP_BUFFER: expected (window:string)")),
                }
            }

            "clear_buffer" => {
                if args.len() != 1 {
                    return Err(anyhow!("CLEAR_BUFFER expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        let color = self.current_color;
                        if let Some(back) = self.back_buffers.get_mut(win_h) {
                            back.clear(color);
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("CLEAR_BUFFER: unknown window handle '{}'", win_h))
                        }
                    }
                    _ => Err(anyhow!("CLEAR_BUFFER: expected (window:string)")),
                }
            }

            "draw_to_buffer" => {
                if args.len() != 2 {
                    return Err(anyhow!("DRAW_TO_BUFFER expects 2 args (window, canvas)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(win_h), Value::String(canvas_h)) => {
                        let canvas_clone = self.canvases.get(canvas_h).cloned();
                        if let Some(src) = canvas_clone {
                            if let Some(back) = self.back_buffers.get_mut(win_h) {
                                let copy_w = src.width.min(back.width);
                                let copy_h = src.height.min(back.height);
                                for y in 0..copy_h {
                                    for x in 0..copy_w {
                                        back.pixels[y * back.width + x] =
                                            src.pixels[y * src.width + x];
                                    }
                                }
                                Ok(Value::None)
                            } else {
                                Err(anyhow!("DRAW_TO_BUFFER: unknown window handle '{}'", win_h))
                            }
                        } else {
                            Err(anyhow!(
                                "DRAW_TO_BUFFER: unknown canvas handle '{}'",
                                canvas_h
                            ))
                        }
                    }
                    _ => Err(anyhow!(
                        "DRAW_TO_BUFFER: expected (window:string, canvas:string)"
                    )),
                }
            }

            "set_draw_target" => {
                if args.len() != 1 {
                    return Err(anyhow!("SET_DRAW_TARGET expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        self.draw_target = Some(win_h.clone());
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("SET_DRAW_TARGET: expected (window:string)")),
                }
            }

            "copy_to_buffer" => {
                // COPY_TO_BUFFER(canvas, x1, y1, x2, y2) - copies canvas region to draw_target's back_buffer
                if args.len() != 5 {
                    return Err(anyhow!(
                        "COPY_TO_BUFFER expects 5 args (canvas, x1, y1, x2, y2)"
                    ));
                }
                let target = match &self.draw_target {
                    Some(t) => t.clone(),
                    None => return Err(anyhow!("COPY_TO_BUFFER: no draw target set")),
                };
                match (&args[0], &args[1], &args[2], &args[3], &args[4]) {
                    (
                        Value::String(canvas_h),
                        Value::Number(x1),
                        Value::Number(y1),
                        Value::Number(x2),
                        Value::Number(y2),
                    ) => {
                        let sx = x1.min(*x2) as usize;
                        let sy = y1.min(*y2) as usize;
                        let ex = x1.max(*x2) as usize;
                        let ey = y1.max(*y2) as usize;
                        let canvas_clone = self.canvases.get(canvas_h).cloned();
                        if let Some(src) = canvas_clone {
                            if let Some(back) = self.back_buffers.get_mut(&target) {
                                for y in sy..ey.min(src.height).min(back.height) {
                                    for x in sx..ex.min(src.width).min(back.width) {
                                        back.pixels[y * back.width + x] =
                                            src.pixels[y * src.width + x];
                                    }
                                }
                                Ok(Value::None)
                            } else {
                                Err(anyhow!(
                                    "COPY_TO_BUFFER: no back buffer for target '{}'",
                                    target
                                ))
                            }
                        } else {
                            Err(anyhow!(
                                "COPY_TO_BUFFER: unknown canvas handle '{}'",
                                canvas_h
                            ))
                        }
                    }
                    _ => Err(anyhow!(
                        "COPY_TO_BUFFER: expected (canvas:string, x1, y1, x2, y2)"
                    )),
                }
            }

            "pop_buffer" => {
                // POP_BUFFER(window, sx1,sy1,sx2,sy2, dx1,dy1,dx2,dy2)
                if args.len() != 9 {
                    return Err(anyhow!("POP_BUFFER expects 9 args"));
                }
                let win_h = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("POP_BUFFER: first arg must be window handle")),
                };
                let sx1 = match &args[1] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let sy1 = match &args[2] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let sx2 = match &args[3] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let sy2 = match &args[4] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let dx1 = match &args[5] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let dy1 = match &args[6] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let _dx2 = match &args[7] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let _dy2 = match &args[8] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POP_BUFFER: numbers required")),
                };
                let rw = sx2.saturating_sub(sx1);
                let rh = sy2.saturating_sub(sy1);
                let (canvases, back_buffers) = (&mut self.canvases, &self.back_buffers);
                if let (Some(front), Some(back)) =
                    (canvases.get_mut(&win_h), back_buffers.get(&win_h))
                {
                    front.copy_region_from(back, sx1, sy1, rw, rh, dx1, dy1);
                    Ok(Value::None)
                } else {
                    Err(anyhow!("POP_BUFFER: unknown window handle '{}'", win_h))
                }
            }

            "resize_buffer" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "RESIZE_BUFFER expects 3 args (window, new_w, new_h)"
                    ));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(win_h), Value::Number(w), Value::Number(h)) => {
                        let new_w = *w as usize;
                        let new_h = *h as usize;
                        self.back_buffers.insert(
                            win_h.clone(),
                            crate::stdlib::graphics::canvas::Canvas::new(new_w, new_h),
                        );
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!(
                        "RESIZE_BUFFER: expected (window:string, new_w:number, new_h:number)"
                    )),
                }
            }

            "blend_to_buffer" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "BLEND_TO_BUFFER expects 3 args (window, canvas, alpha)"
                    ));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(win_h), Value::String(canvas_h), Value::Number(alpha)) => {
                        let alpha = alpha.clamp(0.0, 1.0) as f32;
                        let canvas_clone = self.canvases.get(canvas_h).cloned();
                        if let Some(src) = canvas_clone {
                            if let Some(back) = self.back_buffers.get_mut(win_h) {
                                let len = src.pixels.len().min(back.pixels.len());
                                for i in 0..len {
                                    let sp = src.pixels[i];
                                    let dp = back.pixels[i];
                                    let sr = ((sp >> 16) & 0xFF) as f32;
                                    let sg = ((sp >> 8) & 0xFF) as f32;
                                    let sb = (sp & 0xFF) as f32;
                                    let dr = ((dp >> 16) & 0xFF) as f32;
                                    let dg = ((dp >> 8) & 0xFF) as f32;
                                    let db = (dp & 0xFF) as f32;
                                    let r = (sr * alpha + dr * (1.0 - alpha)).round() as u32;
                                    let g = (sg * alpha + dg * (1.0 - alpha)).round() as u32;
                                    let b = (sb * alpha + db * (1.0 - alpha)).round() as u32;
                                    back.pixels[i] = 0xFF000000 | (r << 16) | (g << 8) | b;
                                }
                                Ok(Value::None)
                            } else {
                                Err(anyhow!(
                                    "BLEND_TO_BUFFER: unknown window handle '{}'",
                                    win_h
                                ))
                            }
                        } else {
                            Err(anyhow!(
                                "BLEND_TO_BUFFER: unknown canvas handle '{}'",
                                canvas_h
                            ))
                        }
                    }
                    _ => Err(anyhow!(
                        "BLEND_TO_BUFFER: expected (window:string, canvas:string, alpha:number)"
                    )),
                }
            }

            "scroll_buffer" => {
                if args.len() != 3 {
                    return Err(anyhow!("SCROLL_BUFFER expects 3 args (window, dx, dy)"));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::String(win_h), Value::Number(dxn), Value::Number(dyn_)) => {
                        let dx = *dxn as isize;
                        let dy = *dyn_ as isize;
                        let color = self.current_color;
                        if let Some(back) = self.back_buffers.get_mut(win_h) {
                            let w = back.width as isize;
                            let h = back.height as isize;
                            let old_pixels = back.pixels.clone();
                            back.pixels.fill(color);
                            for y in 0..h {
                                for x in 0..w {
                                    let sx = x - dx;
                                    let sy = y - dy;
                                    if sx >= 0 && sy >= 0 && sx < w && sy < h {
                                        back.pixels[(y * w + x) as usize] =
                                            old_pixels[(sy * w + sx) as usize];
                                    }
                                }
                            }
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("SCROLL_BUFFER: unknown window handle '{}'", win_h))
                        }
                    }
                    _ => Err(anyhow!(
                        "SCROLL_BUFFER: expected (window:string, dx:number, dy:number)"
                    )),
                }
            }

            "tint_buffer" => {
                if args.len() != 5 {
                    return Err(anyhow!(
                        "TINT_BUFFER expects 5 args (window, r, g, b, alpha)"
                    ));
                }
                match (&args[0], &args[1], &args[2], &args[3], &args[4]) {
                    (
                        Value::String(win_h),
                        Value::Number(rn),
                        Value::Number(gn),
                        Value::Number(bn),
                        Value::Number(an),
                    ) => {
                        let tr = *rn as f32;
                        let tg = *gn as f32;
                        let tb = *bn as f32;
                        let alpha = an.clamp(0.0, 1.0) as f32;
                        if let Some(back) = self.back_buffers.get_mut(win_h) {
                            for px in back.pixels.iter_mut() {
                                let br = (((*px >> 16) & 0xFF) as f32 * tr / 255.0) * alpha
                                    + ((*px >> 16) & 0xFF) as f32 * (1.0 - alpha);
                                let bg = (((*px >> 8) & 0xFF) as f32 * tg / 255.0) * alpha
                                    + ((*px >> 8) & 0xFF) as f32 * (1.0 - alpha);
                                let bb = ((*px & 0xFF) as f32 * tb / 255.0) * alpha
                                    + (*px & 0xFF) as f32 * (1.0 - alpha);
                                *px = 0xFF000000
                                    | ((br.round() as u32) << 16)
                                    | ((bg.round() as u32) << 8)
                                    | bb.round() as u32;
                            }
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("TINT_BUFFER: unknown window handle '{}'", win_h))
                        }
                    }
                    _ => Err(anyhow!(
                        "TINT_BUFFER: expected (window:string, r, g, b, alpha)"
                    )),
                }
            }

            "canvas_draw_line" => {
                // canvas_draw_line(canvas, x0, y0, x1, y1) uses current_color
                // canvas_draw_line(canvas, x0, y0, x1, y1, color) explicit color
                if args.len() < 5 {
                    return Err(anyhow!("canvas_draw_line expects at least 5 args"));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let x0 = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("canvas_draw_line: x0 must be number")),
                        };
                        let y0 = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("canvas_draw_line: y0 must be number")),
                        };
                        let x1 = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("canvas_draw_line: x1 must be number")),
                        };
                        let y1 = match &args[4] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("canvas_draw_line: y1 must be number")),
                        };
                        let color = if args.len() >= 6 {
                            match &args[5] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::draw_line(canvas, x0, y0, x1, y1, color);
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_draw_line: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!("canvas_draw_line: first arg must be canvas handle")),
                }
            }

            "canvas_draw_circle" => {
                if args.len() < 4 {
                    return Err(anyhow!(
                        "canvas_draw_circle expects at least 4 args (canvas, cx, cy, r)"
                    ));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let cx = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cx must be number")),
                        };
                        let cy = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cy must be number")),
                        };
                        let r = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("r must be number")),
                        };
                        let color = if args.len() >= 5 {
                            match &args[4] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::draw_circle(canvas, cx, cy, r, color);
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_draw_circle: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_draw_circle: first arg must be canvas handle"
                    )),
                }
            }

            "canvas_fill_circle" => {
                if args.len() < 4 {
                    return Err(anyhow!(
                        "canvas_fill_circle expects at least 4 args (canvas, cx, cy, r)"
                    ));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let cx = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cx must be number")),
                        };
                        let cy = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cy must be number")),
                        };
                        let r = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("r must be number")),
                        };
                        let color = if args.len() >= 5 {
                            match &args[4] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::fill_circle(canvas, cx, cy, r, color);
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_fill_circle: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_fill_circle: first arg must be canvas handle"
                    )),
                }
            }

            "canvas_draw_ellipse" => {
                if args.len() < 5 {
                    return Err(anyhow!(
                        "canvas_draw_ellipse expects at least 5 args (canvas, cx, cy, rx, ry)"
                    ));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let cx = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cx")),
                        };
                        let cy = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cy")),
                        };
                        let rx = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("rx")),
                        };
                        let ry = match &args[4] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("ry")),
                        };
                        let color = if args.len() >= 6 {
                            match &args[5] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::draw_ellipse(
                                canvas, cx, cy, rx, ry, color,
                            );
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_draw_ellipse: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_draw_ellipse: first arg must be canvas handle"
                    )),
                }
            }

            "canvas_fill_ellipse" => {
                if args.len() < 5 {
                    return Err(anyhow!(
                        "canvas_fill_ellipse expects at least 5 args (canvas, cx, cy, rx, ry)"
                    ));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let cx = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cx")),
                        };
                        let cy = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cy")),
                        };
                        let rx = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("rx")),
                        };
                        let ry = match &args[4] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("ry")),
                        };
                        let color = if args.len() >= 6 {
                            match &args[5] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::fill_ellipse(
                                canvas, cx, cy, rx, ry, color,
                            );
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_fill_ellipse: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_fill_ellipse: first arg must be canvas handle"
                    )),
                }
            }

            "canvas_draw_triangle" => {
                if args.len() < 7 {
                    return Err(anyhow!(
                        "canvas_draw_triangle expects at least 7 args (canvas, x0,y0,x1,y1,x2,y2)"
                    ));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let x0 = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("x0")),
                        };
                        let y0 = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("y0")),
                        };
                        let x1 = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("x1")),
                        };
                        let y1 = match &args[4] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("y1")),
                        };
                        let x2 = match &args[5] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("x2")),
                        };
                        let y2 = match &args[6] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("y2")),
                        };
                        let color = if args.len() >= 8 {
                            match &args[7] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::draw_triangle(
                                canvas, x0, y0, x1, y1, x2, y2, color,
                            );
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_draw_triangle: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_draw_triangle: first arg must be canvas handle"
                    )),
                }
            }

            "canvas_fill_triangle" => {
                if args.len() < 7 {
                    return Err(anyhow!(
                        "canvas_fill_triangle expects at least 7 args (canvas, x0,y0,x1,y1,x2,y2)"
                    ));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let x0 = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("x0")),
                        };
                        let y0 = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("y0")),
                        };
                        let x1 = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("x1")),
                        };
                        let y1 = match &args[4] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("y1")),
                        };
                        let x2 = match &args[5] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("x2")),
                        };
                        let y2 = match &args[6] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("y2")),
                        };
                        let color = if args.len() >= 8 {
                            match &args[7] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::fill_triangle(
                                canvas, x0, y0, x1, y1, x2, y2, color,
                            );
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_fill_triangle: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!(
                        "canvas_fill_triangle: first arg must be canvas handle"
                    )),
                }
            }

            "canvas_draw_arc" => {
                if args.len() < 6 {
                    return Err(anyhow!("canvas_draw_arc expects at least 6 args (canvas, cx, cy, r, start_deg, end_deg)"));
                }
                match &args[0] {
                    Value::String(handle) => {
                        let cx = match &args[1] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cx")),
                        };
                        let cy = match &args[2] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("cy")),
                        };
                        let r = match &args[3] {
                            Value::Number(n) => *n as isize,
                            _ => return Err(anyhow!("r")),
                        };
                        let s = match &args[4] {
                            Value::Number(n) => *n,
                            _ => return Err(anyhow!("start_deg")),
                        };
                        let e = match &args[5] {
                            Value::Number(n) => *n,
                            _ => return Err(anyhow!("end_deg")),
                        };
                        let color = if args.len() >= 7 {
                            match &args[6] {
                                Value::Number(n) => *n as u32,
                                _ => self.current_color,
                            }
                        } else {
                            self.current_color
                        };
                        let handle = handle.clone();
                        if let Some(canvas) = self.get_canvas_for_draw(&handle) {
                            crate::stdlib::graphics::draw::draw_arc(canvas, cx, cy, r, s, e, color);
                            Ok(Value::None)
                        } else {
                            Err(anyhow!("canvas_draw_arc: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!("canvas_draw_arc: first arg must be canvas handle")),
                }
            }

            "gfx_memory_usage" => {
                if args.len() != 1 {
                    return Err(anyhow!("gfx_memory_usage expects 1 arg (handle)"));
                }
                match &args[0] {
                    Value::String(handle) => {
                        if let Some(canvas) = self.canvases.get(handle) {
                            Ok(Value::Number((canvas.width * canvas.height * 4) as f64))
                        } else {
                            Err(anyhow!("gfx_memory_usage: unknown handle '{}'", handle))
                        }
                    }
                    _ => Err(anyhow!("gfx_memory_usage: expected (handle:string)")),
                }
            }

            "graphics_cleanup" => {
                if !args.is_empty() {
                    return Err(anyhow!("GRAPHICS_CLEANUP takes no arguments"));
                }
                self.cleanup_graphics();
                Ok(Value::None)
            }

            "window_poll" => {
                if args.len() != 1 {
                    return Err(anyhow!("WINDOW_POLL expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        #[cfg(feature = "x11")]
                        {
                            if let Some(xwin) = self.x11_windows.get_mut(win_h) {
                                let open = xwin.poll();
                                if let Some(entry) = self.gfx_handles.get_mut(win_h) {
                                    entry.2 = open;
                                }
                                return Ok(Value::Bool(open));
                            }
                        }
                        if let Some((_, _, is_open)) = self.gfx_handles.get(win_h) {
                            Ok(Value::Bool(*is_open))
                        } else {
                            Err(anyhow!("WINDOW_POLL: unknown window handle '{}'", win_h))
                        }
                    }
                    _ => Err(anyhow!("WINDOW_POLL: expected (window:string)")),
                }
            }

            "window_is_open" => {
                if args.len() != 1 {
                    return Err(anyhow!("WINDOW_IS_OPEN expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        if let Some((_, _, is_open)) = self.gfx_handles.get(win_h) {
                            Ok(Value::Bool(*is_open))
                        } else {
                            Err(anyhow!("WINDOW_IS_OPEN: unknown window handle '{}'", win_h))
                        }
                    }
                    _ => Err(anyhow!("WINDOW_IS_OPEN: expected (window:string)")),
                }
            }

            "window_close" => {
                if args.len() != 1 {
                    return Err(anyhow!("WINDOW_CLOSE expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        #[cfg(feature = "x11")]
                        {
                            if let Some(mut xwin) = self.x11_windows.remove(win_h) {
                                xwin.close();
                            }
                        }
                        if let Some(entry) = self.gfx_handles.get_mut(win_h) {
                            entry.2 = false;
                        }
                        self.canvases.remove(win_h);
                        self.back_buffers.remove(win_h);
                        self.grid_configs.remove(win_h);
                        if self.draw_target.as_deref() == Some(win_h) {
                            self.draw_target = None;
                        }
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("WINDOW_CLOSE: expected (window:string)")),
                }
            }

            "window_key" => {
                if args.len() != 1 {
                    return Err(anyhow!("WINDOW_KEY expects 1 arg (window)"));
                }
                match &args[0] {
                    Value::String(win_h) => {
                        #[cfg(feature = "x11")]
                        {
                            if let Some(xwin) = self.x11_windows.get_mut(win_h) {
                                let _open = xwin.poll();
                                let key = xwin.latest_key();
                                return Ok(Value::String(key));
                            }
                        }
                        Ok(Value::String(String::new()))
                    }
                    _ => Err(anyhow!("WINDOW_KEY: expected (window:string)")),
                }
            }

            "window_open" => {
                if args.len() != 1 {
                    return Err(anyhow!("WINDOW_OPEN expects 1 arg (handle)"));
                }
                match &args[0] {
                    Value::String(h) => {
                        #[cfg(feature = "x11")]
                        {
                            if let Some(xwin) = self.x11_windows.get_mut(h) {
                                let open = xwin.poll();
                                if let Some(entry) = self.gfx_handles.get_mut(h) {
                                    entry.2 = open;
                                }
                            }
                        }
                        let open = self.gfx_handles.get(h).map(|e| e.2).unwrap_or(false);
                        Ok(Value::Bool(open))
                    }
                    _ => Err(anyhow!("WINDOW_OPEN: expected string handle")),
                }
            }

            "close" => {
                if args.len() != 1 {
                    return Err(anyhow!("CLOSE expects 1 arg (handle)"));
                }
                match &args[0] {
                    Value::String(h) => {
                        if self.open_files.contains_key(h) {
                            use std::io::Write as _;
                            match self.open_files.remove(h) {
                                Some(OpenFile::Writer(mut w)) => {
                                    w.flush().map_err(|e| anyhow!("CLOSE flush: {}", e))?;
                                }
                                Some(OpenFile::Appender(mut w)) => {
                                    w.flush().map_err(|e| anyhow!("CLOSE flush: {}", e))?;
                                }
                                Some(OpenFile::Reader(_)) => {}
                                None => {}
                            }
                            return Ok(Value::None);
                        }

                        #[cfg(feature = "x11")]
                        {
                            if let Some(mut xwin) = self.x11_windows.remove(h) {
                                xwin.close();
                            }
                        }
                        if let Some(entry) = self.gfx_handles.get_mut(h) {
                            entry.2 = false;
                        }
                        self.canvases.remove(h);
                        self.back_buffers.remove(h);
                        self.grid_configs.remove(h);
                        if self.draw_target.as_deref() == Some(h) {
                            self.draw_target = None;
                        }
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("CLOSE: expected string handle")),
                }
            }

            // ── CompactCanvas legacy stubs (kept as no-ops for backward compat) ────────
            "gfx_set_base_color"
            | "gfx_get_base_color"
            | "gfx_set_palette"
            | "gfx_init_grayscale_palette" => {
                // These were CompactCanvas-specific; now no-ops
                Ok(Value::None)
            }

            // ── FPS System ───────────────────────────────────────────────────────────────
            // fps_init(target_fps) — initialise (or reinitialise) the FPS state
            "fps_init" => {
                let target = match args.first() {
                    Some(Value::Number(n)) => *n,
                    _ => 60.0,
                };
                self.fps_state = Some(FpsState::new(target));
                Ok(Value::None)
            }

            // fps_begin(target_fps) — start of a frame; auto-inits if needed
            "fps_begin" => {
                let target = match args.first() {
                    Some(Value::Number(n)) => *n,
                    _ => 60.0,
                };
                if self.fps_state.is_none() {
                    self.fps_state = Some(FpsState::new(target));
                } else if let Some(ref mut s) = self.fps_state {
                    s.target_fps = target;
                    s.fixed_delta = 1.0 / target;
                }
                if let Some(ref mut s) = self.fps_state {
                    s.frame_start = std::time::Instant::now();
                }
                Ok(Value::None)
            }

            // fps_end() — sleep remaining frame time and record frame duration
            "fps_end" => {
                if let Some(ref mut s) = self.fps_state {
                    if !s.paused {
                        let work_elapsed = s.frame_start.elapsed();
                        // Record the work time (pre-sleep) for fps_behind / fps_frame_time
                        s.last_frame_time_ms = work_elapsed.as_secs_f64() * 1000.0;
                        let target_dur = std::time::Duration::from_secs_f64(s.fixed_delta);
                        if work_elapsed < target_dur {
                            std::thread::sleep(target_dur - work_elapsed);
                        }
                        // Record total wall time (post-sleep) for fps_get / fps_avg
                        let total = s.frame_start.elapsed();
                        let secs = total.as_secs_f64();
                        s.frame_times.push_back(secs);
                        if s.frame_times.len() > 256 {
                            s.frame_times.pop_front();
                        }
                    }
                }
                Ok(Value::None)
            }

            // fps_tick() — advance frame counter, return fixed delta
            "fps_tick" => {
                if let Some(ref mut s) = self.fps_state {
                    if !s.paused {
                        s.frame_count += 1;
                    }
                    Ok(Value::Number(s.fixed_delta))
                } else {
                    Ok(Value::Number(1.0 / 60.0))
                }
            }

            // fps_sleep() — sleep remaining frame time without recording
            "fps_sleep" => {
                if let Some(ref s) = self.fps_state {
                    if !s.paused {
                        let elapsed = s.frame_start.elapsed();
                        let target_dur = std::time::Duration::from_secs_f64(s.fixed_delta);
                        if elapsed < target_dur {
                            std::thread::sleep(target_dur - elapsed);
                        }
                    }
                }
                Ok(Value::None)
            }

            // fps_delta() — fixed timestep (1.0 / target_fps)
            "fps_delta" => {
                let d = self
                    .fps_state
                    .as_ref()
                    .map(|s| s.fixed_delta)
                    .unwrap_or(1.0 / 60.0);
                Ok(Value::Number(d))
            }

            // fps_get() — actual measured FPS from last frame
            "fps_get" => {
                let fps = self
                    .fps_state
                    .as_ref()
                    .map(|s| {
                        if s.last_frame_time_ms > 0.0 {
                            1000.0 / s.last_frame_time_ms
                        } else {
                            s.target_fps
                        }
                    })
                    .unwrap_or(0.0);
                Ok(Value::Number(fps))
            }

            // fps_target() — current target FPS
            "fps_target" => {
                let t = self
                    .fps_state
                    .as_ref()
                    .map(|s| s.target_fps)
                    .unwrap_or(60.0);
                Ok(Value::Number(t))
            }

            // fps_set_target(n) — change target FPS at runtime
            "fps_set_target" => {
                let target = match args.first() {
                    Some(Value::Number(n)) if *n > 0.0 => *n,
                    _ => return Err(anyhow!("fps_set_target expects a positive number")),
                };
                if let Some(ref mut s) = self.fps_state {
                    s.target_fps = target;
                    s.fixed_delta = 1.0 / target;
                } else {
                    self.fps_state = Some(FpsState::new(target));
                }
                Ok(Value::None)
            }

            // fps_frame_count() — total frames since fps_init
            "fps_frame_count" => {
                let n = self.fps_state.as_ref().map(|s| s.frame_count).unwrap_or(0);
                Ok(Value::Number(n as f64))
            }

            // fps_avg(n) — rolling average FPS over last n frames
            "fps_avg" => {
                let n = match args.first() {
                    Some(Value::Number(v)) => (*v as usize).max(1),
                    _ => 10,
                };
                if let Some(ref s) = self.fps_state {
                    let len = s.frame_times.len();
                    if len == 0 {
                        return Ok(Value::Number(s.target_fps));
                    }
                    let take = n.min(len);
                    let avg_secs: f64 =
                        s.frame_times.iter().rev().take(take).sum::<f64>() / take as f64;
                    Ok(Value::Number(if avg_secs > 0.0 {
                        1.0 / avg_secs
                    } else {
                        s.target_fps
                    }))
                } else {
                    Ok(Value::Number(0.0))
                }
            }

            // fps_paused() — true if clock is paused
            "fps_paused" => {
                let p = self.fps_state.as_ref().map(|s| s.paused).unwrap_or(false);
                Ok(Value::Bool(p))
            }

            // fps_pause() — pause the fps clock
            "fps_pause" => {
                if let Some(ref mut s) = self.fps_state {
                    if !s.paused {
                        s.paused = true;
                        s.pause_start = Some(std::time::Instant::now());
                    }
                }
                Ok(Value::None)
            }

            // fps_resume() — resume the fps clock
            "fps_resume" => {
                if let Some(ref mut s) = self.fps_state {
                    if s.paused {
                        if let Some(ps) = s.pause_start.take() {
                            s.paused_duration += ps.elapsed();
                        }
                        s.paused = false;
                    }
                }
                Ok(Value::None)
            }

            // fps_elapsed() — real seconds since fps_init (excluding paused time)
            "fps_elapsed" => {
                if let Some(ref s) = self.fps_state {
                    let total = s.init_time.elapsed();
                    let paused = if s.paused {
                        s.paused_duration + s.pause_start.map(|ps| ps.elapsed()).unwrap_or_default()
                    } else {
                        s.paused_duration
                    };
                    let active = total.saturating_sub(paused);
                    Ok(Value::Number(active.as_secs_f64()))
                } else {
                    Ok(Value::Number(0.0))
                }
            }

            // fps_frame_time() — milliseconds the last frame took
            "fps_frame_time" => {
                let ms = self
                    .fps_state
                    .as_ref()
                    .map(|s| s.last_frame_time_ms)
                    .unwrap_or(0.0);
                Ok(Value::Number(ms))
            }

            // fps_behind() — true if last frame exceeded the target budget
            "fps_behind" => {
                if let Some(ref s) = self.fps_state {
                    let budget_ms = 1000.0 / s.target_fps;
                    Ok(Value::Bool(s.last_frame_time_ms > budget_ms))
                } else {
                    Ok(Value::Bool(false))
                }
            }

            "__pasta_stdin_readline" | "stdin_readline" => {
                let mut buf = String::new();
                let n = io::stdin().read_line(&mut buf)?;
                if n == 0 {
                    return Ok(Value::None);
                }
                while buf.ends_with('\n') || buf.ends_with('\r') {
                    buf.pop();
                }
                Ok(Value::String(buf))
            }

            // ── sys.* namespace ───────────────────────────────────────────────
            "__pasta_sys_env" | "sys.env" => {
                if args.len() != 1 {
                    return Err(anyhow!("sys.env expects 1 arg (name)"));
                }
                match &args[0] {
                    Value::String(k) => Ok(Value::String(std::env::var(k).unwrap_or_default())),
                    _ => Err(anyhow!("sys.env: argument must be a string")),
                }
            }
            "sys.exit" => {
                let code = match args.first() {
                    Some(Value::Number(n)) => *n as i32,
                    _ => 0,
                };
                std::process::exit(code);
            }
            "sys.args" => {
                let argv: Vec<Value> = std::env::args().map(Value::String).collect();
                Ok(Value::List(argv))
            }
            "sys.platform" => {
                let p = if cfg!(target_os = "windows") {
                    "windows"
                } else if cfg!(target_os = "macos") {
                    "macos"
                } else {
                    "linux"
                };
                Ok(Value::String(p.to_string()))
            }
            "sys.sleep" => {
                if args.len() != 1 {
                    return Err(anyhow!("sys.sleep expects 1 arg (ms)"));
                }
                match &args[0] {
                    Value::Number(ms) => {
                        std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("sys.sleep: argument must be a number (ms)")),
                }
            }
            "sys.getcwd" => {
                let cwd = std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| ".".to_string());
                Ok(Value::String(cwd))
            }

            // ── time.* namespace ──────────────────────────────────────────────
            "__pasta_time_now_ms" | "time.now" => {
                let ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64;
                Ok(Value::Number(ms))
            }
            "time.now_ns" => {
                let ns = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos() as f64;
                Ok(Value::Number(ns))
            }
            "time.sleep" => {
                if args.len() != 1 {
                    return Err(anyhow!("time.sleep expects 1 arg (ms)"));
                }
                match &args[0] {
                    Value::Number(ms) => {
                        std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("time.sleep: argument must be a number (ms)")),
                }
            }
            "time.format" => {
                if args.len() != 1 {
                    return Err(anyhow!("time.format expects 1 arg (epoch_ms)"));
                }
                match &args[0] {
                    Value::Number(ms) => {
                        Ok(Value::String(format!("epoch+{}s", (*ms / 1000.0) as u64)))
                    }
                    _ => Err(anyhow!("time.format: argument must be a number")),
                }
            }

            // ── rand.* namespace ──────────────────────────────────────────────
            "__pasta_rand_int" => {
                let nanos = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos();
                Ok(Value::Number((nanos % 0x7fff) as f64))
            }
            "rand.float" => Ok(Value::Number(
                self.rng.next_u64() as f64 / (u64::MAX as f64 + 1.0),
            )),
            "rand.seed" => Ok(Value::None),
            "rand.choice" => {
                if args.len() != 1 {
                    return Err(anyhow!("rand.choice expects 1 arg (list)"));
                }
                match &args[0] {
                    Value::List(l) if !l.is_empty() => {
                        let idx = (self.rng.next_u64() as usize) % l.len();
                        Ok(l[idx].clone())
                    }
                    Value::List(_) => Err(anyhow!("rand.choice: list is empty")),
                    _ => Err(anyhow!("rand.choice: argument must be a list")),
                }
            }

            // ── gc.* namespace ────────────────────────────────────────────────
            "gc.collect" => {
                let n = self.collect_garbage();
                Ok(Value::Number(n as f64))
            }
            "gc.count" => Ok(Value::Number(self.gc.allocated_count() as f64)),
            "gc.stats" => Ok(Value::String(format!(
                "gc: {} objects live",
                self.gc.allocated_count()
            ))),
            "gc.pause" | "gc.resume" => Ok(Value::None),

            // ── debug.* namespace ─────────────────────────────────────────────
            "debug.print" => {
                for v in &args {
                    println!("[DEBUG] {}", Executor::value_to_string(v));
                }
                Ok(Value::None)
            }
            "debug.type" => {
                if args.len() != 1 {
                    return Err(anyhow!("debug.type expects 1 argument"));
                }
                let t = match &args[0] {
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Bool(_) => "bool",
                    Value::List(_) => "list",
                    Value::Tensor(_) => "tensor",
                    Value::Lambda(_, _, _) => "lambda",
                    Value::LazyImport { .. } => "lazy",
                    Value::Heap(_) => "heap",
                    Value::Dict(_) => "dict",
                    Value::Pending(_, _) => "pending",
                    Value::Pointer(_) => "pointer",
                    Value::FamilyNode { .. } => "family_node",
                    Value::Builtin(_) => "builtin",
                    Value::None => "none",
                };
                Ok(Value::String(t.to_string()))
            }
            "debug.len" => {
                if args.len() != 1 {
                    return Err(anyhow!("debug.len expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(Value::Number(l.len() as f64)),
                    Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                    _ => Err(anyhow!("debug.len: expected list or string")),
                }
            }
            "debug.dump" => {
                self.env.debug_print();
                Ok(Value::None)
            }
            "debug.trace" => {
                if args.len() != 1 {
                    return Err(anyhow!("debug.trace expects 1 argument"));
                }
                if std::env::var("PASTA_TRACE").is_ok() {
                    println!("[TRACE] {}", Executor::value_to_string(&args[0]));
                }
                Ok(Value::None)
            }
            "debug.assert" => {
                if args.is_empty() {
                    return Err(anyhow!("debug.assert expects 1 or 2 arguments"));
                }
                if !self.value_is_truthy(&args[0]) {
                    let msg = args
                        .get(1)
                        .map(Executor::value_to_string)
                        .unwrap_or_else(|| "assertion failed".to_string());
                    return Err(anyhow!("[ASSERT] {}", msg));
                }
                Ok(Value::None)
            }
            "debug.backtrace" => {
                let bt = self
                    .traceback
                    .0
                    .iter()
                    .enumerate()
                    .map(|(i, f)| {
                        format!(
                            "  #{} {}:{} — {}",
                            i, f.span.start_line, f.span.start_col, f.context
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                let out = if bt.is_empty() {
                    "  (no frames)".to_string()
                } else {
                    bt
                };
                println!("[BACKTRACE]\n{}", out);
                Ok(Value::None)
            }

            // ── fs.* namespace ────────────────────────────────────────────────
            "fs.read" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.read expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => std::fs::read_to_string(p)
                        .map(Value::String)
                        .map_err(|e| anyhow!("fs.read: {}", e)),
                    _ => Err(anyhow!("fs.read: path must be a string")),
                }
            }
            "fs.write" => {
                if args.len() != 2 {
                    return Err(anyhow!("fs.write expects 2 arguments (path, content)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(p), v) => {
                        let content = Executor::value_to_string(v);
                        std::fs::write(p, &content).map_err(|e| anyhow!("fs.write: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.write: path must be a string")),
                }
            }
            "fs.append" => {
                if args.len() != 2 {
                    return Err(anyhow!("fs.append expects 2 arguments (path, content)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(p), v) => {
                        use std::io::Write;
                        let content = Executor::value_to_string(v);
                        let mut f = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(p)
                            .map_err(|e| anyhow!("fs.append: {}", e))?;
                        f.write_all(content.as_bytes())
                            .map_err(|e| anyhow!("fs.append: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.append: path must be a string")),
                }
            }
            "fs.exists" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.exists expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::Bool(std::path::Path::new(p).exists())),
                    _ => Err(anyhow!("fs.exists: path must be a string")),
                }
            }
            "fs.delete" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.delete expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => {
                        std::fs::remove_file(p).map_err(|e| anyhow!("fs.delete: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.delete: path must be a string")),
                }
            }
            "fs.list" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.list expects 1 argument (dir)"));
                }
                match &args[0] {
                    Value::String(p) => {
                        let entries = std::fs::read_dir(p)
                            .map_err(|e| anyhow!("fs.list: {}", e))?
                            .flatten()
                            .filter_map(|e| {
                                e.file_name().to_str().map(|s| Value::String(s.to_string()))
                            })
                            .collect();
                        Ok(Value::List(entries))
                    }
                    _ => Err(anyhow!("fs.list: path must be a string")),
                }
            }
            "fs.mkdir" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.mkdir expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => {
                        std::fs::create_dir_all(p).map_err(|e| anyhow!("fs.mkdir: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.mkdir: path must be a string")),
                }
            }
            "fs.rmdir" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.rmdir expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => {
                        std::fs::remove_dir(p).map_err(|e| anyhow!("fs.rmdir: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.rmdir: path must be a string")),
                }
            }
            "fs.size" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.size expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => {
                        let meta = std::fs::metadata(p).map_err(|e| anyhow!("fs.size: {}", e))?;
                        Ok(Value::Number(meta.len() as f64))
                    }
                    _ => Err(anyhow!("fs.size: path must be a string")),
                }
            }
            "fs.is_dir" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.is_dir expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::Bool(std::path::Path::new(p).is_dir())),
                    _ => Err(anyhow!("fs.is_dir: path must be a string")),
                }
            }
            "fs.is_file" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.is_file expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::Bool(std::path::Path::new(p).is_file())),
                    _ => Err(anyhow!("fs.is_file: path must be a string")),
                }
            }
            "fs.copy" => {
                if args.len() != 2 {
                    return Err(anyhow!("fs.copy expects 2 arguments (src, dst)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(src), Value::String(dst)) => {
                        std::fs::copy(src, dst).map_err(|e| anyhow!("fs.copy: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.copy: arguments must be strings")),
                }
            }
            "fs.move" => {
                if args.len() != 2 {
                    return Err(anyhow!("fs.move expects 2 arguments (src, dst)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(src), Value::String(dst)) => {
                        std::fs::rename(src, dst).map_err(|e| anyhow!("fs.move: {}", e))?;
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.move: arguments must be strings")),
                }
            }
            "fs.realpath" | "fs.getcwd" => {
                if name == "fs.getcwd" {
                    let cwd = std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| ".".to_string());
                    return Ok(Value::String(cwd));
                }
                if args.len() != 1 {
                    return Err(anyhow!("fs.realpath expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::String(
                        std::fs::canonicalize(p)
                            .map(|p| p.to_string_lossy().to_string())
                            .unwrap_or_else(|_| p.clone()),
                    )),
                    _ => Err(anyhow!("fs.realpath: path must be a string")),
                }
            }
            "fs.touch" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.touch expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => {
                        let f = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(p)
                            .map_err(|e| anyhow!("fs.touch: {}", e))?;
                        drop(f);
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("fs.touch: path must be a string")),
                }
            }
            "fs.basename" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.basename expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::String(
                        std::path::Path::new(p)
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| p.clone()),
                    )),
                    _ => Err(anyhow!("fs.basename: path must be a string")),
                }
            }
            "fs.dirname" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.dirname expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::String(
                        std::path::Path::new(p)
                            .parent()
                            .map(|d| d.to_string_lossy().to_string())
                            .unwrap_or_else(|| ".".to_string()),
                    )),
                    _ => Err(anyhow!("fs.dirname: path must be a string")),
                }
            }
            "fs.ext" => {
                if args.len() != 1 {
                    return Err(anyhow!("fs.ext expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::String(
                        std::path::Path::new(p)
                            .extension()
                            .map(|e| e.to_string_lossy().to_string())
                            .unwrap_or_default(),
                    )),
                    _ => Err(anyhow!("fs.ext: path must be a string")),
                }
            }

            // ── net.* namespace (stubs) ───────────────────────────────────────
            "net.get" | "net.post" | "net.connect" | "net.send" | "net.recv" | "net.close" => {
                Err(anyhow!(
                    "{}: networking is a stub; enable the net feature in Cargo.toml",
                    name
                ))
            }

            // ── ffi.* namespace (stubs) ───────────────────────────────────────
            "ffi.load" | "ffi.call" | "ffi.close" | "ffi.symbol" => Err(anyhow!(
                "{}: FFI is a stub; enable the ffi feature in Cargo.toml",
                name
            )),

            // ── thread.* namespace ────────────────────────────────────────────
            "thread.id" => Ok(Value::Number(
                format!("{:?}", std::thread::current().id())
                    .trim_start_matches("ThreadId(")
                    .trim_end_matches(")")
                    .parse::<f64>()
                    .unwrap_or(0.0),
            )),
            "thread.count" => Ok(Value::Number(num_cpus_estimate() as f64)),
            "thread.yield" | "thread.spawn" | "thread.join" => {
                if name == "thread.yield" {
                    std::thread::yield_now();
                    return Ok(Value::None);
                }
                Err(anyhow!(
                    "{}: use PASTA DO blocks for concurrency; thread.spawn/join are stubs",
                    name
                ))
            }
            "thread.sleep" => {
                if args.len() != 1 {
                    return Err(anyhow!("thread.sleep expects 1 arg (ms)"));
                }
                match &args[0] {
                    Value::Number(ms) => {
                        std::thread::sleep(std::time::Duration::from_millis(*ms as u64));
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("thread.sleep: argument must be a number (ms)")),
                }
            }

            // ── device.* namespace ────────────────────────────────────────────
            "device.arch" => {
                let arch = if cfg!(target_arch = "x86_64") {
                    "x86_64"
                } else if cfg!(target_arch = "aarch64") {
                    "aarch64"
                } else if cfg!(target_arch = "arm") {
                    "arm"
                } else if cfg!(target_arch = "wasm32") {
                    "wasm32"
                } else {
                    "unknown"
                };
                Ok(Value::String(arch.to_string()))
            }
            "device.cpu" | "device.name" => Ok(Value::String("cpu".to_string())),
            "device.cores" => Ok(Value::Number(num_cpus_estimate() as f64)),
            "device.ram" => Ok(Value::Number(0.0)),
            "device.gpu" => Ok(Value::String("none".to_string())),
            "device.features" => {
                let mut feats: Vec<Value> = Vec::new();
                if cfg!(target_feature = "avx2") {
                    feats.push(Value::String("avx2".to_string()));
                }
                if cfg!(target_feature = "sse4.1") {
                    feats.push(Value::String("sse4.1".to_string()));
                }
                if cfg!(target_feature = "neon") {
                    feats.push(Value::String("neon".to_string()));
                }
                Ok(Value::List(feats))
            }

            // ── tensor.* extended ─────────────────────────────────────────────
            "tensor.fill" | "tensor_fill" => {
                if args.len() != 2 {
                    return Err(anyhow!("tensor.fill expects 2 args (shape, value)"));
                }
                let fill_val = match &args[1] {
                    Value::Number(n) => *n,
                    _ => return Err(anyhow!("tensor.fill: value must be a number")),
                };
                let shape = match &args[0] {
                    Value::List(l) => l
                        .iter()
                        .map(|v| match v {
                            Value::Number(n) => Ok(*n as usize),
                            _ => Err(anyhow!("tensor.fill: shape dims must be numbers")),
                        })
                        .collect::<Result<Vec<usize>>>()?,
                    Value::Number(n) => vec![*n as usize],
                    _ => return Err(anyhow!("tensor.fill: shape must be a list or number")),
                };
                let numel: usize = shape.iter().product();
                Ok(Value::Tensor(RuntimeTensor::new(
                    shape,
                    "float32",
                    vec![fill_val; numel],
                )))
            }
            "tensor.clone" | "tensor_clone" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.clone expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Tensor(t.clone())),
                    _ => Err(anyhow!("tensor.clone: argument must be a tensor")),
                }
            }
            "tensor.to_list" | "tensor_to_list" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.to_list expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::List(
                        t.data.iter().map(|&n| Value::Number(n)).collect(),
                    )),
                    _ => Err(anyhow!("tensor.to_list: argument must be a tensor")),
                }
            }

            // ── rand.* extended ───────────────────────────────────────────────
            "rand.int" => match args.len() {
                0 => Ok(Value::Number(
                    (self.rng.next_u64() as i64 & 0x7FFF_FFFF) as f64,
                )),
                1 => match &args[0] {
                    Value::Number(n) => {
                        let max = if *n <= 0.0 { 0 } else { *n as u64 };
                        if max == 0 {
                            return Ok(Value::Number(0.0));
                        }
                        Ok(Value::Number((self.rng.next_u64() % max) as f64))
                    }
                    _ => Err(anyhow!("rand.int: expected numeric argument")),
                },
                2 => match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => {
                        let (mn, mx) = (*a as i64, *b as i64);
                        if mx <= mn {
                            return Ok(Value::Number(mn as f64));
                        }
                        Ok(Value::Number(
                            ((self.rng.next_u64() % (mx - mn) as u64) as i64 + mn) as f64,
                        ))
                    }
                    _ => Err(anyhow!("rand.int: expected two numeric arguments")),
                },
                _ => Err(anyhow!("rand.int: expected 0, 1, or 2 arguments")),
            },
            "rand.ls" => {
                let n = match args.first() {
                    None => 8usize,
                    Some(Value::Number(x)) => *x as usize,
                    _ => return Err(anyhow!("rand.ls: expected numeric length")),
                };
                Ok(Value::List(
                    (0..n)
                        .map(|_| Value::Number(self.rng.next_u64() as f64 / u64::MAX as f64))
                        .collect(),
                ))
            }

            // ── tensor stat functions ─────────────────────────────────────────
            "tensor.rand" | "tensor_rand" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.rand expects 1 argument (shape)"));
                }
                let shape = match &args[0] {
                    Value::List(dims) => dims
                        .iter()
                        .map(|d| match d {
                            Value::Number(n) => Ok(n.round() as usize),
                            _ => Err(anyhow!("tensor.rand: shape dimensions must be numbers")),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    Value::Number(n) => vec![n.round() as usize],
                    _ => return Err(anyhow!("tensor.rand: argument must be a list or number")),
                };
                let numel: usize = shape.iter().product();
                let seed = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .subsec_nanos() as u64;
                let mut state = seed ^ 0xdeadbeef_cafebabe;
                let data: Vec<f64> = (0..numel)
                    .map(|_| {
                        state = state
                            .wrapping_mul(6364136223846793005)
                            .wrapping_add(1442695040888963407);
                        (state >> 33) as f64 / u32::MAX as f64
                    })
                    .collect();
                Ok(Value::Tensor(RuntimeTensor::new(
                    shape,
                    "float32".to_string(),
                    data,
                )))
            }
            "tensor.eye" | "tensor_eye" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.eye expects 1 argument (n)"));
                }
                let n = match &args[0] {
                    Value::Number(x) => x.round() as usize,
                    _ => return Err(anyhow!("tensor.eye argument must be a number")),
                };
                let mut data = vec![0.0f64; n * n];
                for i in 0..n {
                    data[i * n + i] = 1.0;
                }
                Ok(Value::Tensor(RuntimeTensor::new(
                    vec![n, n],
                    "float32".to_string(),
                    data,
                )))
            }
            "tensor.from_list" | "tensor_from_list" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.from_list expects 1 argument"));
                }
                match &args[0] {
                    Value::List(items) => {
                        let mut data = Vec::with_capacity(items.len());
                        let mut has_float = false;
                        for item in items {
                            match item {
                                Value::Number(n) => {
                                    if n.fract() != 0.0 {
                                        has_float = true;
                                    }
                                    data.push(*n);
                                }
                                other => {
                                    return Err(anyhow!(
                                        "tensor.from_list: non-numeric element: {:?}",
                                        other
                                    ))
                                }
                            }
                        }
                        let dtype = if has_float { "float32" } else { "int32" }.to_string();
                        let len = data.len();
                        Ok(Value::Tensor(RuntimeTensor::new(vec![len], dtype, data)))
                    }
                    _ => Err(anyhow!("tensor.from_list expects a list")),
                }
            }
            "tensor.shape" | "tensor_shape" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.shape expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::List(
                        t.shape.iter().map(|&s| Value::Number(s as f64)).collect(),
                    )),
                    _ => Err(anyhow!("tensor.shape expects a tensor")),
                }
            }
            "tensor.dtype" | "tensor_dtype" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.dtype expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::String(t.dtype.clone())),
                    _ => Err(anyhow!("tensor.dtype expects a tensor")),
                }
            }
            "tensor.sum" | "tensor_sum" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.sum expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Number(t.data.iter().copied().sum())),
                    _ => Err(anyhow!("tensor.sum expects a tensor")),
                }
            }
            "tensor.mean" | "tensor_mean" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.mean expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        let total: f64 = t.data.iter().copied().sum();
                        let cnt = t.data.len() as f64;
                        Ok(Value::Number(if cnt == 0.0 { 0.0 } else { total / cnt }))
                    }
                    _ => Err(anyhow!("tensor.mean expects a tensor")),
                }
            }

            "tensor" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor() expects 1 argument"));
                }
                return self.build_tensor_from_value(&args[0]);
            }
            "tensor.zeros" | "tensor_zeros" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.zeros expects 1 argument (shape)"));
                }
                let shape = match &args[0] {
                    Value::List(dims) => dims
                        .iter()
                        .map(|d| match d {
                            Value::Number(n) => Ok(n.round() as usize),
                            _ => Err(anyhow!("tensor.zeros: shape must be numbers")),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    Value::Number(n) => vec![n.round() as usize],
                    _ => return Err(anyhow!("tensor.zeros: argument must be a list or number")),
                };
                let numel: usize = shape.iter().product();
                Ok(Value::Tensor(RuntimeTensor::new(
                    shape,
                    "float32".to_string(),
                    vec![0.0; numel],
                )))
            }
            "tensor.ones" | "tensor_ones" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.ones expects 1 argument (shape)"));
                }
                let shape = match &args[0] {
                    Value::List(dims) => dims
                        .iter()
                        .map(|d| match d {
                            Value::Number(n) => Ok(n.round() as usize),
                            _ => Err(anyhow!("tensor.ones: shape must be numbers")),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    Value::Number(n) => vec![n.round() as usize],
                    _ => return Err(anyhow!("tensor.ones: argument must be a list or number")),
                };
                let numel: usize = shape.iter().product();
                Ok(Value::Tensor(RuntimeTensor::new(
                    shape,
                    "float32".to_string(),
                    vec![1.0; numel],
                )))
            }
            "tensor.reshape" | "tensor_reshape" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "tensor.reshape expects 2 arguments (tensor, shape)"
                    ));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(t), Value::List(dims)) => {
                        let new_shape: Vec<usize> = dims
                            .iter()
                            .map(|d| match d {
                                Value::Number(n) => Ok(n.round() as usize),
                                _ => Err(anyhow!("tensor.reshape: shape must be numbers")),
                            })
                            .collect::<Result<Vec<_>>>()?;
                        let new_numel: usize = new_shape.iter().product();
                        if new_numel != t.data.len() {
                            return Err(anyhow!(
                                "tensor.reshape: size mismatch {} vs {}",
                                t.data.len(),
                                new_numel
                            ));
                        }
                        Ok(Value::Tensor(RuntimeTensor::new(
                            new_shape,
                            t.dtype.clone(),
                            t.data.clone(),
                        )))
                    }
                    _ => Err(anyhow!("tensor.reshape expects (tensor, list)")),
                }
            }
            "tensor.transpose" | "tensor_transpose" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.transpose expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) if t.rank() == 2 => {
                        let (rows, cols) = (t.shape[0], t.shape[1]);
                        let mut data = vec![0.0f64; rows * cols];
                        for r in 0..rows {
                            for c in 0..cols {
                                data[c * rows + r] = t.data[r * cols + c];
                            }
                        }
                        Ok(Value::Tensor(RuntimeTensor::new(
                            vec![cols, rows],
                            t.dtype.clone(),
                            data,
                        )))
                    }
                    Value::Tensor(t) => Err(anyhow!(
                        "tensor.transpose: expected 2D tensor, got {:?}",
                        t.shape
                    )),
                    _ => Err(anyhow!("tensor.transpose expects a tensor")),
                }
            }
            "tensor.flatten" | "tensor_flatten" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor.flatten expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        let len = t.data.len();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            vec![len],
                            t.dtype.clone(),
                            t.data.clone(),
                        )))
                    }
                    _ => Err(anyhow!("tensor.flatten expects a tensor")),
                }
            }
            "tensor.add" | "tensor_add" => {
                if args.len() != 2 {
                    return Err(anyhow!("tensor.add expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(a), Value::Tensor(b)) => {
                        if a.shape != b.shape {
                            return Err(anyhow!(
                                "tensor.add: shape mismatch {:?} vs {:?}",
                                a.shape,
                                b.shape
                            ));
                        }
                        let data: Vec<f64> = a
                            .data
                            .iter()
                            .zip(b.data.iter())
                            .map(|(x, y)| x + y)
                            .collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    (Value::Tensor(a), Value::Number(s)) => {
                        let data: Vec<f64> = a.data.iter().map(|x| x + s).collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    _ => Err(anyhow!(
                        "tensor.add: expected (tensor, tensor) or (tensor, number)"
                    )),
                }
            }
            "tensor.sub" | "tensor_sub" => {
                if args.len() != 2 {
                    return Err(anyhow!("tensor.sub expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(a), Value::Tensor(b)) => {
                        if a.shape != b.shape {
                            return Err(anyhow!("tensor.sub: shape mismatch"));
                        }
                        let data: Vec<f64> = a
                            .data
                            .iter()
                            .zip(b.data.iter())
                            .map(|(x, y)| x - y)
                            .collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    (Value::Tensor(a), Value::Number(s)) => {
                        let data: Vec<f64> = a.data.iter().map(|x| x - s).collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    _ => Err(anyhow!(
                        "tensor.sub: expected (tensor, tensor) or (tensor, number)"
                    )),
                }
            }
            "tensor.mul" | "tensor_mul" => {
                if args.len() != 2 {
                    return Err(anyhow!("tensor.mul expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(a), Value::Tensor(b)) => {
                        if a.shape != b.shape {
                            return Err(anyhow!("tensor.mul: shape mismatch"));
                        }
                        let data: Vec<f64> = a
                            .data
                            .iter()
                            .zip(b.data.iter())
                            .map(|(x, y)| x * y)
                            .collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    (Value::Tensor(a), Value::Number(s)) => {
                        let data: Vec<f64> = a.data.iter().map(|x| x * s).collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    _ => Err(anyhow!(
                        "tensor.mul: expected (tensor, tensor) or (tensor, number)"
                    )),
                }
            }
            "tensor.div" | "tensor_div" => {
                if args.len() != 2 {
                    return Err(anyhow!("tensor.div expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(a), Value::Tensor(b)) => {
                        if a.shape != b.shape {
                            return Err(anyhow!("tensor.div: shape mismatch"));
                        }
                        let data: Vec<f64> = a
                            .data
                            .iter()
                            .zip(b.data.iter())
                            .map(|(x, y)| if *y == 0.0 { f64::NAN } else { x / y })
                            .collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    (Value::Tensor(a), Value::Number(s)) => {
                        if *s == 0.0 {
                            return Err(anyhow!("tensor.div: division by zero"));
                        }
                        let data: Vec<f64> = a.data.iter().map(|x| x / s).collect();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            a.shape.clone(),
                            a.dtype.clone(),
                            data,
                        )))
                    }
                    _ => Err(anyhow!(
                        "tensor.div: expected (tensor, tensor) or (tensor, number)"
                    )),
                }
            }

            // ── AI builtins ───────────────────────────────────────────────────
            "tensor.matmul" | "tensor_matmul" => {
                if args.len() != 2 {
                    return Err(anyhow!("tensor.matmul expects 2 arguments (a, b)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_matmul(a, b),
                    _ => Err(anyhow!("tensor.matmul requires two tensors")),
                }
            }
            "ai.relu" | "ai_relu" => {
                if args.len() != 1 {
                    return Err(anyhow!("ai.relu expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Tensor(RuntimeTensor::new(
                        t.shape.clone(),
                        t.dtype.clone(),
                        ai_network::AILayer::relu(&t.data),
                    ))),
                    _ => Err(anyhow!("ai.relu expects a tensor")),
                }
            }
            "ai.softmax" | "ai_softmax" => {
                if args.len() != 1 {
                    return Err(anyhow!("ai.softmax expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Tensor(RuntimeTensor::new(
                        t.shape.clone(),
                        t.dtype.clone(),
                        ai_network::AILayer::softmax(&t.data),
                    ))),
                    _ => Err(anyhow!("ai.softmax expects a tensor")),
                }
            }
            "ai.loss.mse" | "ai_loss_mse" => {
                if args.len() != 2 {
                    return Err(anyhow!("ai.loss.mse expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(pred), Value::Tensor(target)) => {
                        if pred.data.len() != target.data.len() {
                            return Err(anyhow!("MSE loss: tensors must have same size"));
                        }
                        Ok(Value::Number(ai_network::mse_loss(
                            &pred.data,
                            &target.data,
                        )?))
                    }
                    _ => Err(anyhow!("ai.loss.mse expects two tensors")),
                }
            }
            "ai.loss.crossentropy" | "ai_loss_crossentropy" => {
                if args.len() != 2 {
                    return Err(anyhow!("ai.loss.crossentropy expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Tensor(logits), Value::Number(target)) => Ok(Value::Number(
                        ai_network::cross_entropy_loss(&logits.data, target.round() as usize)?,
                    )),
                    _ => Err(anyhow!(
                        "ai.loss.crossentropy expects (logits_tensor, target_class_number)"
                    )),
                }
            }
            "ai.tensor_to_list" | "ai_tensor_to_list" => {
                if args.len() != 1 {
                    return Err(anyhow!("ai.tensor_to_list expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::List(
                        t.data.iter().map(|&v| Value::Number(v)).collect(),
                    )),
                    _ => Err(anyhow!("ai.tensor_to_list expects a tensor")),
                }
            }
            "ai.list_to_tensor" | "ai_list_to_tensor" => {
                if args.len() != 1 {
                    return Err(anyhow!("ai.list_to_tensor expects 1 argument"));
                }
                match &args[0] {
                    Value::List(items) => {
                        let data: Result<Vec<f64>> = items
                            .iter()
                            .map(|item| match item {
                                Value::Number(n) => Ok(*n),
                                _ => Err(anyhow!(
                                    "ai.list_to_tensor: list must contain only numbers"
                                )),
                            })
                            .collect();
                        let data = data?;
                        let len = data.len();
                        Ok(Value::Tensor(RuntimeTensor::new(
                            vec![len],
                            "float32".to_string(),
                            data,
                        )))
                    }
                    _ => Err(anyhow!("ai.list_to_tensor expects a list")),
                }
            }

            // ── Shell / File System ───────────────────────────────────────────
            "shell.pwd" | "shell_pwd" | "pwd" => Ok(Value::String(self.shell.pwd())),
            "shell.cd" | "shell_cd" | "cd" => {
                if args.len() != 1 {
                    return Err(anyhow!("cd expects 1 argument (directory)"));
                }
                match &args[0] {
                    Value::String(path) => self.shell.cd(path).map(Value::String),
                    _ => Err(anyhow!("cd: path must be a string")),
                }
            }
            "shell.ls" | "shell_ls" | "ls" => {
                let path = match args.len() {
                    0 => None,
                    1 => match &args[0] {
                        Value::String(p) => Some(p.as_str()),
                        _ => return Err(anyhow!("ls: path must be a string")),
                    },
                    _ => return Err(anyhow!("ls expects 0 or 1 argument")),
                };
                self.shell.ls(path).map(Value::List)
            }
            "shell.ls_long" | "shell_ls_long" | "ls_long" => {
                let path = match args.len() {
                    0 => None,
                    1 => match &args[0] {
                        Value::String(p) => Some(p.as_str()),
                        _ => return Err(anyhow!("ls_long: path must be a string")),
                    },
                    _ => return Err(anyhow!("ls_long expects 0 or 1 argument")),
                };
                self.shell.ls_long(path).map(Value::List)
            }
            "shell.mkdir" | "shell_mkdir" | "mkdir" => {
                if args.is_empty() || args.len() > 2 {
                    return Err(anyhow!("mkdir expects 1-2 arguments (path, [parents])"));
                }
                let path = match &args[0] {
                    Value::String(p) => p,
                    _ => return Err(anyhow!("mkdir: path must be a string")),
                };
                let parents = match args.get(1) {
                    None => false,
                    Some(Value::Bool(b)) => *b,
                    Some(Value::Number(n)) => *n != 0.0,
                    _ => return Err(anyhow!("mkdir: parents must be a boolean")),
                };
                self.shell.mkdir(path, parents).map(Value::String)
            }
            "shell.rm" | "shell_rm" | "rm" => {
                if args.len() != 1 {
                    return Err(anyhow!("rm expects 1 argument (file)"));
                }
                match &args[0] {
                    Value::String(path) => self.shell.rm(path).map(Value::String),
                    _ => Err(anyhow!("rm: path must be a string")),
                }
            }
            "shell.rmdir" | "shell_rmdir" | "rmdir" => {
                if args.len() != 1 {
                    return Err(anyhow!("rmdir expects 1 argument (directory)"));
                }
                match &args[0] {
                    Value::String(path) => self.shell.rmdir(path).map(Value::String),
                    _ => Err(anyhow!("rmdir: path must be a string")),
                }
            }
            "shell.rmdir_r" | "shell_rmdir_recursive" | "rm_r" => {
                if args.len() != 1 {
                    return Err(anyhow!("rm_r expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(path) => self.shell.rmdir_recursive(path).map(Value::String),
                    _ => Err(anyhow!("rm_r: path must be a string")),
                }
            }
            "shell.touch" | "shell_touch" | "touch" => {
                if args.len() != 1 {
                    return Err(anyhow!("touch expects 1 argument (file)"));
                }
                match &args[0] {
                    Value::String(path) => self.shell.touch(path).map(Value::String),
                    _ => Err(anyhow!("touch: path must be a string")),
                }
            }
            "shell.cp" | "shell_cp" | "cp" => {
                if args.len() != 2 {
                    return Err(anyhow!("cp expects 2 arguments (from, to)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(from), Value::String(to)) => {
                        self.shell.cp(from, to).map(Value::String)
                    }
                    _ => Err(anyhow!("cp: both arguments must be strings")),
                }
            }
            "shell.mv" | "shell_mv" | "mv" => {
                if args.len() != 2 {
                    return Err(anyhow!("mv expects 2 arguments (from, to)"));
                }
                match (&args[0], &args[1]) {
                    (Value::String(from), Value::String(to)) => {
                        self.shell.mv(from, to).map(Value::String)
                    }
                    _ => Err(anyhow!("mv: both arguments must be strings")),
                }
            }
            "shell.exists" | "shell_exists" => {
                if args.len() != 1 {
                    return Err(anyhow!("exists expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(path) => Ok(Value::Bool(self.shell.exists(path))),
                    _ => Err(anyhow!("exists: path must be a string")),
                }
            }
            "shell.is_file" | "shell_is_file" => {
                if args.len() != 1 {
                    return Err(anyhow!("is_file expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(path) => Ok(Value::Bool(self.shell.is_file(path))),
                    _ => Err(anyhow!("is_file: path must be a string")),
                }
            }
            "shell.is_dir" | "shell_is_dir" => {
                if args.len() != 1 {
                    return Err(anyhow!("is_dir expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(path) => Ok(Value::Bool(self.shell.is_dir(path))),
                    _ => Err(anyhow!("is_dir: path must be a string")),
                }
            }
            "shell.file_size" | "shell_file_size" => {
                if args.len() != 1 {
                    return Err(anyhow!("file_size expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(path) => {
                        self.shell.file_size(path).map(|s| Value::Number(s as f64))
                    }
                    _ => Err(anyhow!("file_size: path must be a string")),
                }
            }
            "shell.realpath" | "shell_realpath" => {
                if args.len() != 1 {
                    return Err(anyhow!("realpath expects 1 argument (path)"));
                }
                match &args[0] {
                    Value::String(path) => self.shell.realpath(path).map(Value::String),
                    _ => Err(anyhow!("realpath: path must be a string")),
                }
            }

            // ── String builtins ───────────────────────────────────────────────
            "string_length" | "string_len" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_length expects 1 argument"));
                }
                match &args[0] {
                    Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                    Value::List(l) => Ok(Value::Number(l.len() as f64)),
                    Value::None => Ok(Value::Number(0.0)),
                    other => Err(anyhow!("string_length: expected string, got {:?}", other)),
                }
            }
            "concat" | "string_concat" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_concat expects 2 arguments"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]) + &Executor::value_to_string(&args[1]),
                ))
            }
            "string_concat_with_sep" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "string_concat_with_sep expects 3 arguments (s1, s2, sep)"
                    ));
                }
                let (a, b, sep) = (
                    Executor::value_to_string(&args[0]),
                    Executor::value_to_string(&args[1]),
                    Executor::value_to_string(&args[2]),
                );
                Ok(Value::String(format!("{}{}{}", a, sep, b)))
            }
            "string_repeat" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_repeat expects 2 arguments (s, count)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let n = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("string_repeat: count must be a number")),
                };
                Ok(Value::String(s.repeat(n)))
            }
            "string_pad_left" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "string_pad_left expects 3 arguments (s, width, pad_char)"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let width = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("string_pad_left: width must be a number")),
                };
                let pad_ch = Executor::value_to_string(&args[2])
                    .chars()
                    .next()
                    .unwrap_or(' ');
                let char_len = s.chars().count();
                if char_len >= width {
                    Ok(Value::String(s))
                } else {
                    let padding: String = std::iter::repeat_n(pad_ch, width - char_len).collect();
                    Ok(Value::String(format!("{}{}", padding, s)))
                }
            }
            "string_pad_right" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "string_pad_right expects 3 arguments (s, width, pad_char)"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let width = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("string_pad_right: width must be a number")),
                };
                let pad_ch = Executor::value_to_string(&args[2])
                    .chars()
                    .next()
                    .unwrap_or(' ');
                let char_len = s.chars().count();
                if char_len >= width {
                    Ok(Value::String(s))
                } else {
                    let padding: String = std::iter::repeat_n(pad_ch, width - char_len).collect();
                    Ok(Value::String(format!("{}{}", s, padding)))
                }
            }
            "string_upper" | "upper" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_upper expects 1 argument"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]).to_uppercase(),
                ))
            }
            "string_lower" | "lower" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_lower expects 1 argument"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]).to_lowercase(),
                ))
            }
            "string_reverse" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_reverse expects 1 argument"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]).chars().rev().collect(),
                ))
            }
            "string_trim" | "trim" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_trim expects 1 argument"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]).trim().to_string(),
                ))
            }
            "string_starts_with" | "starts_with" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_starts_with expects 2 arguments"));
                }
                let s = Executor::value_to_string(&args[0]);
                let pre = Executor::value_to_string(&args[1]);
                Ok(Value::Bool(s.starts_with(pre.as_str())))
            }
            "string_ends_with" | "ends_with" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_ends_with expects 2 arguments"));
                }
                let s = Executor::value_to_string(&args[0]);
                let suf = Executor::value_to_string(&args[1]);
                Ok(Value::Bool(s.ends_with(suf.as_str())))
            }
            "string_contains" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_contains expects 2 arguments"));
                }
                let s = Executor::value_to_string(&args[0]);
                let sub = Executor::value_to_string(&args[1]);
                Ok(Value::Bool(s.contains(sub.as_str())))
            }
            "string_replace" | "replace" => {
                if args.len() != 3 {
                    return Err(anyhow!("string_replace expects 3 arguments (s, old, new)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let old = Executor::value_to_string(&args[1]);
                let new = Executor::value_to_string(&args[2]);
                Ok(Value::String(s.replace(old.as_str(), &new)))
            }
            "string_split" | "split" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_split expects 2 arguments (s, delimiter)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let del = Executor::value_to_string(&args[1]);
                Ok(Value::List(
                    s.split(del.as_str())
                        .map(|p| Value::String(p.to_string()))
                        .collect(),
                ))
            }
            "string_join" | "join" => {
                if args.len() != 2 {
                    return Err(anyhow!("string_join expects 2 arguments (list, delimiter)"));
                }
                let del = Executor::value_to_string(&args[1]);
                match &args[0] {
                    Value::List(items) => {
                        let parts: Vec<String> =
                            items.iter().map(Executor::value_to_string).collect();
                        Ok(Value::String(parts.join(&del)))
                    }
                    _ => Err(anyhow!("string_join: first argument must be a list")),
                }
            }
            "string_to_chars" => {
                if args.len() != 1 {
                    return Err(anyhow!("string_to_chars expects 1 argument"));
                }
                Ok(Value::List(
                    Executor::value_to_string(&args[0])
                        .chars()
                        .map(|c| Value::String(c.to_string()))
                        .collect(),
                ))
            }
            "chars_to_string" => {
                if args.len() != 1 {
                    return Err(anyhow!("chars_to_string expects 1 argument"));
                }
                match &args[0] {
                    Value::List(items) => Ok(Value::String(
                        items.iter().map(Executor::value_to_string).collect(),
                    )),
                    _ => Err(anyhow!("chars_to_string: expected a list")),
                }
            }

            // ── Extended string utilities (v1.5.5) ────────────────────────────
            "contains" => {
                if args.len() != 2 {
                    return Err(anyhow!("contains expects 2 arguments (string, substring)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let sub = Executor::value_to_string(&args[1]);
                Ok(Value::Bool(s.contains(sub.as_str())))
            }
            "trim_left" => {
                if args.len() != 1 {
                    return Err(anyhow!("trim_left expects 1 argument"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]).trim_start().to_string(),
                ))
            }
            "trim_right" => {
                if args.len() != 1 {
                    return Err(anyhow!("trim_right expects 1 argument"));
                }
                Ok(Value::String(
                    Executor::value_to_string(&args[0]).trim_end().to_string(),
                ))
            }
            // replace.all / replace.first / replace.last
            "replace.all" => {
                if args.len() != 3 {
                    return Err(anyhow!("replace.all expects 3 arguments (s, old, new)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let old = Executor::value_to_string(&args[1]);
                let new = Executor::value_to_string(&args[2]);
                Ok(Value::String(s.replace(old.as_str(), &new)))
            }
            "replace.first" => {
                if args.len() != 3 {
                    return Err(anyhow!("replace.first expects 3 arguments (s, old, new)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let old = Executor::value_to_string(&args[1]);
                let new = Executor::value_to_string(&args[2]);
                Ok(Value::String(s.replacen(old.as_str(), &new, 1)))
            }
            "replace.last" => {
                if args.len() != 3 {
                    return Err(anyhow!("replace.last expects 3 arguments (s, old, new)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let old = Executor::value_to_string(&args[1]);
                let new_s = Executor::value_to_string(&args[2]);
                if let Some(pos) = s.rfind(old.as_str()) {
                    Ok(Value::String(format!(
                        "{}{}{}",
                        &s[..pos],
                        new_s,
                        &s[pos + old.len()..]
                    )))
                } else {
                    Ok(Value::String(s))
                }
            }
            // index_of(str, substr) → 1-based position, 0 = not found
            "index_of" | "str_index_of" => {
                if args.len() != 2 {
                    return Err(anyhow!("index_of expects 2 arguments (string, substring)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let sub = Executor::value_to_string(&args[1]);
                Ok(Value::Number(match s.find(sub.as_str()) {
                    Some(byte_pos) => (s[..byte_pos].chars().count() + 1) as f64,
                    None => 0.0,
                }))
            }
            // count_occurrences(str, substr) → number
            "count_occurrences" | "str_count" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "count_occurrences expects 2 arguments (string, substring)"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let sub = Executor::value_to_string(&args[1]);
                if sub.is_empty() {
                    return Ok(Value::Number(0.0));
                }
                Ok(Value::Number(s.matches(sub.as_str()).count() as f64))
            }
            // pad_left(str, width [, char]) → string
            "pad_left" | "str_pad_left" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(anyhow!(
                        "pad_left expects 2 or 3 arguments (string, width [, pad_char])"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let width = match &args[1] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("pad_left: width must be a number")),
                };
                let pad_char = args
                    .get(2)
                    .map(|v| Executor::value_to_string(v))
                    .unwrap_or_else(|| " ".to_string());
                let pad_ch = pad_char.chars().next().unwrap_or(' ');
                let cur_len = s.chars().count();
                if cur_len >= width {
                    return Ok(Value::String(s));
                }
                let padding: String = std::iter::repeat(pad_ch).take(width - cur_len).collect();
                Ok(Value::String(format!("{}{}", padding, s)))
            }
            // pad_right(str, width [, char]) → string
            "pad_right" | "str_pad_right" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(anyhow!(
                        "pad_right expects 2 or 3 arguments (string, width [, pad_char])"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let width = match &args[1] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("pad_right: width must be a number")),
                };
                let pad_char = args
                    .get(2)
                    .map(|v| Executor::value_to_string(v))
                    .unwrap_or_else(|| " ".to_string());
                let pad_ch = pad_char.chars().next().unwrap_or(' ');
                let cur_len = s.chars().count();
                if cur_len >= width {
                    return Ok(Value::String(s));
                }
                let padding: String = std::iter::repeat(pad_ch).take(width - cur_len).collect();
                Ok(Value::String(format!("{}{}", s, padding)))
            }
            // substr(str, start, length?) → string  (1-based start)
            "substr" | "substring" => {
                if args.len() < 2 || args.len() > 3 {
                    return Err(anyhow!(
                        "substr expects 2 or 3 arguments (string, start [, length])"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len();
                let start = match &args[1] {
                    Value::Number(n) => {
                        let i = *n as isize - 1;
                        if i < 0 {
                            0
                        } else {
                            i as usize
                        }
                    }
                    _ => return Err(anyhow!("substr: start must be a number")),
                };
                if start >= len {
                    return Ok(Value::String(String::new()));
                }
                let take = args
                    .get(2)
                    .map(|v| match v {
                        Value::Number(n) => *n as usize,
                        _ => len,
                    })
                    .unwrap_or(len - start);
                let end = (start + take).min(len);
                Ok(Value::String(chars[start..end].iter().collect()))
            }

            // ── Regex (v1.5.5) ────────────────────────────────────────────────

            // regex_match(str, pattern) → bool
            "regex_match" | "REGEX_MATCH" => {
                if args.len() != 2 {
                    return Err(anyhow!("regex_match expects 2 arguments (string, pattern)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let pat = Executor::value_to_string(&args[1]);
                match regex::Regex::new(&pat) {
                    Ok(re) => Ok(Value::Bool(re.is_match(&s))),
                    Err(e) => Err(anyhow!("regex_match: invalid pattern: {}", e)),
                }
            }
            // regex_find(str, pattern) → list of all matches
            "regex_find" | "REGEX_FIND" => {
                if args.len() != 2 {
                    return Err(anyhow!("regex_find expects 2 arguments (string, pattern)"));
                }
                let s = Executor::value_to_string(&args[0]);
                let pat = Executor::value_to_string(&args[1]);
                match regex::Regex::new(&pat) {
                    Ok(re) => {
                        let matches: Vec<Value> = re
                            .find_iter(&s)
                            .map(|m| Value::String(m.as_str().to_string()))
                            .collect();
                        let id = self.gc.allocate(Value::List(matches));
                        Ok(Value::Heap(id))
                    }
                    Err(e) => Err(anyhow!("regex_find: invalid pattern: {}", e)),
                }
            }
            // regex_replace(str, pattern, replacement) → string
            "regex_replace" | "REGEX_REPLACE" => {
                if args.len() != 3 {
                    return Err(anyhow!(
                        "regex_replace expects 3 arguments (string, pattern, replacement)"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let pat = Executor::value_to_string(&args[1]);
                let rep = Executor::value_to_string(&args[2]);
                match regex::Regex::new(&pat) {
                    Ok(re) => Ok(Value::String(re.replace_all(&s, rep.as_str()).to_string())),
                    Err(e) => Err(anyhow!("regex_replace: invalid pattern: {}", e)),
                }
            }
            // regex_captures(str, pattern) → list of capture groups from first match
            "regex_captures" | "REGEX_CAPTURES" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "regex_captures expects 2 arguments (string, pattern)"
                    ));
                }
                let s = Executor::value_to_string(&args[0]);
                let pat = Executor::value_to_string(&args[1]);
                match regex::Regex::new(&pat) {
                    Ok(re) => {
                        let caps: Vec<Value> = match re.captures(&s) {
                            Some(c) => c
                                .iter()
                                .map(|m| match m {
                                    Some(m) => Value::String(m.as_str().to_string()),
                                    None => Value::None,
                                })
                                .collect(),
                            None => Vec::new(),
                        };
                        let id = self.gc.allocate(Value::List(caps));
                        Ok(Value::Heap(id))
                    }
                    Err(e) => Err(anyhow!("regex_captures: invalid pattern: {}", e)),
                }
            }

            // ── List builtins ─────────────────────────────────────────────────
            "len" | "length" => {
                if args.len() != 1 {
                    return Err(anyhow!("length() takes exactly one argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(Value::Number(l.len() as f64)),
                    Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                    Value::Tensor(t) => Ok(Value::Number(t.numel() as f64)),
                    _ => Err(anyhow!("length() expects list, string, or tensor")),
                }
            }
            "list_len" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_len expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(Value::Number(l.len() as f64)),
                    Value::String(s) => Ok(Value::Number(s.chars().count() as f64)),
                    _ => Err(anyhow!("list_len expects a list or string")),
                }
            }
            "list_push" => {
                if args.len() != 2 {
                    return Err(anyhow!("list_push expects 2 arguments (list, element)"));
                }
                match &args[0] {
                    Value::List(l) => {
                        let mut out = l.clone();
                        out.push(args[1].clone());
                        Ok(Value::List(out))
                    }
                    _ => Err(anyhow!("list_push: first argument must be a list")),
                }
            }
            "list_pop" => {
                // Returns the last element (non-destructive — Pasta values are immutable snapshots).
                if args.len() != 1 {
                    return Err(anyhow!("list_pop expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(l.last().cloned().unwrap_or(Value::None)),
                    _ => Err(anyhow!("list_pop: expected a list")),
                }
            }
            "list_first" | "first" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_first expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(l.first().cloned().unwrap_or(Value::None)),
                    _ => Err(anyhow!("list_first: expected a list")),
                }
            }
            "list_last" | "last" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_last expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(l.last().cloned().unwrap_or(Value::None)),
                    _ => Err(anyhow!("list_last: expected a list")),
                }
            }
            "list_rest" | "rest" | "tail" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_rest expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) if l.len() > 1 => Ok(Value::List(l[1..].to_vec())),
                    Value::List(_) => Ok(Value::List(vec![])),
                    _ => Err(anyhow!("list_rest: expected a list")),
                }
            }
            "list_rev" | "list_reverse" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_rev expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => {
                        let mut rev = l.clone();
                        rev.reverse();
                        Ok(Value::List(rev))
                    }
                    _ => Err(anyhow!("list_rev: expected a list")),
                }
            }
            "list_sort" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_sort expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => {
                        let mut sorted = l.clone();
                        sorted.sort_by(|a, b| match (a, b) {
                            (Value::Number(x), Value::Number(y)) => {
                                x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal)
                            }
                            _ => Executor::value_to_string(a).cmp(&Executor::value_to_string(b)),
                        });
                        Ok(Value::List(sorted))
                    }
                    _ => Err(anyhow!("list_sort: expected a list")),
                }
            }
            "list_min" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_min expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) if !l.is_empty() => {
                        let m = l
                            .iter()
                            .filter_map(|v| {
                                if let Value::Number(n) = v {
                                    Some(*n)
                                } else {
                                    None
                                }
                            })
                            .fold(f64::INFINITY, f64::min);
                        Ok(Value::Number(m))
                    }
                    Value::List(_) => Err(anyhow!("list_min: empty list")),
                    _ => Err(anyhow!("list_min: expected a list")),
                }
            }
            "list_max" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_max expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) if !l.is_empty() => {
                        let m = l
                            .iter()
                            .filter_map(|v| {
                                if let Value::Number(n) = v {
                                    Some(*n)
                                } else {
                                    None
                                }
                            })
                            .fold(f64::NEG_INFINITY, f64::max);
                        Ok(Value::Number(m))
                    }
                    Value::List(_) => Err(anyhow!("list_max: empty list")),
                    _ => Err(anyhow!("list_max: expected a list")),
                }
            }
            "list_avg" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_avg expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) if !l.is_empty() => {
                        let nums: Vec<f64> = l
                            .iter()
                            .filter_map(|v| {
                                if let Value::Number(n) = v {
                                    Some(*n)
                                } else {
                                    None
                                }
                            })
                            .collect();
                        if nums.is_empty() {
                            return Err(anyhow!("list_avg: no numeric elements"));
                        }
                        Ok(Value::Number(nums.iter().sum::<f64>() / nums.len() as f64))
                    }
                    Value::List(_) => Err(anyhow!("list_avg: empty list")),
                    _ => Err(anyhow!("list_avg: expected a list")),
                }
            }
            "list_contains" | "LIST_CONTAINS" => {
                if args.len() != 2 {
                    return Err(anyhow!("list_contains expects 2 arguments (list, value)"));
                }
                match &args[0] {
                    Value::List(l) => {
                        let found = l.iter().any(|v| v == &args[1]);
                        Ok(Value::Bool(found))
                    }
                    _ => Err(anyhow!("list_contains: first argument must be a list")),
                }
            }
            "list_take" => {
                if args.len() != 2 {
                    return Err(anyhow!("list_take expects 2 arguments (list, count)"));
                }
                let n = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("list_take: count must be a number")),
                };
                match &args[0] {
                    Value::List(l) => Ok(Value::List(l.iter().take(n).cloned().collect())),
                    _ => Err(anyhow!("list_take: expected a list")),
                }
            }
            "list_drop" => {
                if args.len() != 2 {
                    return Err(anyhow!("list_drop expects 2 arguments (list, count)"));
                }
                let n = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("list_drop: count must be a number")),
                };
                match &args[0] {
                    Value::List(l) => Ok(Value::List(l.iter().skip(n).cloned().collect())),
                    _ => Err(anyhow!("list_drop: expected a list")),
                }
            }
            "list_slice" => {
                if args.len() != 3 {
                    return Err(anyhow!("list_slice expects 3 arguments (list, start, end)"));
                }
                let start = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("list_slice: start must be a number")),
                };
                let end = match &args[2] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("list_slice: end must be a number")),
                };
                match &args[0] {
                    Value::List(l) => {
                        let e = end.min(l.len());
                        let s = start.min(e);
                        Ok(Value::List(l[s..e].to_vec()))
                    }
                    _ => Err(anyhow!("list_slice: expected a list")),
                }
            }
            "list_concat" => {
                if args.len() != 2 {
                    return Err(anyhow!("list_concat expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let mut out = a.clone();
                        out.extend(b.iter().cloned());
                        Ok(Value::List(out))
                    }
                    _ => Err(anyhow!("list_concat: both arguments must be lists")),
                }
            }
            "list_flatten" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_flatten expects 1 argument"));
                }
                let outer_val = self.deref(args[0].clone());
                match outer_val {
                    Value::List(outer) => {
                        let mut flat: Vec<Value> = Vec::new();
                        for item in outer {
                            let resolved = self.deref(item);
                            match resolved {
                                Value::List(inner) => {
                                    for ii in inner {
                                        flat.push(self.deref(ii));
                                    }
                                }
                                other => flat.push(other),
                            }
                        }
                        Ok(Value::List(flat))
                    }
                    _ => Err(anyhow!("list_flatten: expected a list")),
                }
            }
            "list_sum" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_sum expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => Ok(Value::Number(l.iter().fold(0.0f64, |acc, v| match v {
                        Value::Number(n) => acc + n,
                        Value::Bool(b) => {
                            if *b {
                                acc + 1.0
                            } else {
                                acc
                            }
                        }
                        _ => acc,
                    }))),
                    _ => Err(anyhow!("list_sum: expected a list")),
                }
            }
            "list_average" => {
                if args.len() != 1 {
                    return Err(anyhow!("list_average expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => {
                        if l.is_empty() {
                            return Ok(Value::Number(0.0));
                        }
                        let mut total = 0.0f64;
                        for v in l {
                            match v {
                                Value::Number(n) => total += n,
                                _ => {
                                    return Err(anyhow!(
                                        "list_average: all elements must be numbers"
                                    ))
                                }
                            }
                        }
                        Ok(Value::Number(total / l.len() as f64))
                    }
                    _ => Err(anyhow!("list_average: expected a list")),
                }
            }
            "not_null" => {
                if args.len() != 1 {
                    return Err(anyhow!("not_null expects 1 argument"));
                }
                Ok(Value::Bool(!matches!(&args[0], Value::None)))
            }
            "identity" => {
                if args.len() != 1 {
                    return Err(anyhow!("identity expects 1 argument"));
                }
                Ok(args[0].clone())
            }
            "bool" => {
                if args.len() != 1 {
                    return Err(anyhow!("bool() takes exactly one argument"));
                }
                Ok(Value::Bool(self.value_is_truthy(&args[0])))
            }

            // ── Validation ────────────────────────────────────────────────────
            "validate_not_empty" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "validate_not_empty expects 2 arguments (value, name)"
                    ));
                }
                let is_empty = match &args[0] {
                    Value::List(l) => l.is_empty(),
                    Value::String(s) => s.is_empty(),
                    Value::None => true,
                    _ => false,
                };
                Ok(Value::Bool(!is_empty))
            }
            "validate_is_number" => {
                if args.len() != 2 {
                    return Err(anyhow!("validate_is_number expects 2 arguments"));
                }
                Ok(Value::Bool(matches!(&args[0], Value::Number(_))))
            }
            "validate_is_string" => {
                if args.len() != 2 {
                    return Err(anyhow!("validate_is_string expects 2 arguments"));
                }
                Ok(Value::Bool(matches!(&args[0], Value::String(_))))
            }
            "validate_range" => {
                if args.len() != 4 {
                    return Err(anyhow!(
                        "validate_range expects 4 arguments (value, min, max, name)"
                    ));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Number(v), Value::Number(lo), Value::Number(hi)) => {
                        Ok(Value::Bool(*v >= *lo && *v <= *hi))
                    }
                    _ => Err(anyhow!("validate_range: first 3 arguments must be numbers")),
                }
            }
            "validate_length" => {
                if args.len() != 4 {
                    return Err(anyhow!("validate_length expects 4 arguments"));
                }
                let actual = match &args[0] {
                    Value::List(l) => l.len() as f64,
                    Value::String(s) => s.chars().count() as f64,
                    _ => {
                        return Err(anyhow!(
                            "validate_length: first argument must be list or string"
                        ))
                    }
                };
                match (&args[1], &args[2]) {
                    (Value::Number(lo), Value::Number(hi)) => {
                        Ok(Value::Bool(actual >= *lo && actual <= *hi))
                    }
                    _ => Err(anyhow!(
                        "validate_length: min_len and max_len must be numbers"
                    )),
                }
            }

            // ── Collection ────────────────────────────────────────────────────
            "collection_empty" => Ok(Value::List(vec![])),
            "collection_single" => {
                if args.len() != 1 {
                    return Err(anyhow!("collection_single expects 1 argument"));
                }
                Ok(Value::List(vec![args[0].clone()]))
            }
            "collection_pair" => {
                if args.len() != 2 {
                    return Err(anyhow!("collection_pair expects 2 arguments"));
                }
                Ok(Value::List(vec![args[0].clone(), args[1].clone()]))
            }
            "collection_triple" => {
                if args.len() != 3 {
                    return Err(anyhow!("collection_triple expects 3 arguments"));
                }
                Ok(Value::List(vec![
                    args[0].clone(),
                    args[1].clone(),
                    args[2].clone(),
                ]))
            }
            "collection_fill" => {
                if args.len() != 2 {
                    return Err(anyhow!("collection_fill expects 2 arguments (value, size)"));
                }
                let n = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("collection_fill: size must be a number")),
                };
                Ok(Value::List(
                    std::iter::repeat_n(args[0].clone(), n).collect(),
                ))
            }
            "collection_merge" => {
                if args.len() != 2 {
                    return Err(anyhow!("collection_merge expects 2 arguments"));
                }
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
                if args.len() != 2 {
                    return Err(anyhow!("collection_merge_unique expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => {
                        let mut out = a.clone();
                        for v in b {
                            if !out.contains(v) {
                                out.push(v.clone());
                            }
                        }
                        Ok(Value::List(out))
                    }
                    _ => Err(anyhow!(
                        "collection_merge_unique: both arguments must be lists"
                    )),
                }
            }
            "collection_zip" => {
                if args.len() != 2 {
                    return Err(anyhow!("collection_zip expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::List(a), Value::List(b)) => Ok(Value::List(
                        a.iter()
                            .zip(b.iter())
                            .map(|(x, y)| Value::List(vec![x.clone(), y.clone()]))
                            .collect(),
                    )),
                    _ => Err(anyhow!("collection_zip: both arguments must be lists")),
                }
            }

            // ── Dir / File stdlib ─────────────────────────────────────────────
            "dir_get_current" => Ok(Value::String(self.shell.pwd())),
            "dir_exists" => {
                if args.len() != 1 {
                    return Err(anyhow!("dir_exists expects 1 argument"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::Bool(self.shell.is_dir(p))),
                    _ => Err(anyhow!("dir_exists: path must be string")),
                }
            }
            "dir_create" => {
                if args.is_empty() {
                    return Err(anyhow!("dir_create expects 1 argument"));
                }
                let p = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("dir_create: path must be string")),
                };
                let parents = matches!(args.get(1), Some(Value::Bool(true)));
                self.shell.mkdir(&p, parents).map(Value::String)
            }
            "dir_list" => {
                if args.len() != 1 {
                    return Err(anyhow!("dir_list expects 1 argument"));
                }
                match &args[0] {
                    Value::String(p) => self.shell.ls(Some(p)).map(Value::List),
                    _ => Err(anyhow!("dir_list: path must be string")),
                }
            }
            "dir_delete" => {
                if args.len() != 1 {
                    return Err(anyhow!("dir_delete expects 1 argument"));
                }
                match &args[0] {
                    Value::String(p) => self.shell.rmdir(p).map(Value::String),
                    _ => Err(anyhow!("dir_delete: path must be string")),
                }
            }
            "file_exists" => {
                if args.len() != 1 {
                    return Err(anyhow!("file_exists expects 1 argument"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::Bool(self.shell.exists(p))),
                    _ => Err(anyhow!("file_exists: path must be string")),
                }
            }
            "file_write" => {
                if args.len() != 2 {
                    return Err(anyhow!("file_write expects 2 arguments (path, data)"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("file_write: path must be string")),
                };
                let data = match &args[1] {
                    Value::String(s) => s.as_bytes().to_vec(),
                    Value::List(items) => items
                        .iter()
                        .map(|v| match v {
                            Value::Number(n) => Ok((n.round() as u32).min(255) as u8),
                            _ => Err(anyhow!("file_write: list must contain numbers")),
                        })
                        .collect::<Result<Vec<_>>>()?,
                    _ => return Err(anyhow!("file_write: data must be string or list")),
                };
                std::fs::write(&path, &data).map_err(|e| anyhow!("file_write: {}", e))?;
                Ok(Value::String(format!(
                    "Wrote {} bytes to {}",
                    data.len(),
                    path
                )))
            }
            "file_size" => {
                if args.len() != 1 {
                    return Err(anyhow!("file_size expects 1 argument"));
                }
                match &args[0] {
                    Value::String(p) => self.shell.file_size(p).map(|s| Value::Number(s as f64)),
                    _ => Err(anyhow!("file_size: path must be string")),
                }
            }
            "file_delete" => {
                if args.len() != 1 {
                    return Err(anyhow!("file_delete expects 1 argument"));
                }
                match &args[0] {
                    Value::String(p) => self.shell.rm(p).map(Value::String),
                    _ => Err(anyhow!("file_delete: path must be string")),
                }
            }
            "file_read" => {
                if args.len() != 1 {
                    return Err(anyhow!("file_read expects 1 argument"));
                }
                match &args[0] {
                    Value::String(path) => std::fs::read(path)
                        .map(|bytes| {
                            Value::List(bytes.iter().map(|&b| Value::Number(b as f64)).collect())
                        })
                        .map_err(|e| anyhow!("file_read: {}", e)),
                    _ => Err(anyhow!("file_read: path must be string")),
                }
            }

            // ── Comprehensive File I/O ─────────────────────────────────────────

            // READ_FILE(path) → String
            "read_file" | "READ_FILE" => {
                if args.len() != 1 {
                    return Err(anyhow!("READ_FILE expects 1 argument: path"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("READ_FILE: path must be a string")),
                };
                std::fs::read_to_string(&path)
                    .map(Value::String)
                    .map_err(|e| anyhow!("READ_FILE \"{}\": {}", path, e))
            }

            // WRITE_FILE(path, content) — always overwrites (Unix-style)
            "write_file" | "WRITE_FILE" => {
                if args.len() != 2 {
                    return Err(anyhow!("WRITE_FILE expects 2 arguments: path, content"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("WRITE_FILE: path must be a string")),
                };
                let content = Executor::value_to_string(&args[1]);
                std::fs::write(&path, &content)
                    .map_err(|e| anyhow!("WRITE_FILE \"{}\": {}", path, e))?;
                Ok(Value::None)
            }

            // APPEND_FILE(path, content) — creates if missing, never prompts
            "append_file" | "APPEND_FILE" => {
                if args.len() != 2 {
                    return Err(anyhow!("APPEND_FILE expects 2 arguments: path, content"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("APPEND_FILE: path must be a string")),
                };
                let content = Executor::value_to_string(&args[1]);
                use std::io::Write as _;
                let mut f = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&path)
                    .map_err(|e| anyhow!("APPEND_FILE \"{}\": {}", path, e))?;
                f.write_all(content.as_bytes())
                    .map_err(|e| anyhow!("APPEND_FILE \"{}\": {}", path, e))?;
                Ok(Value::None)
            }

            // READ_LINES(path) → List of strings, \n stripped
            "read_lines" | "READ_LINES" => {
                if args.len() != 1 {
                    return Err(anyhow!("READ_LINES expects 1 argument: path"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("READ_LINES: path must be a string")),
                };
                let content = std::fs::read_to_string(&path)
                    .map_err(|e| anyhow!("READ_LINES \"{}\": {}", path, e))?;
                let lines: Vec<Value> = content
                    .lines()
                    .map(|l| Value::String(l.to_string()))
                    .collect();
                Ok(Value::List(lines))
            }

            // WRITE_LINES(path, lines) — joins list with \n, always overwrites
            "write_lines" | "WRITE_LINES" => {
                if args.len() != 2 {
                    return Err(anyhow!("WRITE_LINES expects 2 arguments: path, lines"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("WRITE_LINES: path must be a string")),
                };
                let lines = match &args[1] {
                    Value::List(v) => v
                        .iter()
                        .map(|x| Executor::value_to_string(x))
                        .collect::<Vec<_>>()
                        .join("\n"),
                    _ => return Err(anyhow!("WRITE_LINES: second argument must be a list")),
                };
                std::fs::write(&path, lines + "\n")
                    .map_err(|e| anyhow!("WRITE_LINES \"{}\": {}", path, e))?;
                Ok(Value::None)
            }

            // FILE_EXISTS(path) → Bool  (friendly uppercase alias)
            "FILE_EXISTS" => {
                if args.len() != 1 {
                    return Err(anyhow!("FILE_EXISTS expects 1 argument: path"));
                }
                match &args[0] {
                    Value::String(p) => Ok(Value::Bool(std::path::Path::new(p).exists())),
                    _ => Err(anyhow!("FILE_EXISTS: path must be a string")),
                }
            }

            // DELETE_FILE(path)
            "delete_file" | "DELETE_FILE" => {
                if args.len() != 1 {
                    return Err(anyhow!("DELETE_FILE expects 1 argument: path"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("DELETE_FILE: path must be a string")),
                };
                std::fs::remove_file(&path)
                    .map_err(|e| anyhow!("DELETE_FILE \"{}\": {}", path, e))?;
                Ok(Value::None)
            }

            // RENAME_FILE(src, dst)
            "rename_file" | "RENAME_FILE" => {
                if args.len() != 2 {
                    return Err(anyhow!("RENAME_FILE expects 2 arguments: src, dst"));
                }
                let src = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("RENAME_FILE: src must be a string")),
                };
                let dst = match &args[1] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("RENAME_FILE: dst must be a string")),
                };
                std::fs::rename(&src, &dst)
                    .map_err(|e| anyhow!("RENAME_FILE \"{}\" → \"{}\": {}", src, dst, e))?;
                Ok(Value::None)
            }

            // COPY_FILE(src, dst) — always overwrites dst if it exists
            "copy_file" | "COPY_FILE" => {
                if args.len() != 2 {
                    return Err(anyhow!("COPY_FILE expects 2 arguments: src, dst"));
                }
                let src = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("COPY_FILE: src must be a string")),
                };
                let dst = match &args[1] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("COPY_FILE: dst must be a string")),
                };
                std::fs::copy(&src, &dst)
                    .map_err(|e| anyhow!("COPY_FILE \"{}\" → \"{}\": {}", src, dst, e))?;
                Ok(Value::None)
            }

            // FILE_SIZE(path) → Number (bytes)
            "FILE_SIZE" => {
                if args.len() != 1 {
                    return Err(anyhow!("FILE_SIZE expects 1 argument: path"));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("FILE_SIZE: path must be a string")),
                };
                let meta = std::fs::metadata(&path)
                    .map_err(|e| anyhow!("FILE_SIZE \"{}\": {}", path, e))?;
                Ok(Value::Number(meta.len() as f64))
            }

            // ── File Handle API ───────────────────────────────────────────────

            // OPEN(path, mode) → handle string   mode: "r" | "w" | "a"
            "open" | "OPEN" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "OPEN expects 2 arguments: path, mode (\"r\", \"w\", or \"a\")"
                    ));
                }
                let path = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("OPEN: path must be a string")),
                };
                let mode = match &args[1] {
                    Value::String(s) => s.clone(),
                    _ => {
                        return Err(anyhow!(
                            "OPEN: mode must be a string (\"r\", \"w\", or \"a\")"
                        ))
                    }
                };
                let handle = format!("file://{}", self.next_file_id);
                self.next_file_id += 1;
                match mode.as_str() {
                    "r" => {
                        let f = std::fs::File::open(&path)
                            .map_err(|e| anyhow!("OPEN \"{}\": {}", path, e))?;
                        self.open_files
                            .insert(handle.clone(), OpenFile::Reader(std::io::BufReader::new(f)));
                    }
                    "w" => {
                        let f = std::fs::File::create(&path)
                            .map_err(|e| anyhow!("OPEN \"{}\": {}", path, e))?;
                        self.open_files
                            .insert(handle.clone(), OpenFile::Writer(std::io::BufWriter::new(f)));
                    }
                    "a" => {
                        let f = std::fs::OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&path)
                            .map_err(|e| anyhow!("OPEN \"{}\": {}", path, e))?;
                        self.open_files.insert(
                            handle.clone(),
                            OpenFile::Appender(std::io::BufWriter::new(f)),
                        );
                    }
                    _ => {
                        return Err(anyhow!(
                            "OPEN: unknown mode \"{}\". Use \"r\", \"w\", or \"a\"",
                            mode
                        ))
                    }
                }
                Ok(Value::String(handle))
            }

            // READLINE(handle) → String or none at EOF
            "readline" | "READLINE" => {
                if args.len() != 1 {
                    return Err(anyhow!("READLINE expects 1 argument: file handle"));
                }
                let handle = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("READLINE: handle must be a string")),
                };
                use std::io::BufRead;
                match self.open_files.get_mut(&handle) {
                    Some(OpenFile::Reader(r)) => {
                        let mut line = String::new();
                        let n = r
                            .read_line(&mut line)
                            .map_err(|e| anyhow!("READLINE: {}", e))?;
                        if n == 0 {
                            return Ok(Value::None);
                        }
                        if line.ends_with('\n') {
                            line.pop();
                            if line.ends_with('\r') {
                                line.pop();
                            }
                        }
                        Ok(Value::String(line))
                    }
                    Some(_) => Err(anyhow!(
                        "READLINE: handle \"{}\" was not opened for reading",
                        handle
                    )),
                    None => Err(anyhow!("READLINE: unknown handle \"{}\"", handle)),
                }
            }

            // WRITELINE(handle, text) — writes text + newline
            "writeline" | "WRITELINE" => {
                if args.len() != 2 {
                    return Err(anyhow!("WRITELINE expects 2 arguments: handle, text"));
                }
                let handle = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("WRITELINE: handle must be a string")),
                };
                let text = Executor::value_to_string(&args[1]);
                use std::io::Write as _;
                match self.open_files.get_mut(&handle) {
                    Some(OpenFile::Writer(w)) => {
                        write!(w, "{}\n", text).map_err(|e| anyhow!("WRITELINE: {}", e))?;
                        w.flush().map_err(|e| anyhow!("WRITELINE flush: {}", e))?;
                        Ok(Value::None)
                    }
                    Some(OpenFile::Appender(w)) => {
                        write!(w, "{}\n", text).map_err(|e| anyhow!("WRITELINE: {}", e))?;
                        w.flush().map_err(|e| anyhow!("WRITELINE flush: {}", e))?;
                        Ok(Value::None)
                    }
                    Some(_) => Err(anyhow!(
                        "WRITELINE: handle \"{}\" was not opened for writing",
                        handle
                    )),
                    None => Err(anyhow!("WRITELINE: unknown handle \"{}\"", handle)),
                }
            }

            // READ_ALL(handle) → String — reads remaining content
            "read_all" | "READ_ALL" => {
                if args.len() != 1 {
                    return Err(anyhow!("READ_ALL expects 1 argument: file handle"));
                }
                let handle = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("READ_ALL: handle must be a string")),
                };
                use std::io::Read;
                match self.open_files.get_mut(&handle) {
                    Some(OpenFile::Reader(r)) => {
                        let mut buf = String::new();
                        r.read_to_string(&mut buf)
                            .map_err(|e| anyhow!("READ_ALL: {}", e))?;
                        Ok(Value::String(buf))
                    }
                    Some(_) => Err(anyhow!(
                        "READ_ALL: handle \"{}\" was not opened for reading",
                        handle
                    )),
                    None => Err(anyhow!("READ_ALL: unknown handle \"{}\"", handle)),
                }
            }

            // ── Dictionaries ──────────────────────────────────────────────────

            // DICT(k1, v1, k2, v2, ...) → dict  (or DICT() for empty)
            "dict" | "DICT" => {
                if args.len() % 2 != 0 {
                    return Err(anyhow!(
                        "DICT expects an even number of arguments (key, value pairs)"
                    ));
                }
                let mut map = std::collections::HashMap::new();
                let mut i = 0;
                while i < args.len() {
                    let key = Executor::value_to_string(&args[i]);
                    let val = args[i + 1].clone();
                    map.insert(key, val);
                    i += 2;
                }
                let id = self.gc.allocate(Value::Dict(map));
                Ok(Value::Heap(id))
            }

            // dict_get(d, key) → value or Error
            "dict_get" | "DICT_GET" => {
                if args.len() != 2 {
                    return Err(anyhow!("dict_get expects 2 arguments: dict, key"));
                }
                let key = Executor::value_to_string(&args[1]);
                let d = self.deref(args[0].clone());
                match d {
                    Value::Dict(map) => map
                        .get(&key)
                        .cloned()
                        .ok_or_else(|| anyhow!("dict_get: key \"{}\" not found", key)),
                    _ => Err(anyhow!("dict_get: first argument must be a dict")),
                }
            }

            // dict_has(d, key) → bool
            "dict_has" | "DICT_HAS" => {
                if args.len() != 2 {
                    return Err(anyhow!("dict_has expects 2 arguments: dict, key"));
                }
                let key = Executor::value_to_string(&args[1]);
                let d = self.deref(args[0].clone());
                match d {
                    Value::Dict(map) => Ok(Value::Bool(map.contains_key(&key))),
                    _ => Err(anyhow!("dict_has: first argument must be a dict")),
                }
            }

            // dict_delete is handled before eager-deref (needs raw Heap handle)
            "dict_delete" | "DICT_DELETE" => Err(anyhow!("dict_delete: internal routing error")),

            // dict_keys(d) → list of keys
            "dict_keys" | "DICT_KEYS" => {
                if args.len() != 1 {
                    return Err(anyhow!("dict_keys expects 1 argument: dict"));
                }
                let d = self.deref(args[0].clone());
                match d {
                    Value::Dict(map) => {
                        let mut keys: Vec<Value> =
                            map.keys().map(|k| Value::String(k.clone())).collect();
                        keys.sort_by(|a, b| {
                            let sa = Executor::value_to_string(a);
                            let sb = Executor::value_to_string(b);
                            sa.cmp(&sb)
                        });
                        let id = self.gc.allocate(Value::List(keys));
                        Ok(Value::Heap(id))
                    }
                    _ => Err(anyhow!("dict_keys: argument must be a dict")),
                }
            }

            // dict_values(d) → list of values
            "dict_values" | "DICT_VALUES" => {
                if args.len() != 1 {
                    return Err(anyhow!("dict_values expects 1 argument: dict"));
                }
                let d = self.deref(args[0].clone());
                match d {
                    Value::Dict(map) => {
                        let mut pairs: Vec<(&String, &Value)> = map.iter().collect();
                        pairs.sort_by_key(|(k, _)| k.as_str());
                        let vals: Vec<Value> = pairs.into_iter().map(|(_, v)| v.clone()).collect();
                        let id = self.gc.allocate(Value::List(vals));
                        Ok(Value::Heap(id))
                    }
                    _ => Err(anyhow!("dict_values: argument must be a dict")),
                }
            }

            // dict_items(d) → list of [key, val] pairs
            "dict_items" | "DICT_ITEMS" => {
                if args.len() != 1 {
                    return Err(anyhow!("dict_items expects 1 argument: dict"));
                }
                let d = self.deref(args[0].clone());
                match d {
                    Value::Dict(map) => {
                        let mut pairs: Vec<(&String, &Value)> = map.iter().collect();
                        pairs.sort_by_key(|(k, _)| k.as_str());
                        let items: Vec<Value> = pairs
                            .into_iter()
                            .map(|(k, v)| {
                                let pair_id = self.gc.allocate(Value::List(vec![
                                    Value::String(k.clone()),
                                    v.clone(),
                                ]));
                                Value::Heap(pair_id)
                            })
                            .collect();
                        let id = self.gc.allocate(Value::List(items));
                        Ok(Value::Heap(id))
                    }
                    _ => Err(anyhow!("dict_items: argument must be a dict")),
                }
            }

            // dict_len(d) → number
            "dict_len" | "DICT_LEN" => {
                if args.len() != 1 {
                    return Err(anyhow!("dict_len expects 1 argument: dict"));
                }
                let d = self.deref(args[0].clone());
                match d {
                    Value::Dict(map) => Ok(Value::Number(map.len() as f64)),
                    _ => Err(anyhow!("dict_len: argument must be a dict")),
                }
            }

            // ── User Input ────────────────────────────────────────────────────

            // INPUT(prompt?) → string — reads a line from stdin
            "input" | "INPUT" => {
                use std::io::{self, Write as _};
                if args.len() > 1 {
                    return Err(anyhow!("INPUT expects 0 or 1 argument (optional prompt)"));
                }
                if let Some(prompt) = args.first() {
                    print!("{}", Executor::value_to_string(prompt));
                    io::stdout().flush().ok();
                }
                let mut line = String::new();
                io::stdin()
                    .read_line(&mut line)
                    .map_err(|e| anyhow!("INPUT: {}", e))?;
                Ok(Value::String(
                    line.trim_end_matches('\n')
                        .trim_end_matches('\r')
                        .to_string(),
                ))
            }

            // GETENV(name) → string or none
            "getenv" | "GETENV" => {
                if args.len() != 1 {
                    return Err(anyhow!("GETENV expects 1 argument: variable name"));
                }
                let name = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("GETENV: name must be a string")),
                };
                Ok(std::env::var(&name)
                    .map(Value::String)
                    .unwrap_or(Value::None))
            }

            // SETENV(name, value) → none
            "setenv" | "SETENV" => {
                if args.len() != 2 {
                    return Err(anyhow!("SETENV expects 2 arguments: name, value"));
                }
                let name = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("SETENV: name must be a string")),
                };
                let value = Executor::value_to_string(&args[1]);
                std::env::set_var(&name, &value);
                Ok(Value::None)
            }

            // ARGV() → list of command-line arguments
            "argv" | "ARGV" => {
                let argv: Vec<Value> = std::env::args().map(Value::String).collect();
                let id = self.gc.allocate(Value::List(argv));
                Ok(Value::Heap(id))
            }

            // ── Formatting ────────────────────────────────────────────────────
            "format_number" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "format_number expects 2 arguments (number, decimals)"
                    ));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(n), Value::Number(d)) => {
                        Ok(Value::String(format!("{:.prec$}", n, prec = *d as usize)))
                    }
                    _ => Err(anyhow!("format_number: expected (number, number)")),
                }
            }
            "format_currency" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "format_currency expects 2 arguments (amount, symbol)"
                    ));
                }
                let sym = Executor::value_to_string(&args[1]);
                match &args[0] {
                    Value::Number(n) => Ok(Value::String(format!("{}{:.2}", sym, n))),
                    _ => Err(anyhow!("format_currency: amount must be a number")),
                }
            }
            "format_percentage" => {
                if args.len() != 1 {
                    return Err(anyhow!("format_percentage expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::String(format!("{:.1}%", n))),
                    _ => Err(anyhow!("format_percentage: expected number")),
                }
            }
            "format_bytes" => {
                if args.len() != 1 {
                    return Err(anyhow!("format_bytes expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => {
                        let b = n.round() as u64;
                        let s = if b < 1024 {
                            format!("{} B", b)
                        } else if b < 1024 * 1024 {
                            format!("{:.1} KB", b as f64 / 1024.0)
                        } else if b < 1024 * 1024 * 1024 {
                            format!("{:.1} MB", b as f64 / 1_048_576.0)
                        } else {
                            format!("{:.1} GB", b as f64 / 1_073_741_824.0)
                        };
                        Ok(Value::String(s))
                    }
                    _ => Err(anyhow!("format_bytes: expected number")),
                }
            }

            // ── JSON ──────────────────────────────────────────────────────────

            // JSON_PARSE(str) → Value (dict, list, number, string, bool, none)
            "json_parse" | "JSON_PARSE" => {
                if args.len() != 1 {
                    return Err(anyhow!("JSON_PARSE expects 1 argument: json string"));
                }
                let s = match &args[0] {
                    Value::String(s) => s.clone(),
                    _ => return Err(anyhow!("JSON_PARSE: argument must be a string")),
                };
                let jv: serde_json::Value =
                    serde_json::from_str(&s).map_err(|e| anyhow!("JSON_PARSE: {}", e))?;
                Ok(Self::json_to_value(&mut self.gc, &jv))
            }

            // JSON_STRINGIFY(value) → string
            "json_stringify" | "JSON_STRINGIFY" => {
                if args.len() < 1 || args.len() > 2 {
                    return Err(anyhow!(
                        "JSON_STRINGIFY expects 1 or 2 arguments: value [, pretty]"
                    ));
                }
                let pretty = args
                    .get(1)
                    .map(|v| self.value_is_truthy(v))
                    .unwrap_or(false);
                let jv = Self::value_to_json(&self.gc, &args[0]);
                let out = if pretty {
                    serde_json::to_string_pretty(&jv)
                        .map_err(|e| anyhow!("JSON_STRINGIFY: {}", e))?
                } else {
                    serde_json::to_string(&jv).map_err(|e| anyhow!("JSON_STRINGIFY: {}", e))?
                };
                Ok(Value::String(out))
            }

            // ── Tensor stdlib aliases ─────────────────────────────────────────
            "tensor_create_zeros" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "tensor_create_zeros expects 2 arguments (rows, cols)"
                    ));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(r), Value::Number(c)) => {
                        let (rows, cols) = (*r as usize, *c as usize);
                        Ok(Value::Tensor(RuntimeTensor::new(
                            vec![rows, cols],
                            "float32".to_string(),
                            vec![0.0; rows * cols],
                        )))
                    }
                    _ => Err(anyhow!("tensor_create_zeros: expected two numbers")),
                }
            }
            "tensor_create_ones" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "tensor_create_ones expects 2 arguments (rows, cols)"
                    ));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(r), Value::Number(c)) => {
                        let (rows, cols) = (*r as usize, *c as usize);
                        Ok(Value::Tensor(RuntimeTensor::new(
                            vec![rows, cols],
                            "float32".to_string(),
                            vec![1.0; rows * cols],
                        )))
                    }
                    _ => Err(anyhow!("tensor_create_ones: expected two numbers")),
                }
            }
            "tensor_create_identity" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor_create_identity expects 1 argument (n)"));
                }
                match &args[0] {
                    Value::Number(n) => {
                        let n = n.round() as usize;
                        let mut data = vec![0.0f64; n * n];
                        for i in 0..n {
                            data[i * n + i] = 1.0;
                        }
                        Ok(Value::Tensor(RuntimeTensor::new(
                            vec![n, n],
                            "float32".to_string(),
                            data,
                        )))
                    }
                    _ => Err(anyhow!("tensor_create_identity: expected number")),
                }
            }
            "tensor_get_shape" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor_get_shape expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::List(
                        t.shape.iter().map(|&s| Value::Number(s as f64)).collect(),
                    )),
                    _ => Err(anyhow!("tensor_get_shape: expected a tensor")),
                }
            }
            "tensor_get_dtype" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor_get_dtype expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::String(t.dtype.clone())),
                    _ => Err(anyhow!("tensor_get_dtype: expected a tensor")),
                }
            }
            "tensor_sum_all" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor_sum_all expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => Ok(Value::Number(t.data.iter().copied().sum())),
                    _ => Err(anyhow!("tensor_sum_all: expected a tensor")),
                }
            }
            "tensor_mean_all" => {
                if args.len() != 1 {
                    return Err(anyhow!("tensor_mean_all expects 1 argument"));
                }
                match &args[0] {
                    Value::Tensor(t) => {
                        let s: f64 = t.data.iter().copied().sum();
                        let n = t.data.len() as f64;
                        Ok(Value::Number(if n == 0.0 { 0.0 } else { s / n }))
                    }
                    _ => Err(anyhow!("tensor_mean_all: expected a tensor")),
                }
            }

            // ── Logging ───────────────────────────────────────────────────────
            "log_info" => {
                if args.len() != 1 {
                    return Err(anyhow!("log_info expects 1 argument"));
                }
                println!("[INFO] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }
            "log_warning" | "log_warn" => {
                if args.len() != 1 {
                    return Err(anyhow!("log_warning expects 1 argument"));
                }
                eprintln!("[WARN] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }
            // Raise a runtime error with an optional message
            "error" | "raise" => {
                let msg = if args.is_empty() {
                    "error".to_string()
                } else {
                    Executor::value_to_string(&args[0])
                };
                Err(anyhow!("{}", msg))
            }
            "log_error" => {
                if args.len() != 1 {
                    return Err(anyhow!("log_error expects 1 argument"));
                }
                eprintln!("[ERROR] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }
            "log_debug" => {
                if args.len() != 1 {
                    return Err(anyhow!("log_debug expects 1 argument"));
                }
                println!("[DEBUG] {}", Executor::value_to_string(&args[0]));
                Ok(Value::None)
            }
            "debug_print_type" => {
                if args.len() != 1 {
                    return Err(anyhow!("debug_print_type expects 1 argument"));
                }
                let t = match &args[0] {
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Bool(_) => "bool",
                    Value::List(_) => "list",
                    Value::Tensor(_) => "tensor",
                    Value::Lambda(_, _, _) => "lambda",
                    Value::LazyImport { .. } => "lazy",
                    Value::None => "none",
                    Value::Heap(_) => "heap",
                    Value::Dict(_) => "dict",
                    Value::Pending(_, _) => "pending",
                    Value::Pointer(_) => "pointer",
                    Value::FamilyNode { .. } => "family_node",
                    Value::Builtin(_) => "builtin",
                };
                println!(
                    "[DEBUG] type({}) = {}",
                    Executor::value_to_string(&args[0]),
                    t
                );
                Ok(Value::None)
            }
            "debug_print_length" => {
                if args.len() != 1 {
                    return Err(anyhow!("debug_print_length expects 1 argument"));
                }
                let len = match &args[0] {
                    Value::List(l) => l.len(),
                    Value::String(s) => s.chars().count(),
                    _ => return Err(anyhow!("debug_print_length: expected list or string")),
                };
                println!("[DEBUG] length = {}", len);
                Ok(Value::None)
            }
            "debug_print_value" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "debug_print_value expects 2 arguments (name, value)"
                    ));
                }
                println!(
                    "[DEBUG] {} = {}",
                    Executor::value_to_string(&args[0]),
                    Executor::value_to_string(&args[1])
                );
                Ok(Value::None)
            }

            // ── Functional ────────────────────────────────────────────────────
            "pipe" => Err(anyhow!("call_value_with_one_arg is not implemented")),
            "partial" => {
                if args.len() != 2 {
                    return Err(anyhow!("partial expects 2 arguments (fn, arg)"));
                }
                Ok(Value::List(vec![
                    Value::String("__partial__".to_string()),
                    args[0].clone(),
                    args[1].clone(),
                ]))
            }

            // ── Data Processing ───────────────────────────────────────────────
            "batch_split" => {
                if args.len() != 2 {
                    return Err(anyhow!(
                        "batch_split expects 2 arguments (list, batch_size)"
                    ));
                }
                let batch_size = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("batch_split: batch_size must be a number")),
                };
                if batch_size == 0 {
                    return Err(anyhow!("batch_split: batch_size must be > 0"));
                }
                match &args[0] {
                    Value::List(l) => Ok(Value::List(
                        l.chunks(batch_size)
                            .map(|c| Value::List(c.to_vec()))
                            .collect(),
                    )),
                    _ => Err(anyhow!("batch_split: first argument must be a list")),
                }
            }
            "distinct_values" => {
                if args.len() != 1 {
                    return Err(anyhow!("distinct_values expects 1 argument"));
                }
                match &args[0] {
                    Value::List(l) => {
                        let mut seen: Vec<Value> = Vec::new();
                        for v in l {
                            if !seen.contains(v) {
                                seen.push(v.clone());
                            }
                        }
                        Ok(Value::List(seen))
                    }
                    _ => Err(anyhow!("distinct_values: expected a list")),
                }
            }

            // ── Concurrency ───────────────────────────────────────────────────
            "priority_critical_chain" => {
                self.priorities.add_edge("critical", "important");
                self.priorities.add_edge("important", "normal");
                Ok(Value::None)
            }
            "concurrent_task" => {
                if args.len() != 2 {
                    return Err(anyhow!("concurrent_task expects 2 arguments (name, count)"));
                }
                let name = Executor::value_to_string(&args[0]);
                let count = match &args[1] {
                    Value::Number(n) => n.round() as usize,
                    _ => return Err(anyhow!("concurrent_task: count must be a number")),
                };
                for i in 0..count {
                    println!("[Task {}] iteration {}", name, i);
                }
                Ok(Value::None)
            }

            // ── print/println/echo as function calls ──────────────────────────
            "print" | "println" | "echo" => {
                if args.is_empty() {
                    println!();
                } else {
                    let parts: Vec<String> = args
                        .iter()
                        .map(|v| {
                            let resolved = self.deref(v.clone());
                            match &resolved {
                                Value::List(items) => {
                                    let inner: Vec<String> = items
                                        .iter()
                                        .map(|i| Executor::value_to_string(&self.deref(i.clone())))
                                        .collect();
                                    format!("[{}]", inner.join(", "))
                                }
                                other => Executor::value_to_string(other),
                            }
                        })
                        .collect();
                    println!("{}", parts.join(" "));
                }
                Ok(Value::None)
            }

            // ── System builtins ───────────────────────────────────────────────
            "exit" => {
                let code = match args.first() {
                    Some(Value::Number(n)) => *n as i32,
                    _ => 0,
                };
                std::process::exit(code);
            }
            "sleep" => {
                // SLEEP that polls X11 events to keep input responsive
                if args.len() != 1 {
                    return Err(anyhow!("sleep expects 1 argument (ms)"));
                }
                match &args[0] {
                    Value::Number(ms) => {
                        let total_ms = *ms as u64;
                        let chunk_ms = 10u64; // Poll every 10ms
                        let mut remaining = total_ms;

                        while remaining > 0 {
                            let sleep_time = remaining.min(chunk_ms);
                            std::thread::sleep(std::time::Duration::from_millis(sleep_time));
                            remaining = remaining.saturating_sub(sleep_time);

                            // Poll all X11 windows to process events during sleep
                            #[cfg(feature = "x11")]
                            {
                                for xwin in self.x11_windows.values_mut() {
                                    xwin.poll();
                                }
                            }
                        }
                        Ok(Value::None)
                    }
                    _ => Err(anyhow!("sleep: expected number (milliseconds)")),
                }
            }
            "time" => {
                let secs = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64();
                Ok(Value::Number(secs))
            }
            "time_ms" => {
                let ms = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as f64;
                Ok(Value::Number(ms))
            }
            "env" => {
                if args.len() != 1 {
                    return Err(anyhow!("env expects 1 argument (name)"));
                }
                match &args[0] {
                    Value::String(k) => Ok(Value::String(std::env::var(k).unwrap_or_default())),
                    _ => Err(anyhow!("env: expected string argument")),
                }
            }

            // ── rand aliases ──────────────────────────────────────────────────
            "rand" => {
                // RAND() -> float 0.0-1.0
                // RAND(n) -> int 0 to n-1 (like Python's random.randint(0, n-1))
                if args.is_empty() {
                    Ok(Value::Number(
                        self.rng.next_u64() as f64 / (u64::MAX as f64 + 1.0),
                    ))
                } else if args.len() == 1 {
                    match &args[0] {
                        Value::Number(n) => {
                            let max = if *n <= 0.0 { 1 } else { *n as u64 };
                            Ok(Value::Number((self.rng.next_u64() % max) as f64))
                        }
                        _ => Err(anyhow!("rand: expected numeric argument")),
                    }
                } else {
                    Err(anyhow!("rand: expected 0 or 1 arguments"))
                }
            }
            "rand_int" => {
                if args.len() != 2 {
                    return Err(anyhow!("rand_int expects 2 arguments (lo, hi)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => {
                        let (lo, hi) = (*a as i64, *b as i64);
                        if hi < lo {
                            return Err(anyhow!("rand_int: hi must be >= lo"));
                        }
                        Ok(Value::Number(
                            (lo + (self.rng.next_u64() % (hi - lo + 1) as u64) as i64) as f64,
                        ))
                    }
                    _ => Err(anyhow!("rand_int: expected two numbers")),
                }
            }
            "rand_range" => {
                if args.len() != 2 {
                    return Err(anyhow!("rand_range expects 2 arguments (lo, hi)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => {
                        let f = self.rng.next_u64() as f64 / (u64::MAX as f64 + 1.0);
                        Ok(Value::Number(a + f * (b - a)))
                    }
                    _ => Err(anyhow!("rand_range: expected two numbers")),
                }
            }

            // ── Numeric conversion ────────────────────────────────────────────
            "num" | "float" => {
                if args.len() != 1 {
                    return Err(anyhow!("num/float expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(*n)),
                    Value::String(s) => s
                        .trim()
                        .parse::<f64>()
                        .map(Value::Number)
                        .map_err(|_| anyhow!("num: cannot parse '{}' as number", s)),
                    Value::Bool(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                    _ => Err(anyhow!("num: cannot convert to number")),
                }
            }
            "int" => {
                if args.len() != 1 {
                    return Err(anyhow!("int expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.trunc())),
                    Value::String(s) => s
                        .trim()
                        .parse::<f64>()
                        .map(|n| Value::Number(n.trunc()))
                        .map_err(|_| anyhow!("int: cannot parse '{}' as number", s)),
                    Value::Bool(b) => Ok(Value::Number(if *b { 1.0 } else { 0.0 })),
                    _ => Err(anyhow!("int: cannot convert to integer")),
                }
            }

            // ── Math builtins ─────────────────────────────────────────────────
            "range" => match args.as_slice() {
                [Value::Number(n)] => {
                    let end = *n as i64;
                    Ok(Value::List(
                        (1..=end).map(|i| Value::Number(i as f64)).collect(),
                    ))
                }
                [Value::Number(start), Value::Number(end)] => {
                    let (s, e) = (*start as i64, *end as i64);
                    if s <= e {
                        Ok(Value::List(
                            (s..=e).map(|i| Value::Number(i as f64)).collect(),
                        ))
                    } else {
                        Ok(Value::List(
                            (e..=s).rev().map(|i| Value::Number(i as f64)).collect(),
                        ))
                    }
                }
                [Value::Number(start), Value::Number(end), Value::Number(step)] => {
                    let (mut cur, e, st) = (*start as i64, *end as i64, *step as i64);
                    if st == 0 {
                        return Err(anyhow!("range: step cannot be zero"));
                    }
                    let mut out = Vec::new();
                    while if st > 0 { cur <= e } else { cur >= e } {
                        out.push(Value::Number(cur as f64));
                        cur += st;
                    }
                    Ok(Value::List(out))
                }
                _ => Err(anyhow!("range() expects 1, 2, or 3 numeric arguments")),
            },
            "color" => {
                if args.len() != 3 {
                    return Err(anyhow!("color expects 3 arguments (r, g, b)"));
                }
                let r = match &args[0] {
                    Value::Number(n) => *n as u32,
                    _ => return Err(anyhow!("color: r must be a number")),
                };
                let g = match &args[1] {
                    Value::Number(n) => *n as u32,
                    _ => return Err(anyhow!("color: g must be a number")),
                };
                let b = match &args[2] {
                    Value::Number(n) => *n as u32,
                    _ => return Err(anyhow!("color: b must be a number")),
                };
                let packed = 0xFF000000u32 | ((r & 0xFF) << 16) | ((g & 0xFF) << 8) | (b & 0xFF);
                Ok(Value::Number(packed as f64))
            }
            "abs" => {
                if args.len() != 1 {
                    return Err(anyhow!("abs expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.abs())),
                    _ => Err(anyhow!("abs: expected number")),
                }
            }
            "sqrt" => {
                if args.len() != 1 {
                    return Err(anyhow!("sqrt expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.sqrt())),
                    _ => Err(anyhow!("sqrt: expected number")),
                }
            }
            "pow" => {
                if args.len() != 2 {
                    return Err(anyhow!("pow expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(b), Value::Number(e)) => Ok(Value::Number(b.powf(*e))),
                    _ => Err(anyhow!("pow: expected numbers")),
                }
            }
            "floor" => {
                if args.len() != 1 {
                    return Err(anyhow!("floor expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.floor())),
                    _ => Err(anyhow!("floor: expected number")),
                }
            }
            "ceil" => {
                if args.len() != 1 {
                    return Err(anyhow!("ceil expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.ceil())),
                    _ => Err(anyhow!("ceil: expected number")),
                }
            }
            "round" | "rnd" => {
                if args.len() != 1 {
                    return Err(anyhow!("round expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.round())),
                    _ => Err(anyhow!("round: expected number")),
                }
            }
            "min" => {
                if args.len() != 2 {
                    return Err(anyhow!("min expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.min(*b))),
                    _ => Err(anyhow!("min: expected numbers")),
                }
            }
            "max" => {
                if args.len() != 2 {
                    return Err(anyhow!("max expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.max(*b))),
                    _ => Err(anyhow!("max: expected numbers")),
                }
            }
            "clamp" => {
                if args.len() != 3 {
                    return Err(anyhow!("clamp expects 3 arguments"));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Number(v), Value::Number(lo), Value::Number(hi)) => {
                        Ok(Value::Number(v.max(*lo).min(*hi)))
                    }
                    _ => Err(anyhow!("clamp: expected numbers")),
                }
            }
            "sign" => {
                if args.len() != 1 {
                    return Err(anyhow!("sign expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(if *n > 0.0 {
                        1.0
                    } else if *n < 0.0 {
                        -1.0
                    } else {
                        0.0
                    })),
                    _ => Err(anyhow!("sign: expected number")),
                }
            }

            // ── math.* namespace — trig / advanced ────────────────────────────
            "math.sin" | "sin" => {
                if args.len() != 1 {
                    return Err(anyhow!("sin expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.sin())),
                    _ => Err(anyhow!("sin: expected number")),
                }
            }
            "math.cos" | "cos" => {
                if args.len() != 1 {
                    return Err(anyhow!("cos expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.cos())),
                    _ => Err(anyhow!("cos: expected number")),
                }
            }
            "math.tan" | "tan" => {
                if args.len() != 1 {
                    return Err(anyhow!("tan expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.tan())),
                    _ => Err(anyhow!("tan: expected number")),
                }
            }
            "math.asin" | "asin" => {
                if args.len() != 1 {
                    return Err(anyhow!("asin expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.asin())),
                    _ => Err(anyhow!("asin: expected number")),
                }
            }
            "math.acos" | "acos" => {
                if args.len() != 1 {
                    return Err(anyhow!("acos expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.acos())),
                    _ => Err(anyhow!("acos: expected number")),
                }
            }
            "math.atan" | "atan" => {
                if args.len() != 1 {
                    return Err(anyhow!("atan expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.atan())),
                    _ => Err(anyhow!("atan: expected number")),
                }
            }
            "math.atan2" | "atan2" => {
                if args.len() != 2 {
                    return Err(anyhow!("atan2 expects 2 arguments (y, x)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(y), Value::Number(x)) => Ok(Value::Number(y.atan2(*x))),
                    _ => Err(anyhow!("atan2: expected numbers")),
                }
            }
            "math.exp" | "exp" => {
                if args.len() != 1 {
                    return Err(anyhow!("exp expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.exp())),
                    _ => Err(anyhow!("exp: expected number")),
                }
            }
            "math.ln" | "ln" => {
                if args.len() != 1 {
                    return Err(anyhow!("ln expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.ln())),
                    _ => Err(anyhow!("ln: expected number")),
                }
            }
            "math.log2" | "log2" => {
                if args.len() != 1 {
                    return Err(anyhow!("log2 expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.log2())),
                    _ => Err(anyhow!("log2: expected number")),
                }
            }
            "math.log10" | "log10" => {
                if args.len() != 1 {
                    return Err(anyhow!("log10 expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.log10())),
                    _ => Err(anyhow!("log10: expected number")),
                }
            }
            "math.hypot" | "hypot" => {
                if args.len() != 2 {
                    return Err(anyhow!("hypot expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.hypot(*b))),
                    _ => Err(anyhow!("hypot: expected numbers")),
                }
            }
            "math.degrees" | "degrees" => {
                if args.len() != 1 {
                    return Err(anyhow!("degrees expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.to_degrees())),
                    _ => Err(anyhow!("degrees: expected number")),
                }
            }
            "math.radians" | "radians" => {
                if args.len() != 1 {
                    return Err(anyhow!("radians expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.to_radians())),
                    _ => Err(anyhow!("radians: expected number")),
                }
            }
            "math.sqrt" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.sqrt expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.sqrt())),
                    _ => Err(anyhow!("math.sqrt: expected number")),
                }
            }
            "math.abs" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.abs expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.abs())),
                    _ => Err(anyhow!("math.abs: expected number")),
                }
            }
            "math.floor" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.floor expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.floor())),
                    _ => Err(anyhow!("math.floor: expected number")),
                }
            }
            "math.ceil" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.ceil expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.ceil())),
                    _ => Err(anyhow!("math.ceil: expected number")),
                }
            }
            "math.round" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.round expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.round())),
                    _ => Err(anyhow!("math.round: expected number")),
                }
            }
            "math.pow" => {
                if args.len() != 2 {
                    return Err(anyhow!("math.pow expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(b), Value::Number(e)) => Ok(Value::Number(b.powf(*e))),
                    _ => Err(anyhow!("math.pow: expected numbers")),
                }
            }
            "math.min" => {
                if args.len() != 2 {
                    return Err(anyhow!("math.min expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.min(*b))),
                    _ => Err(anyhow!("math.min: expected numbers")),
                }
            }
            "math.max" => {
                if args.len() != 2 {
                    return Err(anyhow!("math.max expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.max(*b))),
                    _ => Err(anyhow!("math.max: expected numbers")),
                }
            }
            "math.clamp" => {
                if args.len() != 3 {
                    return Err(anyhow!("math.clamp expects 3 arguments"));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Number(v), Value::Number(lo), Value::Number(hi)) => {
                        Ok(Value::Number(v.max(*lo).min(*hi)))
                    }
                    _ => Err(anyhow!("math.clamp: expected numbers")),
                }
            }
            "math.sign" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.sign expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Number(if *n > 0.0 {
                        1.0
                    } else if *n < 0.0 {
                        -1.0
                    } else {
                        0.0
                    })),
                    _ => Err(anyhow!("math.sign: expected number")),
                }
            }
            "math.gcd" | "gcd" => {
                if args.len() != 2 {
                    return Err(anyhow!("math.gcd expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => {
                        let (mut x, mut y) =
                            ((*a as i64).unsigned_abs(), (*b as i64).unsigned_abs());
                        while y != 0 {
                            let t = y;
                            y = x % y;
                            x = t;
                        }
                        Ok(Value::Number(x as f64))
                    }
                    _ => Err(anyhow!("math.gcd: expected two numbers")),
                }
            }
            "math.lcm" | "lcm" => {
                if args.len() != 2 {
                    return Err(anyhow!("math.lcm expects 2 arguments"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => {
                        let (ai, bi) = ((*a as i64).unsigned_abs(), (*b as i64).unsigned_abs());
                        if ai == 0 || bi == 0 {
                            return Ok(Value::Number(0.0));
                        }
                        let mut x = ai;
                        let mut y = bi;
                        while y != 0 {
                            let t = y;
                            y = x % y;
                            x = t;
                        }
                        Ok(Value::Number(((ai / x) * bi) as f64))
                    }
                    _ => Err(anyhow!("math.lcm: expected two numbers")),
                }
            }
            "math.factorial" | "factorial" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.factorial expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => {
                        let ni = *n as u64;
                        if ni > 20 {
                            return Err(anyhow!("math.factorial: argument too large (max 20)"));
                        }
                        Ok(Value::Number((1..=ni).product::<u64>() as f64))
                    }
                    _ => Err(anyhow!("math.factorial: expected a non-negative integer")),
                }
            }
            "math.is_nan" | "is_nan" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.is_nan expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Bool(n.is_nan())),
                    _ => Ok(Value::Bool(false)),
                }
            }
            "math.is_inf" | "is_inf" => {
                if args.len() != 1 {
                    return Err(anyhow!("math.is_inf expects 1 argument"));
                }
                match &args[0] {
                    Value::Number(n) => Ok(Value::Bool(n.is_infinite())),
                    _ => Ok(Value::Bool(false)),
                }
            }
            "math.log" | "log" => match args.len() {
                1 => match &args[0] {
                    Value::Number(n) => Ok(Value::Number(n.ln())),
                    _ => Err(anyhow!("math.log: expected number")),
                },
                2 => match (&args[0], &args[1]) {
                    (Value::Number(x), Value::Number(b)) => Ok(Value::Number(x.log(*b))),
                    _ => Err(anyhow!("math.log: expected two numbers")),
                },
                _ => Err(anyhow!("math.log expects 1 or 2 arguments")),
            },

            // ── resolve() ─────────────────────────────────────────────────────
            "resolve" => {
                if args.len() != 1 {
                    return Err(anyhow!("resolve() expects 1 argument"));
                }
                let arg = self.deref(args[0].clone());
                match arg {
                    Value::Pending(val, trigger) => {
                        use crate::interpreter::environment::PendingTrigger;
                        match trigger {
                            PendingTrigger::AtMs(deliver_at_ms) => {
                                if deliver_at_ms > 0 {
                                    let now_ms = SystemTime::now()
                                        .duration_since(UNIX_EPOCH)
                                        .map(|d| d.as_millis() as u64)
                                        .unwrap_or(0);
                                    if now_ms < deliver_at_ms {
                                        std::thread::sleep(std::time::Duration::from_millis(
                                            deliver_at_ms - now_ms,
                                        ));
                                    }
                                }
                                Ok(*val)
                            }
                            PendingTrigger::WhenCalled(fn_name) => {
                                if self.fired_events.contains(&fn_name) {
                                    Ok(*val)
                                } else {
                                    Err(anyhow!("resolve(): trigger '{}()' has not been called yet — call {}() before resolving this handle", fn_name, fn_name))
                                }
                            }
                        }
                    }
                    other => Ok(other),
                }
            }

            // ── import() ──────────────────────────────────────────────────────
            "import" => {
                if args.len() != 1 {
                    return Err(anyhow!("import expects 1 argument (module name)"));
                }
                let module = match &args[0] {
                    Value::String(s) => s.clone(),
                    other => Executor::value_to_string(other),
                };
                let candidates = [
                    format!("stdlib/{}.ph", module),
                    format!("headers/{}.ph", module),
                    format!("{}.ph", module),
                    format!("src/stdlib/{}.ph", module),
                ];
                for path in &candidates {
                    if std::path::Path::new(path).exists() {
                        self.load_header_if_exists(path);
                        return Ok(Value::String(format!("imported {}", module)));
                    }
                }
                Err(anyhow!(
                    "import: module {} not found (searched stdlib/, headers/, .)",
                    module
                ))
            }

            // ── debug.vars ────────────────────────────────────────────────────
            "debug.vars" => {
                let vars: Vec<Value> = self
                    .env
                    .get_scopes()
                    .last()
                    .map(|s| {
                        s.get_vars()
                            .keys()
                            .map(|k| Value::String(k.clone()))
                            .collect()
                    })
                    .unwrap_or_default();
                Ok(Value::List(vars))
            }

            // ── rand.shuffle / rand.sample ────────────────────────────────────
            "rand.shuffle" => {
                if args.len() != 1 {
                    return Err(anyhow!("rand.shuffle expects 1 argument (list)"));
                }
                match &args[0] {
                    Value::List(l) => {
                        let mut v = l.clone();
                        let n = v.len();
                        for i in (1..n).rev() {
                            let j = (self.rng.next_u64() as usize) % (i + 1);
                            v.swap(i, j);
                        }
                        Ok(Value::List(v))
                    }
                    _ => Err(anyhow!("rand.shuffle: argument must be a list")),
                }
            }
            "rand.sample" => {
                if args.len() != 2 {
                    return Err(anyhow!("rand.sample expects 2 arguments (list, k)"));
                }
                let k = match &args[1] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("rand.sample: k must be a number")),
                };
                match &args[0] {
                    Value::List(l) => {
                        if k > l.len() {
                            return Err(anyhow!(
                                "rand.sample: k ({}) > list length ({})",
                                k,
                                l.len()
                            ));
                        }
                        let mut indices: Vec<usize> = (0..l.len()).collect();
                        for i in 0..k {
                            let j = i + (self.rng.next_u64() as usize) % (l.len() - i);
                            indices.swap(i, j);
                        }
                        Ok(Value::List(
                            indices[..k].iter().map(|&i| l[i].clone()).collect(),
                        ))
                    }
                    _ => Err(anyhow!("rand.sample: first argument must be a list")),
                }
            }

            // ── time.delta ────────────────────────────────────────────────────
            "time.delta" => {
                if args.len() != 2 {
                    return Err(anyhow!("time.delta expects 2 arguments (a, b)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(a), Value::Number(b)) => Ok(Value::Number(b - a)),
                    _ => Err(anyhow!("time.delta: expected two numbers (epoch_ms)")),
                }
            }

            // ── type_of ───────────────────────────────────────────────────────
            "type_of" => {
                if args.len() != 1 {
                    return Err(anyhow!("type_of expects 1 argument"));
                }
                let t = match &args[0] {
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Bool(_) => "bool",
                    Value::List(_) => "list",
                    Value::Tensor(_) => "tensor",
                    Value::Lambda(_, _, _) => "lambda",
                    Value::LazyImport { .. } => "lazy",
                    Value::Heap(_) => "heap",
                    Value::Dict(_) => "dict",
                    Value::Pending(_, _) => "pending",
                    Value::Pointer(_) => "pointer",
                    Value::FamilyNode { .. } => "family_node",
                    Value::Builtin(_) => "builtin",
                    Value::None => "none",
                };
                Ok(Value::String(t.to_string()))
            }

            // ══════════════════════════════════════════════════════════════════
            // FAST MEMORY ACCESS - PEEK/POKE for direct indexed byte access
            // ══════════════════════════════════════════════════════════════════

            // PEEK(ptr, index) -> number (0-255)
            // Fast indexed read without GOTO/SEEK/PULL overhead
            "peek" => {
                if args.len() != 2 {
                    return Err(anyhow!("PEEK expects 2 arguments (ptr, index)"));
                }
                let index = match &args[1] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("PEEK: index must be a number")),
                };

                // Extract pointer ID
                let ptr_id: u64 = match &args[0] {
                    Value::Pointer(id) => *id,
                    Value::String(h) if h.starts_with("mem://") => h
                        .trim_start_matches("mem://")
                        .parse()
                        .map_err(|_| anyhow!("PEEK: invalid handle"))?,
                    _ => return Err(anyhow!("PEEK: expected pointer or mem:// handle")),
                };

                let reg = self
                    .pointer_registry
                    .read()
                    .map_err(|e| anyhow!("PEEK: lock error: {}", e))?;
                if let Some(ptr) = reg.lookup(ptr_id) {
                    if let crate::runtime::pointer::pointer::PointerTarget::Memory {
                        data, ..
                    } = &ptr.target
                    {
                        if index < data.len() {
                            Ok(Value::Number(data[index] as f64))
                        } else {
                            Err(anyhow!(
                                "PEEK: index {} out of bounds (size {})",
                                index,
                                data.len()
                            ))
                        }
                    } else {
                        Err(anyhow!("PEEK: pointer is not a memory pointer"))
                    }
                } else {
                    Err(anyhow!("PEEK: invalid pointer ID {}", ptr_id))
                }
            }

            // POKE(ptr, index, value) -> none
            // Fast indexed write without GOTO/SEEK/PUSH overhead
            "poke" => {
                if args.len() != 3 {
                    return Err(anyhow!("POKE expects 3 arguments (ptr, index, value)"));
                }
                let index = match &args[1] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("POKE: index must be a number")),
                };
                let byte = match &args[2] {
                    Value::Number(n) => (*n as i32).clamp(0, 255) as u8,
                    _ => return Err(anyhow!("POKE: value must be a number")),
                };

                let ptr_id: u64 = match &args[0] {
                    Value::Pointer(id) => *id,
                    Value::String(h) if h.starts_with("mem://") => h
                        .trim_start_matches("mem://")
                        .parse()
                        .map_err(|_| anyhow!("POKE: invalid handle"))?,
                    _ => return Err(anyhow!("POKE: expected pointer or mem:// handle")),
                };

                let mut reg = self
                    .pointer_registry
                    .write()
                    .map_err(|e| anyhow!("POKE: lock error: {}", e))?;
                if let Some(ptr) = reg.lookup_mut(ptr_id) {
                    if let crate::runtime::pointer::pointer::PointerTarget::Memory {
                        data, ..
                    } = &mut ptr.target
                    {
                        if index < data.len() {
                            data[index] = byte;
                            Ok(Value::None)
                        } else {
                            Err(anyhow!(
                                "POKE: index {} out of bounds (size {})",
                                index,
                                data.len()
                            ))
                        }
                    } else {
                        Err(anyhow!("POKE: pointer is not a memory pointer"))
                    }
                } else {
                    Err(anyhow!("POKE: invalid pointer ID {}", ptr_id))
                }
            }

            // wrap(value, max) -> wrapped value (0 to max-1)
            // For toroidal grid wrapping: wrap(x + 1, WIDTH)
            "wrap" => {
                if args.len() != 2 {
                    return Err(anyhow!("wrap expects 2 arguments (value, max)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(v), Value::Number(m)) => {
                        let max = *m as i64;
                        if max <= 0 {
                            return Err(anyhow!("wrap: max must be > 0"));
                        }
                        let val = *v as i64;
                        let wrapped = ((val % max) + max) % max;
                        Ok(Value::Number(wrapped as f64))
                    }
                    _ => Err(anyhow!("wrap: expected (number, number)")),
                }
            }

            // idx(x, y, width) -> linear index
            // Convert 2D coords to linear index: idx(3, 5, 10) = 53
            "idx" => {
                if args.len() != 3 {
                    return Err(anyhow!("idx expects 3 arguments (x, y, width)"));
                }
                match (&args[0], &args[1], &args[2]) {
                    (Value::Number(x), Value::Number(y), Value::Number(w)) => {
                        let index = (*y as i64) * (*w as i64) + (*x as i64);
                        Ok(Value::Number(index as f64))
                    }
                    _ => Err(anyhow!("idx: expected (x, y, width)")),
                }
            }

            // idx2d(index, width) -> [x, y]
            // Convert linear index to 2D coords: idx2d(53, 10) = [3, 5]
            "idx2d" => {
                if args.len() != 2 {
                    return Err(anyhow!("idx2d expects 2 arguments (index, width)"));
                }
                match (&args[0], &args[1]) {
                    (Value::Number(i), Value::Number(w)) => {
                        let width = *w as i64;
                        if width <= 0 {
                            return Err(anyhow!("idx2d: width must be > 0"));
                        }
                        let index = *i as i64;
                        let y = index / width;
                        let x = index % width;
                        Ok(Value::List(vec![
                            Value::Number(x as f64),
                            Value::Number(y as f64),
                        ]))
                    }
                    _ => Err(anyhow!("idx2d: expected (index, width)")),
                }
            }

            // memset(ptr, value, count) -> none
            // Fast memory fill
            "memset" => {
                if args.len() != 3 {
                    return Err(anyhow!("memset expects 3 arguments (ptr, value, count)"));
                }
                let byte = match &args[1] {
                    Value::Number(n) => (*n as i32).clamp(0, 255) as u8,
                    _ => return Err(anyhow!("memset: value must be a number")),
                };
                let count = match &args[2] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("memset: count must be a number")),
                };

                let ptr_id: u64 = match &args[0] {
                    Value::Pointer(id) => *id,
                    Value::String(h) if h.starts_with("mem://") => h
                        .trim_start_matches("mem://")
                        .parse()
                        .map_err(|_| anyhow!("memset: invalid handle"))?,
                    _ => return Err(anyhow!("memset: expected pointer or mem:// handle")),
                };

                let mut reg = self
                    .pointer_registry
                    .write()
                    .map_err(|e| anyhow!("memset: lock error: {}", e))?;
                if let Some(ptr) = reg.lookup_mut(ptr_id) {
                    if let crate::runtime::pointer::pointer::PointerTarget::Memory {
                        data, ..
                    } = &mut ptr.target
                    {
                        let end = count.min(data.len());
                        for i in 0..end {
                            data[i] = byte;
                        }
                        Ok(Value::None)
                    } else {
                        Err(anyhow!("memset: pointer is not a memory pointer"))
                    }
                } else {
                    Err(anyhow!("memset: invalid pointer ID {}", ptr_id))
                }
            }

            // memcpy(src, dst, count) -> none
            // Fast memory copy between pointers
            "memcpy" => {
                if args.len() != 3 {
                    return Err(anyhow!("memcpy expects 3 arguments (src, dst, count)"));
                }
                let count = match &args[2] {
                    Value::Number(n) => *n as usize,
                    _ => return Err(anyhow!("memcpy: count must be a number")),
                };

                let src_id: u64 = match &args[0] {
                    Value::Pointer(id) => *id,
                    Value::String(h) if h.starts_with("mem://") => h
                        .trim_start_matches("mem://")
                        .parse()
                        .map_err(|_| anyhow!("memcpy: invalid src handle"))?,
                    _ => return Err(anyhow!("memcpy: src must be a pointer")),
                };
                let dst_id: u64 = match &args[1] {
                    Value::Pointer(id) => *id,
                    Value::String(h) if h.starts_with("mem://") => h
                        .trim_start_matches("mem://")
                        .parse()
                        .map_err(|_| anyhow!("memcpy: invalid dst handle"))?,
                    _ => return Err(anyhow!("memcpy: dst must be a pointer")),
                };

                // Read source data
                let src_bytes: Vec<u8> = {
                    let reg = self
                        .pointer_registry
                        .read()
                        .map_err(|e| anyhow!("memcpy: lock error: {}", e))?;
                    if let Some(ptr) = reg.lookup(src_id) {
                        if let crate::runtime::pointer::pointer::PointerTarget::Memory {
                            data,
                            ..
                        } = &ptr.target
                        {
                            let end = count.min(data.len());
                            data[0..end].to_vec()
                        } else {
                            return Err(anyhow!("memcpy: src is not a memory pointer"));
                        }
                    } else {
                        return Err(anyhow!("memcpy: invalid src pointer ID"));
                    }
                };

                // Write to destination
                let mut reg = self
                    .pointer_registry
                    .write()
                    .map_err(|e| anyhow!("memcpy: lock error: {}", e))?;
                if let Some(ptr) = reg.lookup_mut(dst_id) {
                    if let crate::runtime::pointer::pointer::PointerTarget::Memory {
                        data, ..
                    } = &mut ptr.target
                    {
                        let end = src_bytes.len().min(data.len());
                        data[0..end].copy_from_slice(&src_bytes[0..end]);
                        Ok(Value::None)
                    } else {
                        Err(anyhow!("memcpy: dst is not a memory pointer"))
                    }
                } else {
                    Err(anyhow!("memcpy: invalid dst pointer ID"))
                }
            }

            other => Err(anyhow!("Unknown function '{}'", other)),
        }
    }

    // ── Value helpers ─────────────────────────────────────────────────────────

    pub fn value_to_string(v: &Value) -> String {
        match v {
            Value::Number(n) => fmt_f64(*n),
            Value::String(s) => s.clone(),
            Value::Bool(b) => b.to_string(),
            Value::List(items) => {
                let parts: Vec<String> = items.iter().map(Executor::value_to_string).collect();
                format!("[{}]", parts.join(", "))
            }
            Value::Tensor(_) => "<tensor>".to_string(),
            Value::Lambda(_, _, _) => "<lambda>".to_string(),
            Value::LazyImport {
                module,
                name,
                alias,
            } => {
                if let Some(a) = alias {
                    format!("<lazy:{}:{} as {}>", module, name, a)
                } else {
                    format!("<lazy:{}:{}>", module, name)
                }
            }
            Value::None => "None".to_string(),
            Value::Heap(_) => "<heap>".to_string(),
            Value::Dict(map) => {
                let mut pairs: Vec<(&String, &Value)> = map.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                let inner: Vec<String> = pairs
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, Executor::value_to_string(v)))
                    .collect();
                format!("{{{}}}", inner.join(", "))
            }
            Value::Pending(v, trigger) => {
                format!("<pending:{} {}>", Executor::value_to_string(v), trigger)
            }
            Value::Pointer(id) => format!("<pointer:{}>", id),
            Value::FamilyNode { id, mutable } => {
                format!("<obj:{}{}>", id, if *mutable { ".MUT" } else { "" })
            }
            Value::Builtin(name) => format!("<builtin: {}>", name),
        }
    }

    // ── JSON helpers ──────────────────────────────────────────────────────────

    /// Convert a serde_json::Value to a Pasta Value, allocating lists/dicts on GC.
    fn json_to_value(gc: &mut crate::runtime::strainer::Strainer, jv: &serde_json::Value) -> Value {
        match jv {
            serde_json::Value::Null => Value::None,
            serde_json::Value::Bool(b) => Value::Bool(*b),
            serde_json::Value::Number(n) => Value::Number(n.as_f64().unwrap_or(0.0)),
            serde_json::Value::String(s) => Value::String(s.clone()),
            serde_json::Value::Array(arr) => {
                let items: Vec<Value> = arr.iter().map(|v| Self::json_to_value(gc, v)).collect();
                let id = gc.allocate(Value::List(items));
                Value::Heap(id)
            }
            serde_json::Value::Object(obj) => {
                let map: std::collections::HashMap<String, Value> = obj
                    .iter()
                    .map(|(k, v)| (k.clone(), Self::json_to_value(gc, v)))
                    .collect();
                let id = gc.allocate(Value::Dict(map));
                Value::Heap(id)
            }
        }
    }

    /// Convert a Pasta Value to a serde_json::Value for serialisation.
    /// Non-serialisable types (Lambda, Tensor, etc.) become JSON strings.
    fn value_to_json(gc: &crate::runtime::strainer::Strainer, v: &Value) -> serde_json::Value {
        match v {
            Value::None => serde_json::Value::Null,
            Value::Bool(b) => serde_json::Value::Bool(*b),
            Value::Number(n) => {
                // Serialize whole numbers as integers, fractions as floats
                if n.fract() == 0.0 && n.abs() < 9.007199254740992e15 {
                    serde_json::Value::Number((*n as i64).into())
                } else {
                    serde_json::Number::from_f64(*n)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null)
                }
            }
            Value::String(s) => serde_json::Value::String(s.clone()),
            Value::List(items) => {
                serde_json::Value::Array(items.iter().map(|i| Self::value_to_json(gc, i)).collect())
            }
            Value::Dict(map) => {
                let obj: serde_json::Map<String, serde_json::Value> = map
                    .iter()
                    .map(|(k, val)| (k.clone(), Self::value_to_json(gc, val)))
                    .collect();
                serde_json::Value::Object(obj)
            }
            Value::Heap(id) => match gc.get(*id) {
                Some(inner) => Self::value_to_json(gc, &inner.clone()),
                None => serde_json::Value::String("<heap>".to_string()),
            },
            other => serde_json::Value::String(Self::value_to_string(other)),
        }
    }

    #[allow(dead_code)]
    fn tensor_to_string(&self, t: &RuntimeTensor) -> String {
        let shape_str = t
            .shape
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>()
            .join(",");
        let header = format!("tensor<{}>[{}]", t.dtype, shape_str);
        const MAX_SHOW: usize = 64;
        match t.rank() {
            0 => format!(
                "{} scalar({})",
                header,
                if t.data.is_empty() {
                    "?".to_string()
                } else {
                    fmt_f64(t.data[0])
                }
            ),
            1 => {
                let truncated = t.data.len() > MAX_SHOW;
                let items: Vec<String> =
                    t.data.iter().take(MAX_SHOW).map(|&v| fmt_f64(v)).collect();
                format!(
                    "{}{}",
                    header,
                    if truncated {
                        format!("[{}, ...]", items.join(", "))
                    } else {
                        format!("[{}]", items.join(", "))
                    }
                )
            }
            2 => {
                let (rows, cols) = (t.shape[0], t.shape[1]);
                let max_rows = if rows > 8 { 4 } else { rows };
                let mut row_strs: Vec<String> = Vec::with_capacity(max_rows);
                for r in 0..max_rows {
                    let start = r * cols;
                    let end = (start + cols).min(t.data.len());
                    let items: Vec<String> = t.data[start..end]
                        .iter()
                        .take(MAX_SHOW)
                        .map(|&v| fmt_f64(v))
                        .collect();
                    row_strs.push(if cols > MAX_SHOW {
                        format!("[{}, ...]", items.join(", "))
                    } else {
                        format!("[{}]", items.join(", "))
                    });
                }
                format!(
                    "{}\n[{}]",
                    header,
                    if rows > 8 {
                        format!(
                            "{},\n  ...\n  (showing {}/{} rows)",
                            row_strs.join(",\n "),
                            max_rows,
                            rows
                        )
                    } else {
                        row_strs.join(",\n ")
                    }
                )
            }
            _ => {
                let truncated = t.data.len() > MAX_SHOW;
                let items: Vec<String> =
                    t.data.iter().take(MAX_SHOW).map(|&v| fmt_f64(v)).collect();
                format!(
                    "{}{}",
                    header,
                    if truncated {
                        format!("[{}, ...]", items.join(", "))
                    } else {
                        format!("[{}]", items.join(", "))
                    }
                )
            }
        }
    }

    pub fn do_print(&self, v: &Value) {
        let resolved = self.deref(v.clone());
        match &resolved {
            Value::None => println!("None"),
            Value::Builtin(name) => println!("<builtin: {}>", name),
            Value::List(items) => {
                // Multi-arg PRINT (and explicit list prints) are rendered
                // space-separated without brackets, matching Python-style
                // `print(a, b, c)` semantics.
                let parts: Vec<String> = items
                    .iter()
                    .map(|item| Executor::value_to_string(&self.deref(item.clone())))
                    .collect();
                println!("{}", parts.join(" "));
            }
            other => println!("{}", Executor::value_to_string(other)),
        }
    }

    // ── Tensor builder ────────────────────────────────────────────────────────

    pub fn build_tensor(&mut self, expr: &Expr) -> Result<Value> {
        let tmp = self.eval_expr(expr)?;
        let evaluated = self.deref(tmp);

        fn collect(exe: &Executor, v: &Value) -> Result<(Vec<usize>, Vec<f64>)> {
            let v = exe.deref(v.clone());
            match &v {
                Value::Number(n) => Ok((Vec::new(), vec![*n])),
                Value::List(items) => {
                    if items.is_empty() {
                        return Err(anyhow!("Tensor rows cannot be empty"));
                    }
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
                    let mut shape = Vec::with_capacity(1 + first_shape.len());
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
        Ok(Value::Tensor(RuntimeTensor::new(
            shape,
            "float32".to_string(),
            data,
        )))
    }

    // NEW: build tensor from already-evaluated Value
    pub fn build_tensor_from_value(&mut self, v: &Value) -> Result<Value> {
        fn collect(exe: &Executor, v: &Value) -> Result<(Vec<usize>, Vec<f64>)> {
            let v = exe.deref(v.clone());
            match &v {
                Value::Number(n) => Ok((Vec::new(), vec![*n])),
                Value::List(items) => {
                    if items.is_empty() {
                        return Err(anyhow!("Tensor rows cannot be empty"));
                    }
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
                    let mut shape = Vec::with_capacity(1 + first_shape.len());
                    shape.push(items.len());
                    shape.extend(first_shape);
                    Ok((shape, flat))
                }
                other => Err(anyhow!("Tensor element must be a number, got: {:?}", other)),
            }
        }

        let (shape, data) = collect(self, v)?;
        if shape.is_empty() {
            return Err(anyhow!("Cannot build tensor from scalar"));
        }
        Ok(Value::Tensor(RuntimeTensor::new(
            shape,
            "float32".to_string(),
            data,
        )))
    }

    // ── Binary operations ─────────────────────────────────────────────────────

    pub fn eval_binary(&self, op: &BinaryOp, left: Value, right: Value) -> Result<Value> {
        use BinaryOp::*;
        let left = self.deref(left);
        let right = self.deref(right);

        let coerce_to_number = |v: &Value| -> Option<f64> {
            match v {
                Value::Number(n) => Some(*n),
                Value::String(s) => {
                    let trimmed = s.trim();
                    if trimmed.is_empty() {
                        Some(0.0)
                    } else {
                        trimmed.parse::<f64>().ok()
                    }
                }
                _ => None,
            }
        };
        let coerce_to_num_cmp = |v: &Value| -> Option<f64> {
            match v {
                Value::Number(n) => Some(*n),
                Value::String(s) => s.trim().parse::<f64>().ok(),
                Value::Bool(b) => Some(if *b { 1.0 } else { 0.0 }),
                Value::None => Some(0.0),
                Value::List(l) => Some(l.len() as f64),
                _ => None,
            }
        };
        let value_to_lex_str = |v: &Value| -> String {
            match v {
                Value::String(s) => s.clone(),
                Value::Number(n) => {
                    if n.fract() == 0.0 && n.abs() < 1e15 {
                        format!("{}", *n as i64)
                    } else {
                        format!("{}", n)
                    }
                }
                Value::Bool(b) => b.to_string(),
                Value::None => String::new(),
                Value::List(l) => format!("{}", l.len()),
                _ => format!("{:?}", v),
            }
        };

        match op {
            Add => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a + b)),
                (Value::String(a), Value::String(b)) => Ok(Value::String(a.clone() + b)),
                (Value::String(a), Value::Number(b)) => {
                    // If the string is numeric (including ""), coerce and add.
                    if let Some(na) = coerce_to_number(&Value::String(a.clone())) {
                        Ok(Value::Number(na + b))
                    } else {
                        Ok(Value::String(a.clone() + &fmt_f64(*b)))
                    }
                }
                (Value::Number(a), Value::String(b)) => {
                    if let Some(nb) = coerce_to_number(&Value::String(b.clone())) {
                        Ok(Value::Number(a + nb))
                    } else {
                        Ok(Value::String(fmt_f64(*a) + b))
                    }
                }
                // String + Bool / String + None → string concat
                (Value::String(a), Value::Bool(b)) => {
                    Ok(Value::String(a.clone() + if *b { "true" } else { "false" }))
                }
                (Value::Bool(b), Value::String(a)) => Ok(Value::String(
                    (if *b { "true" } else { "false" }).to_string() + a,
                )),
                (Value::String(a), Value::None) => Ok(Value::String(a.clone())),
                (Value::None, Value::String(b)) => Ok(Value::String(b.clone())),
                (Value::Tensor(a), Value::Tensor(b)) => {
                    Executor::tensor_elementwise(a, b, |x, y| x + y)
                }
                (Value::Tensor(a), Value::Number(s)) => {
                    Executor::tensor_scalar(a, *s, |x, y| x + y)
                }
                (Value::Number(s), Value::Tensor(b)) => {
                    Executor::tensor_scalar(b, *s, |x, y| y + x)
                }
                // List concatenation - handle BEFORE number coercion fallback
                (Value::List(_), Value::List(_)) => match (left, right) {
                    (Value::List(mut a), Value::List(b)) => {
                        a.extend(b);
                        Ok(Value::List(a))
                    }
                    _ => unreachable!(),
                },
                _ => {
                    if let (Some(na), Some(nb)) = (
                        coerce_to_number(&left).or_else(|| coerce_to_num_cmp(&left)),
                        coerce_to_number(&right).or_else(|| coerce_to_num_cmp(&right)),
                    ) {
                        return Ok(Value::Number(na + nb));
                    }
                    Err(anyhow!("Unsupported operands for +"))
                }
            },
            // These four ops are intercepted in ex_eval::eval_expr before
            // eval_binary is ever called. Reaching here is a bug.
            Pipe | PipeOr | PipeBoth | PipeMap => Err(anyhow::anyhow!(
                "pipe operator '{}' reached eval_binary — should have been handled in eval_expr",
                format!("{:?}", op)
            )),
            Sub => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a - b)),
                (Value::Number(a), Value::String(b)) => {
                    if let Some(nb) = coerce_to_number(&Value::String(b.clone())) {
                        Ok(Value::Number(a - nb))
                    } else {
                        Err(anyhow!("Unsupported operands for -"))
                    }
                }
                (Value::String(a), Value::Number(b)) => {
                    if let Some(na) = coerce_to_number(&Value::String(a.clone())) {
                        Ok(Value::Number(na - b))
                    } else {
                        Err(anyhow!("Cannot subtract: '{}' is not numeric", a))
                    }
                }
                (Value::Tensor(a), Value::Tensor(b)) => {
                    Executor::tensor_elementwise(a, b, |x, y| x - y)
                }
                (Value::Tensor(a), Value::Number(s)) => {
                    Executor::tensor_scalar(a, *s, |x, y| x - y)
                }
                (Value::Number(s), Value::Tensor(b)) => {
                    Executor::tensor_scalar(b, *s, |y, x| x - y)
                }
                _ => Err(anyhow!("Unsupported operands for -")),
            },
            Mul => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a * b)),
                (Value::Number(a), Value::String(b)) => {
                    if let Some(nb) = coerce_to_number(&Value::String(b.clone())) {
                        Ok(Value::Number(a * nb))
                    } else {
                        Err(anyhow!("Unsupported operands for *"))
                    }
                }
                (Value::String(a), Value::Number(b)) => {
                    if let Some(na) = coerce_to_number(&Value::String(a.clone())) {
                        Ok(Value::Number(na * b))
                    } else {
                        Err(anyhow!("Cannot multiply: '{}' is not numeric", a))
                    }
                }
                (Value::Tensor(a), Value::Tensor(b)) => {
                    Executor::tensor_elementwise(a, b, |x, y| x * y)
                }
                (Value::Tensor(a), Value::Number(s)) => {
                    Executor::tensor_scalar(a, *s, |x, y| x * y)
                }
                (Value::Number(s), Value::Tensor(b)) => {
                    Executor::tensor_scalar(b, *s, |x, y| y * x)
                }
                _ => Err(anyhow!("Unsupported operands for *")),
            },
            Div => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b == 0.0 {
                        Err(anyhow!("Division by zero"))
                    } else {
                        Ok(Value::Number(a / b))
                    }
                }
                (Value::Number(a), Value::String(b)) => {
                    if let Some(nb) = coerce_to_number(&Value::String(b.clone())) {
                        if nb == 0.0 {
                            Err(anyhow!("Division by zero"))
                        } else {
                            Ok(Value::Number(a / nb))
                        }
                    } else {
                        Err(anyhow!("Unsupported operands for /"))
                    }
                }
                (Value::Tensor(a), Value::Number(s)) => {
                    if *s == 0.0 {
                        return Err(anyhow!("Tensor division by zero"));
                    }
                    Executor::tensor_scalar(a, *s, |x, y| x / y)
                }
                (Value::Number(s), Value::Tensor(b)) => {
                    if *s == 0.0 {
                        return Err(anyhow!("Tensor division by zero"));
                    }
                    Executor::tensor_scalar(b, *s, |y, x| x / y)
                }
                (Value::Tensor(a), Value::Tensor(b)) => {
                    Executor::tensor_elementwise(a, b, |x, y| x / y)
                }
                _ => Err(anyhow!("Unsupported operands for /")),
            },
            Mod => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b == 0.0 {
                        return Err(anyhow!("modulo by zero"));
                    }
                    Ok(Value::Number(a % b))
                }
                (Value::Number(a), Value::String(b)) => {
                    if let Some(nb) = coerce_to_number(&Value::String(b.clone())) {
                        if nb == 0.0 {
                            return Err(anyhow!("modulo by zero"));
                        }
                        Ok(Value::Number(a % nb))
                    } else {
                        Err(anyhow!("Unsupported operands for %"))
                    }
                }
                _ => Err(anyhow!("Unsupported operands for %")),
            },
            Pow => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => Ok(Value::Number(a.powf(*b))),
                (Value::Number(a), Value::String(b)) => {
                    if let Some(nb) = coerce_to_number(&Value::String(b.clone())) {
                        Ok(Value::Number(a.powf(nb)))
                    } else {
                        Err(anyhow!("Unsupported operands for ^"))
                    }
                }
                _ => Err(anyhow!("Unsupported operands for ^")),
            },
            MatMul => match (&left, &right) {
                (Value::Tensor(a), Value::Tensor(b)) => Executor::tensor_matmul(a, b),
                _ => Err(anyhow!("@ (matmul) requires two tensors")),
            },
            Eq => {
                if left == right {
                    return Ok(Value::Bool(true));
                }
                match (&left, &right) {
                    (Value::String(s), Value::Number(n)) => {
                        if let Some(v) = coerce_to_number(&Value::String(s.clone())) {
                            return Ok(Value::Bool((v - n).abs() < 1e-12));
                        }
                    }
                    (Value::Number(n), Value::String(s)) => {
                        if let Some(v) = coerce_to_number(&Value::String(s.clone())) {
                            return Ok(Value::Bool((n - v).abs() < 1e-12));
                        }
                    }
                    _ => {}
                }
                Ok(Value::Bool(false))
            }
            Neq => {
                if left == right {
                    return Ok(Value::Bool(false));
                }
                match (&left, &right) {
                    (Value::String(s), Value::Number(n)) => {
                        if let Some(v) = coerce_to_number(&Value::String(s.clone())) {
                            return Ok(Value::Bool((v - n).abs() >= 1e-12));
                        }
                    }
                    (Value::Number(n), Value::String(s)) => {
                        if let Some(v) = coerce_to_number(&Value::String(s.clone())) {
                            return Ok(Value::Bool((n - v).abs() >= 1e-12));
                        }
                    }
                    _ => {}
                }
                Ok(Value::Bool(true))
            }
            Lt => {
                let (a, b) = (coerce_to_num_cmp(&left), coerce_to_num_cmp(&right));
                match (a, b) {
                    (Some(x), Some(y)) => Ok(Value::Bool(x < y)),
                    _ => Ok(Value::Bool(
                        value_to_lex_str(&left) < value_to_lex_str(&right),
                    )),
                }
            }
            Gt => {
                let (a, b) = (coerce_to_num_cmp(&left), coerce_to_num_cmp(&right));
                match (a, b) {
                    (Some(x), Some(y)) => Ok(Value::Bool(x > y)),
                    _ => Ok(Value::Bool(
                        value_to_lex_str(&left) > value_to_lex_str(&right),
                    )),
                }
            }
            Lte => {
                let (a, b) = (coerce_to_num_cmp(&left), coerce_to_num_cmp(&right));
                match (a, b) {
                    (Some(x), Some(y)) => Ok(Value::Bool(x <= y)),
                    _ => Ok(Value::Bool(
                        value_to_lex_str(&left) <= value_to_lex_str(&right),
                    )),
                }
            }
            Gte => {
                let (a, b) = (coerce_to_num_cmp(&left), coerce_to_num_cmp(&right));
                match (a, b) {
                    (Some(x), Some(y)) => Ok(Value::Bool(x >= y)),
                    _ => Ok(Value::Bool(
                        value_to_lex_str(&left) >= value_to_lex_str(&right),
                    )),
                }
            }
            Approx => match (&left, &right) {
                (Value::String(a), Value::String(b)) if a.is_empty() && b.is_empty() => {
                    Ok(Value::Bool(true))
                }
                (Value::Number(a), Value::Number(b)) => Ok(Value::Bool((a - b).abs() <= 0.0001)),
                (Value::String(a), Value::String(b)) => Ok(Value::Bool(
                    a.to_lowercase() == b.to_lowercase() || Executor::levenshtein(a, b) <= 1,
                )),
                (Value::None, Value::None) => Ok(Value::Bool(true)),
                _ => Ok(Value::Bool(false)),
            },
            NotEq => Ok(Value::Bool(left != right)),
            StrictEq => {
                let eq = match (&left, &right) {
                    (Value::Number(a), Value::Number(b)) => (a - b).abs() < f64::EPSILON,
                    (Value::String(a), Value::String(b)) => a == b,
                    (Value::Bool(a), Value::Bool(b)) => a == b,
                    (Value::None, Value::None) => true,
                    _ => false,
                };
                Ok(Value::Bool(eq))
            }
            And => Ok(Value::Bool(
                self.value_is_truthy(&left) && self.value_is_truthy(&right),
            )),
            Or => Ok(Value::Bool(
                self.value_is_truthy(&left) || self.value_is_truthy(&right),
            )),
            Not => match right {
                Value::Bool(b) => Ok(Value::Bool(!b)),
                other => Ok(Value::Bool(!self.value_is_truthy(&other))),
            },

            // ── New operators ─────────────────────────────────────────────────
            FloorDiv => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b == 0.0 {
                        Err(anyhow!("Floor division by zero"))
                    } else {
                        Ok(Value::Number((a / b).floor()))
                    }
                }
                _ => Err(anyhow!("// requires numeric operands")),
            },
            TruncDiv => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    if *b == 0.0 {
                        Err(anyhow!("Truncating division by zero"))
                    } else {
                        Ok(Value::Number((a / b).trunc()))
                    }
                }
                _ => Err(anyhow!("\\ requires numeric operands")),
            },
            Shl => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    Ok(Value::Number((*a as i64).wrapping_shl(*b as u32) as f64))
                }
                _ => Err(anyhow!("<< requires numeric operands")),
            },
            Shr => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    Ok(Value::Number((*a as i64).wrapping_shr(*b as u32) as f64))
                }
                _ => Err(anyhow!(">> requires numeric operands")),
            },
            BitAnd => match (&left, &right) {
                (Value::Number(a), Value::Number(b)) => {
                    Ok(Value::Number((*a as i64 & *b as i64) as f64))
                }
                _ => Err(anyhow!("& requires numeric operands")),
            },
            PipeArrow => {
                // f |> x  means  f(x): apply left (function) to right (argument)
                let callee_val = self.deref(left.clone());
                match callee_val {
                    Value::Lambda(params, body, _captures_e) => {
                        // Create a temporary executor-like call via returning the value
                        // We can't call exec here (no &mut Executor), so return a helpful error
                        // directing users to use the pipe |> only as callee | arg form.
                        // Note: actual dispatch happens in ex_eval before eval_binary is called.
                        let _ = (params, body);
                        Err(anyhow!("|> pipe-arrow reached eval_binary — should have been intercepted in eval_expr"))
                    }
                    _ => Err(anyhow!("|> left operand must be a function")),
                }
            }
        }
    }

    // ── Tensor arithmetic helpers ─────────────────────────────────────────────

    pub fn tensor_elementwise(
        a: &RuntimeTensor,
        b: &RuntimeTensor,
        op: impl Fn(f64, f64) -> f64,
    ) -> Result<Value> {
        if a.shape != b.shape {
            return Err(anyhow!(
                "Shape mismatch for elementwise op: {:?} vs {:?}",
                a.shape,
                b.shape
            ));
        }
        let data: Vec<f64> = a
            .data
            .iter()
            .zip(b.data.iter())
            .map(|(&x, &y)| op(x, y))
            .collect();
        let dtype = if a.dtype == "float32" || b.dtype == "float32" {
            "float32"
        } else {
            "int32"
        };
        Ok(Value::Tensor(RuntimeTensor::new(
            a.shape.clone(),
            dtype.to_string(),
            data,
        )))
    }

    pub fn tensor_scalar(
        t: &RuntimeTensor,
        scalar: f64,
        op: impl Fn(f64, f64) -> f64,
    ) -> Result<Value> {
        let data: Vec<f64> = t.data.iter().map(|&x| op(x, scalar)).collect();
        let dtype = if t.dtype == "float32" || scalar.fract() != 0.0 {
            "float32"
        } else {
            "int32"
        };
        Ok(Value::Tensor(RuntimeTensor::new(
            t.shape.clone(),
            dtype.to_string(),
            data,
        )))
    }

    pub fn tensor_matmul(a: &RuntimeTensor, b: &RuntimeTensor) -> Result<Value> {
        if a.rank() != 2 || b.rank() != 2 {
            return Err(anyhow!(
                "@ (matmul) requires 2D tensors, got shapes {:?} and {:?}",
                a.shape,
                b.shape
            ));
        }
        let (m, k1) = (a.shape[0], a.shape[1]);
        let (k2, n) = (b.shape[0], b.shape[1]);
        if k1 != k2 {
            return Err(anyhow!(
                "matmul inner dimensions must match: {} vs {}",
                k1,
                k2
            ));
        }
        let mut out = vec![0.0f64; m * n];
        for i in 0..m {
            for j in 0..n {
                let mut sum = 0.0f64;
                for k in 0..k1 {
                    sum += a.data[i * k1 + k] * b.data[k * n + j];
                }
                out[i * n + j] = sum;
            }
        }
        let dtype = if a.dtype == "float32" || b.dtype == "float32" {
            "float32"
        } else {
            "int32"
        };
        Ok(Value::Tensor(RuntimeTensor::new(
            vec![m, n],
            dtype.to_string(),
            out,
        )))
    }

    // ── Levenshtein ───────────────────────────────────────────────────────────

    pub fn levenshtein(a: &str, b: &str) -> usize {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let (m, n) = (a.len(), b.len());
        if m == 0 {
            return n;
        }
        if n == 0 {
            return m;
        }
        let mut prev: Vec<usize> = (0..=n).collect();
        let mut curr = vec![0usize; n + 1];
        for i in 1..=m {
            curr[0] = i;
            for j in 1..=n {
                let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
                curr[j] = (prev[j] + 1).min(curr[j - 1] + 1).min(prev[j - 1] + cost);
            }
            std::mem::swap(&mut prev, &mut curr);
        }
        prev[n]
    }

    // ── Repeat-count resolution ───────────────────────────────────────────────

    pub fn resolve_repeat_counts(
        &mut self,
        n_targets: usize,
        repeats_opt: Option<&Vec<Expr>>,
        span: &Span,
    ) -> Result<Vec<usize>> {
        if n_targets == 0 {
            return Ok(vec![]);
        }
        let repeats = match repeats_opt {
            None => return Ok(vec![1; n_targets]),
            Some(r) => r,
        };
        if repeats.len() == 1 {
            let r_val = self.eval_expr(&repeats[0])?;
            let n = Executor::value_to_repeat_count(&r_val).map_err(|e| self.span_err(span, e))?;
            return Ok(vec![n; n_targets]);
        }
        if repeats.len() == n_targets {
            let mut counts = Vec::with_capacity(n_targets);
            for r_expr in repeats.iter() {
                let r_val = self.eval_expr(r_expr)?;
                let n =
                    Executor::value_to_repeat_count(&r_val).map_err(|e| self.span_err(span, e))?;
                counts.push(n);
            }
            return Ok(counts);
        }
        Err(self.span_err(
            span,
            format!(
                "FOR repeat count list length {} does not match number of targets {}",
                repeats.len(),
                n_targets
            ),
        ))
    }

    fn value_to_repeat_count(v: &Value) -> Result<usize, String> {
        match v {
            Value::Number(n) => {
                if n.is_sign_negative() {
                    Err("repeat count must be non-negative".into())
                } else {
                    Ok(n.trunc() as usize)
                }
            }
            Value::String(s) => s
                .parse::<usize>()
                .map_err(|_| "cannot parse repeat count from string".into()),
            _ => Err("repeat count must be a number".into()),
        }
    }

    // ── Color parsing ─────────────────────────────────────────────────────────

    pub fn parse_color_arg(arg: Option<&Value>) -> (u8, u8, u8, u8) {
        if let Some(Value::String(s)) = arg {
            let s = s.trim();
            if s.starts_with('#') {
                if s.len() == 7 {
                    if let (Ok(r), Ok(g), Ok(b)) = (
                        u8::from_str_radix(&s[1..3], 16),
                        u8::from_str_radix(&s[3..5], 16),
                        u8::from_str_radix(&s[5..7], 16),
                    ) {
                        return (r, g, b, 255);
                    }
                } else if s.len() == 4 {
                    let r = u8::from_str_radix(&s[1..2].repeat(2), 16).unwrap_or(0);
                    let g = u8::from_str_radix(&s[2..3].repeat(2), 16).unwrap_or(0);
                    let b = u8::from_str_radix(&s[3..4].repeat(2), 16).unwrap_or(0);
                    return (r, g, b, 255);
                }
            }
            if s.to_lowercase().starts_with("rgb(") && s.ends_with(')') {
                let parts: Vec<&str> = s[4..s.len() - 1].split(',').map(str::trim).collect();
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

    // ── Constraint helper ─────────────────────────────────────────────────────

    pub fn expr_to_simple(&self, e: &Expr) -> ExprSimple {
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

    // ── Error helper ──────────────────────────────────────────────────────────

    pub fn span_err<T: Into<String>>(&self, span: &Span, msg: T) -> anyhow::Error {
        let mut err = RuntimeError::new(RuntimeErrorKind::SyntaxError, span.clone());
        err.message = msg.into();
        err = err.with_traceback(self.traceback.clone());
        anyhow!("{}", err)
    }

    // ── Public convenience API ────────────────────────────────────────────────

    pub fn parse(src: &str) -> crate::parser::Program {
        let tokens = crate::lexer::lexer::Lexer::new(src).lex();
        let (p, parse_diags) = crate::parser::parser::Parser::new(tokens).parse_with_diagnostics();
        for d in &parse_diags {
            eprintln!(
                "Parse error at {}:{}: {}",
                d.span.start_line, d.span.start_col, d.message
            );
        }
        p
    }

    pub fn run(src: &str) -> Result<Environment> {
        // Normalize leading whitespace so leading blank/indented lines don't parse as empty Raw expressions.
        let src = src.trim_start();
        let prog = Executor::parse(src);
        let mut ex = Executor::new();
        ex.execute_program(&prog)?;
        Ok(ex.env)
    }

    pub fn enter_shell(&mut self) -> Result<(), String> {
        crate::interpreter::shell_os::run_shell(self).map(|_| ())
    }

    /// Spawn a detached pipeline: run `left_ast` and `right_ast` in separate interpreter
    /// threads, wire them with a channel, register both threads so `:threads` lists them,
    /// and return immediately.

    /* PASTA_PIPELINE_HELPER */
    /// Spawn a detached pipeline: run `left_ast` and `right_ast` in separate interpreter
    /// threads, wire them with a channel, register both threads so `:threads` lists them,
    /// and return immediately.

    pub fn spawn_pipeline_detached(
        &mut self,
        left_ast: Box<crate::parser::ast::Expr>,
        right_ast: Box<crate::parser::ast::Expr>,
        name_hint: &str,
    ) -> anyhow::Result<()> {
        use crate::interpreter::environment::Value;
        use std::sync::mpsc;
        use std::thread;

        // 1) Create a channel for pipeline items (Value).
        let (tx, rx) = mpsc::channel::<Value>();

        // 2) Register threads in the parent environment so :threads will show them.
        // Use human-friendly names derived from name_hint.
        let left_name = Some(format!("{}-left", name_hint));
        let right_name = Some(format!("{}-right", name_hint));
        let left_tid = self.env.define_thread(left_name.clone(), 1.0);
        let right_tid = self.env.define_thread(right_name.clone(), 1.0);

        // 3) Spawn left (producer) thread
        let left_ast_clone = left_ast.clone();
        let tx_left = tx.clone();
        let left_name_clone = left_name.clone();
        let _left_handle = thread::Builder::new()
            .name(
                left_name_clone
                    .clone()
                    .unwrap_or_else(|| "pipeline-left".to_string()),
            )
            .spawn(move || {
                // Create a fresh Executor for this thread.
                let mut exec = crate::interpreter::executor::Executor::new();

                // Mirror the thread id into the per-thread environment so REPL :threads and thread-local code can see it.
                let _ = exec
                    .env
                    .define_thread_with_id(left_tid, left_name_clone.clone(), 1.0);

                // Optionally set a thread-local variable for convenience.
                crate::interpreter::ex_frame::set_local(
                    &mut exec.env,
                    "_thread_id".to_string(),
                    Value::Number(left_tid as f64),
                );

                // Evaluate the left AST once and send values.
                match crate::interpreter::ex_eval::eval_expr(&mut exec, &*left_ast_clone) {
                    Ok(val) => {
                        // If the left produced a list, stream its elements; otherwise send single value.
                        match val {
                            Value::List(items) => {
                                for item in items.into_iter() {
                                    if tx_left.send(item).is_err() {
                                        break;
                                    }
                                }
                            }
                            other => {
                                let _ = tx_left.send(other);
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("pipeline left evaluation error: {}", e);
                    }
                }
                // Drop sender to signal EOF to consumer.
                drop(tx_left);
            })?;

        // 4) Spawn right (consumer) thread
        let right_ast_clone = right_ast.clone();
        let right_name_clone = right_name.clone();
        let _right_handle = thread::Builder::new()
            .name(
                right_name_clone
                    .clone()
                    .unwrap_or_else(|| "pipeline-right".to_string()),
            )
            .spawn(move || {
                // Create a fresh Executor for this thread.
                let mut exec = crate::interpreter::executor::Executor::new();

                // Mirror the thread id into the per-thread environment.
                let _ = exec
                    .env
                    .define_thread_with_id(right_tid, right_name_clone.clone(), 1.0);

                // Set thread id local
                crate::interpreter::ex_frame::set_local(
                    &mut exec.env,
                    "_thread_id".to_string(),
                    Value::Number(right_tid as f64),
                );

                // Consume values and evaluate the right AST for each incoming item.
                while let Ok(item) = rx.recv() {
                    // Make the incoming item available as a local variable "_" for the right stage.
                    crate::interpreter::ex_frame::set_local(
                        &mut exec.env,
                        "_".to_string(),
                        item.clone(),
                    );

                    // Evaluate the right AST; ignore returned value for now.
                    if let Err(e) =
                        crate::interpreter::ex_eval::eval_expr(&mut exec, &*right_ast_clone)
                    {
                        eprintln!("pipeline right evaluation error: {}", e);
                        // continue consuming remaining items
                    }
                }

                // When channel closes, consumer exits and the thread ends.
            })?;

        // 5) Return immediately; threads run detached.
        Ok(())
    }
}

// ── Free helpers ──────────────────────────────────────────────────────────────

#[inline]
fn num_cpus_estimate() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Free-function wrapper around `Executor::spawn_pipeline_from_files` for call
/// sites that don't have an existing `Executor` (e.g. the interactive shell CLI).
pub fn spawn_pipeline_paths(stage_paths: &[&str]) -> anyhow::Result<()> {
    let mut exe = Executor::new();
    exe.spawn_pipeline_from_files(stage_paths)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod executor_error_tests {
    use super::*;

    #[test]
    fn parse_error_uses_central_messages() {
        let tokens = Lexer::new("DEF    DO END ").lex();
        let mut parser = Parser::new(tokens);
        let (_prog, diags) = parser.parse_with_diagnostics();
        assert_eq!(diags.len(), 1);
        assert_eq!(
            diags[0].message,
            crate::interpreter::errors::messages::EXPECTED_IDENTIFIER_AFTER_DEF
        );
    }

    #[test]
    fn runtime_error_includes_traceback() {
        let prog = parse("DO a WHILE 1:\n  x = 0\nEND ");
        let mut ex = Executor::new();
        ex.set_while_limit(1);
        let err = ex.execute_program(&prog).unwrap_err();
        let text = format!("{}", err);
        assert!(text.contains("exceeded iteration limit"), "got: {}", text);
        assert!(text.contains("Traceback:"));
    }
}

#[cfg(test)]
mod executor_tensor_tests {
    use super::*;

    #[test]
    fn exec_build_tensor_basic() {
        let src = "x = tensor([[1,2],[3,4]])";
        let env = run(src).unwrap();
        let v = env.get("x").expect("x should be defined");
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
        let src = "x = tensor([[1.0,2.0]])";
        let env = run(src).unwrap();
        if let Value::Tensor(t) = env.get("x").unwrap() {
            assert_eq!(t.dtype, "float32");
        } else {
            panic!("not a tensor");
        }
    }

    #[test]
    fn exec_build_tensor_ragged_error() {
        let mut ex = Executor::new();
        let prog = Executor::parse("x = tensor([[1,2],[3]])");
        assert!(
            ex.execute_program(&prog).is_err(),
            "ragged tensor should error"
        );
    }

    #[test]
    fn exec_build_tensor_non_number_error() {
        let mut ex = Executor::new();
        let prog = Executor::parse("x = tensor([[1,true],[3,4]])");
        assert!(
            ex.execute_program(&prog).is_err(),
            "non-number element should error"
        );
    }

    #[test]
    fn exec_build_tensor_multiline() {
        let src = "x = tensor([[1,2],[3,4]])";
        let env = Executor::run(src).unwrap();
        if let Value::Tensor(t) = env.get("x").unwrap() {
            assert_eq!(t.shape, vec![2, 2]);
            assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0]);
        } else {
            panic!("not a tensor");
        }
    }
}

#[cfg(test)]
mod executor_tests6 {
    use super::*;
    #[test]
    fn exec_tensor_stdlib_basic() {
        let env = run("x = tensor.zeros([2,3])\ny = tensor.ones(4)\nz = tensor.eye(3)\n").unwrap();
        if let Value::Tensor(t) = env.get("x").unwrap() {
            assert_eq!(t.shape, vec![2, 3]);
            assert!(t.data.iter().all(|&v| v == 0.0));
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("y").unwrap() {
            assert_eq!(t.shape, vec![4]);
            assert!(t.data.iter().all(|&v| v == 1.0));
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("z").unwrap() {
            assert_eq!(t.shape, vec![3, 3]);
            for i in 0..3 {
                for j in 0..3 {
                    assert_eq!(t.data[i * 3 + j], if i == j { 1.0 } else { 0.0 });
                }
            }
        } else {
            panic!();
        }
    }
}

#[cfg(test)]
mod executor_tests5 {
    use super::*;
    #[test]
    fn exec_tensor_reshape_transpose_flatten() -> anyhow::Result<()> {
        let src = "a = tensor([[1,2,3],[4,5,6]])
b = tensor.reshape(a,[3,2])
c = tensor.transpose(a)
d = tensor.flatten(a)";
        let env = run(src)?;
        if let Value::Tensor(t) = env.get("b").unwrap() {
            assert_eq!(t.shape, vec![3, 2]);
            assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("c").unwrap() {
            assert_eq!(t.shape, vec![3, 2]);
            assert_eq!(t.data, vec![1.0, 4.0, 2.0, 5.0, 3.0, 6.0]);
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("d").unwrap() {
            assert_eq!(t.shape, vec![6]);
            assert_eq!(t.data, vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        } else {
            panic!();
        }
        Ok(())
    }
}

#[cfg(test)]
mod executor_tests4 {
    use super::*;
    #[test]
    fn exec_tensor_elementwise_ops() {
        let src = "a = tensor([1,2,3,4])
b = tensor([5,6,7,8])
c = a + b
d = b - a
e = a * b";
        let env = run(src).unwrap();
        if let Value::Tensor(t) = env.get("c").unwrap() {
            assert_eq!(t.data, vec![6.0, 8.0, 10.0, 12.0]);
        } else {
            panic!("c not tensor");
        }
        if let Value::Tensor(t) = env.get("d").unwrap() {
            assert_eq!(t.data, vec![4.0, 4.0, 4.0, 4.0]);
        } else {
            panic!("d not tensor");
        }
        if let Value::Tensor(t) = env.get("e").unwrap() {
            assert_eq!(t.data, vec![5.0, 12.0, 21.0, 32.0]);
        } else {
            panic!("e not tensor");
        }
    }
}

#[cfg(test)]
mod executor_tests3 {
    use super::*;
    #[test]
    fn exec_tensor_scalar_ops() {
        let src = "a = tensor([1,2,3,4])
b = a + 1
c = a + 2
d = a * 2
e = 8 / a";
        let env = run(src).unwrap();
        if let Value::Tensor(t) = env.get("b").unwrap() {
            assert_eq!(t.data, vec![2.0, 3.0, 4.0, 5.0]);
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("c").unwrap() {
            assert_eq!(t.data, vec![3.0, 4.0, 5.0, 6.0]);
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("d").unwrap() {
            assert_eq!(t.data, vec![2.0, 4.0, 6.0, 8.0]);
        } else {
            panic!();
        }
        if let Value::Tensor(t) = env.get("e").unwrap() {
            assert_eq!(t.data, vec![8.0, 4.0, 8.0 / 3.0, 2.0]);
        } else {
            panic!();
        }
    }
}

#[cfg(test)]
mod executor_tests2 {
    use super::*;
    #[test]
    fn exec_tensor_matmul_and_errors() {
        let src = "a = tensor([[1,2],[3,4]])
b = tensor([[5,6],[7,8]])
c = tensor.matmul(a,b)";
        let env = run(src).unwrap();
        if let Value::Tensor(t) = env.get("c").unwrap() {
            assert_eq!(t.shape, vec![2, 2]);
            assert_eq!(t.data, vec![19.0, 22.0, 43.0, 50.0]);
        } else {
            panic!();
        }
    }
    #[test]
    fn exec_build_tensor_high_dim() {
        let src = "x = tensor([[[1,2],[3,4]],[[5,6],[7,8]],[[9,10],[11,12]]])";
        if let Value::Tensor(t) = run(src).unwrap().get("x").unwrap() {
            assert_eq!(t.shape, vec![3, 2, 2]);
            assert_eq!(
                t.data,
                vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0]
            );
        } else {
            panic!("not a tensor");
        }
    }
}

#[cfg(test)]
mod executor_tests {
    use super::*;
    #[test]
    fn exec_gc_basic_collection() {
        let mut ex = Executor::new();
        ex.execute_program(&parse("a = [1]\n")).unwrap();
        let before = ex.gc.allocated_count();
        ex.execute_program(&parse("a = 0\n")).unwrap();
        let after = ex.gc.allocated_count();
        assert!(after < before, "heap should shrink once 'a' is cleared");
    }
    #[test]
    fn exec_gc_retains_referenced() {
        let mut ex = Executor::new();
        ex.execute_program(&parse("a = [1]\nb = a\n")).unwrap();
        let before = ex.gc.allocated_count();
        ex.execute_program(&parse("a = 0\n")).unwrap();
        let after = ex.gc.allocated_count();
        assert_eq!(after, before, "object should remain reachable via b");
    }
}

// Convenience aliases for tests
#[allow(dead_code)]
#[allow(dead_code)]

fn run(src: &str) -> Result<Environment> {
    Executor::run(src)
}
#[allow(dead_code)]
#[allow(dead_code)]

fn parse(src: &str) -> crate::parser::Program {
    Executor::parse(src)
}

impl Executor {
    /// Spawn a pipeline where each side is a script file path.
    /// Spawns two interpreter threads, registers them in the parent env, and returns immediately.
    /// Spawn a multi-stage script pipeline (up to 8 stages) with MPSC channels
    /// connecting adjacent stages. The first stage's RETURN values are sent to
    /// stage 2; each middle stage receives values via the `PIPE_IN` variable,
    /// runs once per item, and forwards RETURN values downstream.
    ///
    /// * `stage_paths` — slice of .ps file paths, length 2–8.
    ///
    /// Returns immediately; all threads run detached.
    pub fn spawn_pipeline_from_files(&mut self, stage_paths: &[&str]) -> anyhow::Result<()> {
        use crate::interpreter::environment::Value;
        use std::fs;
        use std::sync::mpsc;
        use std::thread;

        const MAX_STAGES: usize = 8;

        let n = stage_paths.len();
        if n < 2 {
            return Err(anyhow::anyhow!("pipeline requires at least 2 stages"));
        }
        if n > MAX_STAGES {
            return Err(anyhow::anyhow!(
                "pipeline supports at most {} stages",
                MAX_STAGES
            ));
        }

        let pipeline_id = next_pipeline_id();

        // Read all source files upfront so we can report errors before spawning.
        let mut sources: Vec<String> = Vec::with_capacity(n);
        for path in stage_paths.iter() {
            let abs = if std::path::Path::new(path).is_absolute() {
                std::path::PathBuf::from(path)
            } else {
                std::env::current_dir().unwrap_or_default().join(path)
            };
            let src = fs::read_to_string(&abs).map_err(|e| {
                anyhow::anyhow!("pipeline: failed to read '{}': {}", abs.display(), e)
            })?;
            sources.push(src);
        }

        // Build N-1 channels: channel[i] connects stage[i] → stage[i+1].
        let mut senders: Vec<mpsc::SyncSender<Value>> = Vec::with_capacity(n - 1);
        let mut receivers: Vec<mpsc::Receiver<Value>> = Vec::with_capacity(n - 1);
        for _ in 0..(n - 1) {
            // Bounded to avoid unbounded memory growth; 64 items in flight per stage.
            let (tx, rx) = mpsc::sync_channel::<Value>(64);
            senders.push(tx);
            receivers.push(rx);
        }

        // Allocate thread IDs and register them in the global registry now so
        // :threads shows them immediately (even before threads start).
        let mut tids: Vec<u64> = Vec::with_capacity(n);
        for i in 0..n {
            let tid = crate::threading::threads::next_thread_id();
            let tname = format!("pipeline-{}-stage-{}", pipeline_id, i);
            let mut pt = crate::threading::threads::PastaThread::new(tid, tname.clone());
            pt.pipeline_id = Some(pipeline_id);
            pt.pipeline_stage = Some(i);
            pt.pipeline_total = Some(n);
            crate::threading::threads::register_thread(pt);
            tids.push(tid);
            // Also register in local env for backward compat.
            let _ = self.env.define_thread(Some(tname), 1.0);
        }

        // Drain receivers into a Vec<Option<…>> so we can move them into threads.
        let mut rx_opts: Vec<Option<mpsc::Receiver<Value>>> =
            receivers.into_iter().map(Some).collect();

        // Spawn stage threads.
        for i in 0..n {
            let src = sources[i].clone();
            let tid = tids[i];
            let stage_idx = i;
            let pipeline_id_c = pipeline_id;
            let thread_name = format!("pipeline-{}-stage-{}", pipeline_id, i);

            // Wire up kill channel.
            let (kill_tx, kill_rx) = std::sync::mpsc::sync_channel::<()>(1);
            crate::threading::threads::global_registry()
                .lock()
                .unwrap()
                .threads
                .get_mut(&tid)
                .map(|t| t.kill_tx = Some(kill_tx));

            // Optional downstream sender (None for last stage).
            let opt_tx: Option<mpsc::SyncSender<Value>> = if i < n - 1 {
                Some(senders[i].clone())
            } else {
                None
            };

            // Optional upstream receiver (None for first stage).
            let opt_rx: Option<mpsc::Receiver<Value>> =
                if i > 0 { rx_opts[i - 1].take() } else { None };

            thread::Builder::new().name(thread_name).spawn(move || {
                crate::interpreter::executor::set_kill_rx(kill_rx);

                // Parse the script once; we may execute it multiple times (per item).
                let prog = crate::interpreter::executor::Executor::parse(&src);

                let run_once =
                    |pipe_in: Option<Value>, tx: &Option<mpsc::SyncSender<Value>>| -> bool {
                        let mut exec = crate::interpreter::executor::Executor::new();
                        exec.pipeline_tx = tx.clone();
                        crate::interpreter::ex_frame::set_local(
                            &mut exec.env,
                            "_thread_id".to_string(),
                            Value::Number(tid as f64),
                        );
                        if let Some(val) = pipe_in {
                            exec.env.set_global("PIPE_IN".to_string(), val);
                        }
                        match exec.execute_program(&prog) {
                            Ok(_) => true,
                            Err(e) => {
                                let msg = e.to_string();
                                if msg == "thread killed" {
                                    return false; // signal: stop looping
                                }
                                eprintln!(
                                    "[pipeline-{} stage {}] error: {}",
                                    pipeline_id_c, stage_idx, msg
                                );
                                true // non-fatal: keep going
                            }
                        }
                    };

                if let Some(rx) = opt_rx {
                    // Middle / last stage: run once per incoming item.
                    for item in rx.iter() {
                        if !run_once(Some(item), &opt_tx) {
                            break; // killed
                        }
                    }
                } else {
                    // First stage: run once; RETURN sends via pipeline_tx.
                    run_once(None, &opt_tx);
                }

                // Signal completion: drop the sender so downstream drains cleanly.
                drop(opt_tx);
                crate::threading::threads::finish_thread(tid);
            })?;
        }

        Ok(())
    }
}

// STACK_OVERFLOW_DIAGNOSTICS_PATCH

use std::sync::Once;

// STACK_OVERFLOW_DIAGNOSTICS_PATCH: panic hook installer
#[allow(dead_code)]
pub fn install_stack_overflow_hook() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|info| {
            let bt = std::backtrace::Backtrace::force_capture();
            let bt_str = format!("{:?}", bt);
            // best-effort: grab recent events
            let recent = crate::interpreter::ex_eval::take_recent_events();
            let context = crate::error_logging::error_handler::DiagnosticContext {
                summary: Some(format!("panic hook captured runtime failure: {:?}", info)),
                ..crate::error_logging::error_handler::DiagnosticContext::default()
            };
            if let Ok(path) =
                crate::error_logging::error_handler::persist_diagnostic(&bt_str, recent, context)
            {
                eprintln!("Diagnostic persisted to: {}", path.display());
            } else {
                eprintln!("Failed to persist diagnostic");
            }
            eprintln!("panic info: {:?}", info);
        }));
    });
}
