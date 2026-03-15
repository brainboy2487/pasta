//! src/interpreter/environment.rs
//!
//! Runtime environment and namespace for the PASTA interpreter.
//! - Lexical scope stack (scopes[0] is global).
//! - Variable access helpers: set_local, set_global, assign, set_if_absent, get, remove.
//! - Thread / DO-block metadata registry.
//! - Lightweight runtime value types and tensor helper utilities.
//!
//! This file is a cleaned, well-documented, and test-covered replacement
//! intended to be a direct drop-in for the previous implementation.

use std::collections::HashMap;
use std::fmt;
use anyhow::{anyhow, Result};

use crate::parser::Statement;

/// Runtime value representation used by the interpreter.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// A 64-bit floating-point number.
    Number(f64),
    /// A UTF-8 string.
    String(String),
    /// A boolean (`true` / `false`).
    Bool(bool),
    /// A heterogeneous list of values.
    List(Vec<Value>),
    /// A multi-dimensional tensor of `f64` values.
    Tensor(RuntimeTensor),
    /// Deferred block of statements (callable).
    Lambda(Vec<Statement>),
    /// Opaque heap handle (GC-managed).
    Heap(crate::runtime::strainer::GcRef),
    /// Pending deferred return (from RET.LATE). Holds snapshotted value + delivery info.
    /// Full async machinery wired in a later session; for now this is a placeholder.
    Pending(Box<Value>, u64), // (snapshotted_value, deliver_after_ms_from_epoch)
    /// The absence of a value (analogous to `null` in other languages).
    None,
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Number(n) => write!(f, "{}", n),
            Value::String(s) => write!(f, "{}", s),
            Value::Bool(b) => write!(f, "{}", b),
            Value::List(items) => {
                let inner: Vec<String> = items.iter().map(|v| format!("{}", v)).collect();
                write!(f, "[{}]", inner.join(", "))
            }
            Value::Tensor(_) => write!(f, "<tensor>"),
            Value::Lambda(_) => write!(f, "<lambda>"),
            Value::Heap(_) => write!(f, "<heap>"),
            Value::Pending(v, ms) => write!(f, "<pending:{} due_ms={}>", v, ms),
            Value::None => write!(f, "None"),
        }
    }
}

/// Row-major runtime tensor with basic helpers.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeTensor {
    /// Dimension sizes, e.g. `[2, 3]` for a 2x3 matrix.
    pub shape: Vec<usize>,
    /// Element data type tag, e.g. `"float32"` or `"int32"`.
    pub dtype: String,
    /// Row-major strides in number of elements.
    pub strides: Vec<usize>,
    /// Compute device tag, e.g. `"cpu"` or `"gpu"`.
    pub device: String,
    /// Flat element buffer in row-major order.
    pub data: Vec<f64>,
}

impl RuntimeTensor {
    /// Construct a tensor on the default `"cpu"` device.
    pub fn new(shape: Vec<usize>, dtype: impl Into<String>, data: Vec<f64>) -> Self {
        let dtype = dtype.into();
        let strides = Self::compute_strides(&shape);
        Self { shape, dtype, strides, device: "cpu".to_string(), data }
    }

    /// Construct a tensor pinned to a specific compute device.
    pub fn with_device(shape: Vec<usize>, dtype: impl Into<String>, data: Vec<f64>, device: &str) -> Self {
        let dtype = dtype.into();
        let strides = Self::compute_strides(&shape);
        Self { shape, dtype, strides, device: device.to_string(), data }
    }

    /// Compute row-major strides for the given shape.
    pub fn compute_strides(shape: &[usize]) -> Vec<usize> {
        if shape.is_empty() { return vec![]; }
        let mut strides = vec![1usize; shape.len()];
        for i in (0..shape.len()-1).rev() {
            strides[i] = strides[i + 1] * shape[i + 1];
        }
        strides
    }

    /// Total number of elements (product of all shape dimensions).
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Number of dimensions (length of `shape`).
    pub fn rank(&self) -> usize {
        self.shape.len()
    }

