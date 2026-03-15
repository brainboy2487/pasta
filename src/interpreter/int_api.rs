// src/interpreter/int_api.rs
//! Interpreter API surface and lightweight default implementations.
//!
//! Exposes:
//!  - `InterpreterApi` (trait)
//!  - `InterpreterSnapshot` (struct)
//!  - `ModuleEnvHandle` (struct)
//!  - `ModuleEnvOps` (trait)
//!  - `default_interpreter_api()` (constructor for a fallback API impl)

use std::path::PathBuf;
use std::sync::{Arc, Mutex, Weak};
use anyhow::{Result, anyhow};
use std::collections::HashMap;

use crate::interpreter::environment::{Environment, Value};
use crate::interpreter::Executor;

/// Snapshot of interpreter state returned by `InterpreterApi::snapshot`.
#[derive(Clone, Debug)]
pub struct InterpreterSnapshot {
    pub cwd: PathBuf,
    pub globals: HashMap<String, Value>,
}

/// Public trait describing the minimal interpreter API surface used by
/// external consumers (e.g., embedding code, tests).
pub trait InterpreterApi: Send + Sync {
    fn snapshot(&self) -> InterpreterSnapshot;
    fn create_module_env(&self, canonical_path: PathBuf) -> Result<ModuleEnvHandle>;
    fn bind_global(&self, name: &str, val: Value) -> Result<()>;
    fn bind_local(&self, name: &str, val: Value) -> Result<()>;
    fn call_value(&self, callable: Value, args: Vec<Value>) -> Result<Value>;
}

/// Trait for operations on a module environment. Implementations are thin
/// adapters around an `Environment` instance.
pub trait ModuleEnvOps: Send + Sync {
    fn get_symbol(&self, name: &str) -> Option<Value>;
    fn set_symbol(&self, name: &str, val: Value);
    fn execute_top_level(&self, source: &str) -> Result<()>;
}

/// Handle returned to callers that represents a module environment.
#[derive(Clone)]
pub struct ModuleEnvHandle {
    pub canonical_path: PathBuf,
    pub inner: Arc<dyn ModuleEnvOps>,
}

impl ModuleEnvHandle {
    pub fn new(canonical_path: PathBuf, inner: Arc<dyn ModuleEnvOps>) -> Self {
        Self { canonical_path, inner }
    }
}

/// Default interpreter API: a minimal, safe implementation that returns
/// benign defaults and errors for unimplemented operations.
pub fn default_interpreter_api() -> Box<dyn InterpreterApi> {
    Box::new(DummyInterpreterApi::new())
}

struct DummyInterpreterApi {}

impl DummyInterpreterApi {
    fn new() -> Self { Self {} }
}

impl InterpreterApi for DummyInterpreterApi {
    fn snapshot(&self) -> InterpreterSnapshot {
        InterpreterSnapshot {
            cwd: PathBuf::from("."),
            globals: HashMap::new(),
        }
    }

    fn create_module_env(&self, _canonical_path: PathBuf) -> Result<ModuleEnvHandle> {
        Err(anyhow!("default_interpreter_api: create_module_env not implemented"))
    }

    fn bind_global(&self, _name: &str, _val: Value) -> Result<()> {
        Err(anyhow!("default_interpreter_api: bind_global not implemented"))
    }

    fn bind_local(&self, _name: &str, _val: Value) -> Result<()> {
        Err(anyhow!("default_interpreter_api: bind_local not implemented"))
    }

    fn call_value(&self, _callable: Value, _args: Vec<Value>) -> Result<Value> {
        Err(anyhow!("default_interpreter_api: call_value not implemented"))
    }
}

// -----------------------------------------------------------------------------
// Thin ModuleEnvOps implementation and Executor wrapper
// -----------------------------------------------------------------------------

/// Thin ModuleEnvOps implementation that delegates to an Environment instance.
#[derive(Clone)]
pub struct ModuleEnvOpsImpl {
    pub env: Arc<Mutex<Environment>>,
}

impl ModuleEnvOpsImpl {
    pub fn new(env: Arc<Mutex<Environment>>) -> Self {
        Self { env }
    }
}

impl ModuleEnvOps for ModuleEnvOpsImpl {
    fn get_symbol(&self, name: &str) -> Option<Value> {
        let e = self.env.lock().unwrap();
        e.get(name)
    }

    fn set_symbol(&self, name: &str, val: Value) {
        // Environment does not expose `set` — use set_local to bind in the module scope.
        let mut e = self.env.lock().unwrap();
        e.set_local(name.to_string(), val);
    }

    fn execute_top_level(&self, _source: &str) -> Result<()> {
        Err(anyhow!("ModuleEnvOpsImpl::execute_top_level not implemented; wire to executor"))
    }
}

/// Wrapper that implements `InterpreterApi` by delegating to a running `Executor`.
pub struct ExecutorApiWrapper {
    exe_weak: Weak<Mutex<Executor>>,
}

impl ExecutorApiWrapper {
    pub fn new(exe_arc: &Arc<Mutex<Executor>>) -> Self {
        Self { exe_weak: Arc::downgrade(exe_arc) }
    }

    fn with_executor<F, R>(&self, f: F) -> Result<R>
    where
        F: FnOnce(&mut Executor) -> Result<R>
    {
        if let Some(exe_arc) = self.exe_weak.upgrade() {
            let mut exe = exe_arc.lock().unwrap();
            f(&mut *exe)
        } else {
            Err(anyhow!("executor has been dropped"))
        }
    }
}

impl InterpreterApi for ExecutorApiWrapper {
    fn snapshot(&self) -> InterpreterSnapshot {
        if let Ok(res) = self.with_executor(|exe| {
            // Conservative snapshot: try to read a cwd string from env if present,
            // otherwise fall back to ".". We do not assume any helper that returns
            // a globals HashMap exists on Executor; return an empty map to be safe.
            let cwd = exe.env.get("__cwd__").and_then(|v| match v {
                Value::String(s) => Some(PathBuf::from(s)),
                _ => None,
            }).unwrap_or_else(|| PathBuf::from("."));
            let globals: HashMap<String, Value> = HashMap::new();
            Ok(InterpreterSnapshot { cwd, globals })
        }) {
            res
        } else {
            InterpreterSnapshot { cwd: PathBuf::from("."), globals: HashMap::new() }
        }
    }

    fn create_module_env(&self, _canonical_path: PathBuf) -> Result<ModuleEnvHandle> {
        self.with_executor(|_exe| {
            Err(anyhow!("ExecutorApiWrapper::create_module_env not implemented; executor must provide environment allocation"))
        })
    }

    fn bind_global(&self, _name: &str, _val: Value) -> Result<()> {
        self.with_executor(|_exe| {
            Err(anyhow!("ExecutorApiWrapper::bind_global not implemented; executor must provide binding helper"))
        })
    }

    fn bind_local(&self, _name: &str, _val: Value) -> Result<()> {
        self.with_executor(|_exe| {
            Err(anyhow!("ExecutorApiWrapper::bind_local not implemented; executor must provide binding helper"))
        })
    }

    fn call_value(&self, _callable: Value, _args: Vec<Value>) -> Result<Value> {
        self.with_executor(|_exe| {
            Err(anyhow!("ExecutorApiWrapper::call_value not implemented; executor must provide call helper"))
        })
    }
}