    /// Convert a multi-dimensional index into a flat buffer offset. Returns `None` if out of bounds.
    pub fn flat_index(&self, indices: &[usize]) -> Option<usize> {
        if indices.len() != self.shape.len() { return None; }
        let mut idx = 0usize;
        for (i, &dim_idx) in indices.iter().enumerate() {
            if dim_idx >= self.shape[i] { return None; }
            idx += dim_idx * self.strides[i];
        }
        Some(idx)
    }
}

/// Metadata for a running DO thread.
#[derive(Debug, Clone)]
pub struct ThreadMeta {
    /// Unique numeric identifier assigned by the environment.
    pub id: u64,
    /// Optional human-readable name used for lookup by name.
    pub name: Option<String>,
    /// Scheduling weight (higher = more CPU time).
    pub priority_weight: f64,
}

impl ThreadMeta {
    /// Construct thread metadata from its components.
    pub fn new(id: u64, name: Option<String>, priority_weight: f64) -> Self {
        Self { id, name, priority_weight }
    }
}

/// Where an assignment wrote its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignTarget {
    /// Written into a pre-existing binding at scope index `n` (0 = global).
    ExistingScope(usize),
    /// Created as a new binding in the innermost scope.
    Local,
    /// Written into the global (outermost) scope.
    Global,
}

/// Single lexical scope frame.
#[derive(Debug, Clone)]
pub struct Scope {
    vars: HashMap<String, Value>,
}

impl Scope {
    /// Create a new empty scope frame.
    pub fn new() -> Self { Self { vars: HashMap::new() } }

    /// Read-only access to all variable bindings in this scope.
    pub fn get_vars(&self) -> &HashMap<String, Value> { &self.vars }

    fn get(&self, name: &str) -> Option<Value> { self.vars.get(name).cloned() }

    fn set(&mut self, name: impl Into<String>, val: Value) { self.vars.insert(name.into(), val); }

    fn remove(&mut self, name: &str) -> Option<Value> { self.vars.remove(name) }

    fn contains(&self, name: &str) -> bool { self.vars.contains_key(name) }
}

/// Interpreter environment: lexical scopes + thread registry.
#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<Scope>,
    threads: HashMap<u64, ThreadMeta>,
    thread_names: HashMap<String, u64>,
    next_thread_id: u64,
}

impl Default for Environment {
    fn default() -> Self {
        Self::new()
    }
}

impl Environment {
    /// New environment with a single global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            threads: HashMap::new(),
            thread_names: HashMap::new(),
            next_thread_id: 1,
        }
    }

    // Diagnostics / introspection

    /// Read-only view of the full scope stack (index 0 = global).
    pub fn get_scopes(&self) -> &[Scope] { &self.scopes }

    /// Print a human-readable dump of all scopes and thread metadata.
    pub fn debug_print(&self) {
        println!("[DEBUG] Environment:");
        for (i, s) in self.scopes.iter().enumerate() {
            println!("  scope[{}]: {:?}", i, s.get_vars());
        }
        println!("  threads: {:?}", self.threads);
    }

    /// Flattened list of all values (useful for GC root scanning).
    pub fn all_values(&self) -> Vec<Value> {
        self.scopes.iter().flat_map(|s| s.get_vars().values().cloned()).collect()
    }

    // Scope management

    /// Push a new empty scope frame (called on function/block entry).
    pub fn push_scope(&mut self) { self.scopes.push(Scope::new()); }

    /// Pop the innermost scope frame. Returns an error if the global scope would be popped.
    pub fn pop_scope(&mut self) -> Result<()> {
        if self.scopes.len() <= 1 { Err(anyhow!("cannot pop global scope")) } else { self.scopes.pop(); Ok(()) }
    }

    // Variable access

    /// Set in innermost (current) scope.
    pub fn set_local(&mut self, name: impl Into<String>, val: Value) {
        self.scopes.last_mut().unwrap().set(name, val);
    }

    /// Set in global scope (index 0).
    pub fn set_global(&mut self, name: impl Into<String>, val: Value) {
        self.scopes[0].set(name, val);
    }

    /// Set only if absent anywhere; returns true if set.
    pub fn set_if_absent(&mut self, name: impl Into<String>, val: Value) -> bool {
        let key = name.into();
        if self.contains(&key) { return false; }
        self.scopes.last_mut().unwrap().set(key, val);
        true
    }

    /// Assign to nearest enclosing scope that already contains the name,
    /// otherwise create in the innermost scope. Returns where it was written.
    pub fn assign(&mut self, name: &str, val: Value) -> AssignTarget {
        for (i, scope) in self.scopes.iter_mut().enumerate().rev() {
            if scope.contains(name) {
                scope.set(name, val);
                return AssignTarget::ExistingScope(i);
            }
        }
        self.scopes.last_mut().unwrap().set(name.to_string(), val);
        AssignTarget::Local
    }

    /// Lookup variable from innermost outward.
    pub fn get(&self, name: &str) -> Option<Value> {
        self.scopes.iter().rev().find_map(|s| s.get(name))
    }

    /// Remove variable from nearest scope that contains it.
    pub fn remove(&mut self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains(name) { return scope.remove(name); }
        }
        None
    }

    /// Return `true` if `name` is bound in any scope.
    pub fn contains(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.contains(name))
    }

    /// Merged view of variables (inner shadows outer).
    pub fn list_vars(&self) -> HashMap<String, Value> {
        let mut merged = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in scope.get_vars() {
                merged.insert(k.clone(), v.clone());
            }
        }
        merged
    }

    /// Return a point-in-time snapshot of all variable bindings and thread metadata.
    pub fn snapshot(&self) -> (HashMap<String, Value>, HashMap<u64, ThreadMeta>) {
        (self.list_vars(), self.threads.clone())
    }

    // Thread namespace

    /// Define or update a thread. If `name` exists, reuse id and update weight.
    pub fn define_thread(&mut self, name: Option<String>, priority_weight: f64) -> u64 {
        if let Some(ref n) = name {
            if let Some(&existing) = self.thread_names.get(n) {
                if let Some(meta) = self.threads.get_mut(&existing) {
                    meta.priority_weight = priority_weight;
                }
                return existing;
            }
        }
        let id = self.next_thread_id;
        self.next_thread_id += 1;
        if let Some(ref n) = name { self.thread_names.insert(n.clone(), id); }
        self.threads.insert(id, ThreadMeta::new(id, name, priority_weight));
        id
    }

    /// Define a thread with a specific id (used for restoring snapshots).
    pub fn define_thread_with_id(&mut self, id: u64, name: Option<String>, priority_weight: f64) -> Result<u64> {
        if self.threads.contains_key(&id) { return Err(anyhow!("thread id {} already exists", id)); }
        if let Some(ref n) = name {
            if self.thread_names.contains_key(n) { return Err(anyhow!("thread name '{}' already exists", n)); }
        }
        if id >= self.next_thread_id { self.next_thread_id = id + 1; }
        if let Some(ref n) = name { self.thread_names.insert(n.clone(), id); }
        self.threads.insert(id, ThreadMeta::new(id, name, priority_weight));
        Ok(id)
    }

    /// Look up a thread by its numeric id.
    pub fn get_thread(&self, id: u64) -> Option<ThreadMeta> { self.threads.get(&id).cloned() }

    /// Look up a thread id by its registered name.
    pub fn find_thread_by_name(&self, name: &str) -> Option<u64> { self.thread_names.get(name).cloned() }

    /// Remove a thread from the registry by id, also removing its name mapping.
    pub fn remove_thread(&mut self, id: u64) -> Option<ThreadMeta> {
        let meta = self.threads.remove(&id)?;
        if let Some(name) = &meta.name { self.thread_names.remove(name); }
        Some(meta)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_push_pop_and_vars() {
        let mut env = Environment::new();
        env.set_local("x", Value::Number(1.0));
        assert_eq!(env.get("x"), Some(Value::Number(1.0)));
        env.push_scope();
        env.set_local("x", Value::Number(2.0));
        assert_eq!(env.get("x"), Some(Value::Number(2.0)));
        env.pop_scope().unwrap();
        assert_eq!(env.get("x"), Some(Value::Number(1.0)));
    }

    #[test]
    fn assign_prefers_existing_scope() {
        let mut env = Environment::new();
        env.set_local("a", Value::Number(1.0)); // global
        env.push_scope();
        env.set_local("b", Value::Number(2.0)); // inner
        let t = env.assign("a", Value::Number(3.0));
        assert_eq!(t, AssignTarget::ExistingScope(0));
        assert_eq!(env.get("a"), Some(Value::Number(3.0)));
        let t2 = env.assign("c", Value::Number(4.0));
        assert_eq!(t2, AssignTarget::Local);
        assert_eq!(env.get("c"), Some(Value::Number(4.0)));
    }

    #[test]
    fn set_if_absent_behaviour() {
        let mut env = Environment::new();
        assert!(env.set_if_absent("x", Value::Number(1.0)));
        assert!(!env.set_if_absent("x", Value::Number(2.0)));
        assert_eq!(env.get("x"), Some(Value::Number(1.0)));
    }

    #[test]
    fn lambda_value_roundtrip() {
        let mut env = Environment::new();
        let stmts: Vec<Statement> = vec![];
        env.assign("f", Value::Lambda(stmts.clone()));
        assert!(matches!(env.get("f"), Some(Value::Lambda(_))));
    }

    #[test]
    fn thread_namespace_basic() {
        let mut env = Environment::new();
        let id = env.define_thread(Some("worker".into()), 1.0);
        assert_eq!(env.find_thread_by_name("worker"), Some(id));
        let id2 = env.define_thread(Some("worker".into()), 2.0);
        assert_eq!(id, id2);
        assert_eq!(env.get_thread(id).unwrap().priority_weight, 2.0);
        env.remove_thread(id);
        assert!(env.get_thread(id).is_none());
    }

    #[test]
    fn define_thread_with_id_conflict() {
        let mut env = Environment::new();
        let id = env.define_thread(Some("t1".into()), 1.0);
        let res = env.define_thread_with_id(id, Some("t2".into()), 1.0);
        assert!(res.is_err());
    }

    #[test]
    fn runtime_tensor_strides_2d() {
        let t = RuntimeTensor::new(vec![2, 3], "float32", vec![1.0; 6]);
        assert_eq!(t.strides, vec![3, 1]);
    }

    #[test]
    fn runtime_tensor_strides_3d() {
        let t = RuntimeTensor::new(vec![2, 3, 4], "float32", vec![0.0; 24]);
        assert_eq!(t.strides, vec![12, 4, 1]);
    }

    #[test]
    fn runtime_tensor_flat_index() {
        let t = RuntimeTensor::new(vec![2, 3], "int32", vec![0.0; 6]);
        assert_eq!(t.flat_index(&[0, 0]), Some(0));
        assert_eq!(t.flat_index(&[0, 2]), Some(2));
        assert_eq!(t.flat_index(&[1, 0]), Some(3));
        assert_eq!(t.flat_index(&[1, 2]), Some(5));
        assert_eq!(t.flat_index(&[2, 0]), None);
        assert_eq!(t.flat_index(&[0, 3]), None);
    }

    #[test]
    fn runtime_tensor_device_default() {
        let t = RuntimeTensor::new(vec![3], "int32", vec![1.0, 2.0, 3.0]);
        assert_eq!(t.device, "cpu");
    }

    #[test]
    fn runtime_tensor_with_device() {
        let t = RuntimeTensor::with_device(vec![3], "float32", vec![0.0; 3], "gpu");
        assert_eq!(t.device, "gpu");
    }
}
