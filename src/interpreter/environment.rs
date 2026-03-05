//! Environment and runtime namespace for the PASTA interpreter.
//!
//! This module provides:
//! - A stack of lexical scopes with clear semantics for `set_local`, `assign`, and `set_global`.
//! - Thread / DO-block metadata registry with name -> id mapping.
//! - Utility helpers for snapshots, listing variables, and safe scope push/pop.

use std::collections::HashMap;
use anyhow::{anyhow, Result};

use crate::parser::ast::Statement;

/// A runtime value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Number(f64),
    String(String),
    Bool(bool),
    List(Vec<Value>),
    Tensor(RuntimeTensor),

    /// A deferred block of statements — a first-class callable.
    Lambda(Vec<Statement>),

    /// A handle to a heap‑allocated object managed by the garbage collector.
    /// The referenced value is owned by the [`Strainer`] instance and can
    /// be inspected by asking the executor to `deref` the handle.  This is
    /// the core mechanism by which strings, lists, tensors, lambdas, and
    /// eventually user‑defined objects live on the GC heap.
    Heap(crate::runtime::strainer::GcRef),

    None,
}

/// A runtime tensor representation.
/// Stores data in row-major format with shape, dtype, strides, and device.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeTensor {
    /// Shape of the tensor (e.g., [2, 3] for a 2x3 matrix)
    pub shape: Vec<usize>,
    /// Data type: "float32" or "int32"
    pub dtype: String,
    /// Row-major strides (auto-computed from shape)
    pub strides: Vec<usize>,
    /// Device: "cpu" (gpu planned for future)
    pub device: String,
    /// Flat data storage (row-major)
    pub data: Vec<f64>,
}

impl RuntimeTensor {
    /// Create a new tensor. Strides are computed automatically from shape.
    pub fn new(shape: Vec<usize>, dtype: String, data: Vec<f64>) -> Self {
        let strides = Self::compute_strides(&shape);
        Self { shape, dtype, strides, device: "cpu".to_string(), data }
    }

    /// Create a tensor with an explicit device string (e.g. "cpu").
    pub fn with_device(shape: Vec<usize>, dtype: String, data: Vec<f64>, device: &str) -> Self {
        let strides = Self::compute_strides(&shape);
        Self { shape, dtype, strides, device: device.to_string(), data }
    }

    /// Compute row-major strides for the given shape.
    ///
    /// For shape [d0, d1, d2]:
    ///   strides[2] = 1
    ///   strides[1] = d2
    ///   strides[0] = d2 * d1
    pub fn compute_strides(shape: &[usize]) -> Vec<usize> {
        let n = shape.len();
        if n == 0 {
            return vec![];
        }
        let mut strides = vec![1usize; n];
        for i in (0..n.saturating_sub(1)).rev() {
            strides[i] = strides[i + 1] * shape[i + 1];
        }
        strides
    }

    /// Get the total number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }

    /// Get the rank (number of dimensions).
    pub fn rank(&self) -> usize {
        self.shape.len()
    }

    /// Get the flat index for a multi-dimensional index.
    /// Returns None if indices are out of bounds or wrong length.
    pub fn flat_index(&self, indices: &[usize]) -> Option<usize> {
        if indices.len() != self.shape.len() {
            return None;
        }
        let mut idx = 0;
        for (i, (&dim_idx, (&stride, &dim_size))) in indices
            .iter()
            .zip(self.strides.iter().zip(self.shape.iter()))
            .enumerate()
        {
            let _ = i;
            if dim_idx >= dim_size {
                return None;
            }
            idx += dim_idx * stride;
        }
        Some(idx)
    }
}


impl From<f64> for Value {
    fn from(n: f64) -> Self { Value::Number(n) }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self { Value::String(s.to_string()) }
}

/// Metadata for a thread or DO block.
#[derive(Debug, Clone)]
pub struct ThreadMeta {
    pub id: u64,
    pub name: Option<String>,
    pub priority_weight: f64,
}

impl ThreadMeta {
    pub fn new(id: u64, name: Option<String>, priority_weight: f64) -> Self {
        Self { id, name, priority_weight }
    }
}

/// Where an assignment wrote the value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssignTarget {
    /// Written into an existing outer scope (index from 0 = global).
    ExistingScope(usize),
    /// Written into the innermost (current) scope.
    Local,
    /// Written into the global scope (index 0).
    Global,
}

/// A single lexical scope frame.
#[derive(Debug, Clone)]
pub struct Scope {
    pub vars: HashMap<String, Value>,
}

impl Scope {
        /// Public getter for vars (for diagnostics only)
        pub fn get_vars(&self) -> &HashMap<String, Value> {
            &self.vars
        }
    fn new() -> Self { Self { vars: HashMap::new() } }
    fn get(&self, name: &str) -> Option<Value> { self.vars.get(name).cloned() }
    fn set(&mut self, name: impl Into<String>, val: Value) { self.vars.insert(name.into(), val); }
    fn remove(&mut self, name: &str) -> Option<Value> { self.vars.remove(name) }
    fn contains(&self, name: &str) -> bool { self.vars.contains_key(name) }
}

/// The interpreter environment: stacked scopes + thread namespace.
#[derive(Debug, Clone)]
pub struct Environment {
    scopes: Vec<Scope>,                     // 0 = global, last = current
    threads: HashMap<u64, ThreadMeta>,     // id -> meta
    thread_names: HashMap<String, u64>,    // name -> id
    next_thread_id: u64,
}

impl Environment {
            pub fn debug_print(&self) {
                println!("[DEBUG] Environment scopes:");
                for (i, scope) in self.get_scopes().iter().enumerate() {
                    println!("  Scope {}: {:?}", i, scope.get_vars());
                }
                println!("[DEBUG] Threads: {:?}", self.threads);
            }
        /// Public getter for scopes (for diagnostics only)
        pub fn get_scopes(&self) -> &Vec<Scope> {
            &self.scopes
        }
    /// Create a new environment with a single global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            threads: HashMap::new(),
            thread_names: HashMap::new(),
            next_thread_id: 1,
        }
    }

    /// Return a flattened list of every `Value` currently stored in all
    /// lexical scopes (global and local).  This is useful for GC root
    /// scanning because the executor can call this method, hand the
    /// returned values to the `Strainer`, and let the collector figure out
    /// which heap objects are reachable.
    pub fn all_values(&self) -> Vec<Value> {
        self.scopes.iter().flat_map(|s| s.vars.values().cloned()).collect()
    }

    // ── Scope management ────────────────────────────────────────────────────

    /// Push a fresh, empty lexical scope.
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Pop the innermost scope. Returns Err if attempting to pop the global scope.
    pub fn pop_scope(&mut self) -> Result<()> {
        if self.scopes.len() <= 1 {
            Err(anyhow!("cannot pop global scope"))
        } else {
            self.scopes.pop();
            Ok(())
        }
    }

    /// Set a variable in the current (innermost) scope.
    pub fn set_local(&mut self, name: impl Into<String>, val: Value) {
        let idx = self.scopes.len() - 1;
        self.scopes[idx].set(name, val);
    }

    /// Set a variable in the global scope (scope index 0).
    pub fn set_global(&mut self, name: impl Into<String>, val: Value) {
        self.scopes[0].set(name, val);
    }

    /// Set a variable only if it does not already exist in any scope.
    /// Returns true if the variable was set, false if it already existed.
    pub fn set_if_absent(&mut self, name: impl Into<String>, val: Value) -> bool {
        let key = name.into();
        if self.contains(&key) {
            false
        } else {
            let idx = self.scopes.len() - 1;
            self.scopes[idx].set(key, val);
            true
        }
    }

    /// Assign to the nearest enclosing scope that already contains the name,
    /// or create it in the innermost scope if not found.
    ///
    /// Returns an `AssignTarget` indicating where the value was written.
    pub fn assign(&mut self, name: &str, val: Value) -> AssignTarget {
        // Search from innermost to outermost
        for (i, scope) in self.scopes.iter_mut().enumerate().rev() {
            if scope.contains(name) {
                scope.set(name.to_string(), val);
                return AssignTarget::ExistingScope(i);
            }
        }
        // Not found: write into innermost scope
        let idx = self.scopes.len() - 1;
        self.scopes[idx].set(name.to_string(), val);
        AssignTarget::Local
    }

    /// Get a variable by searching from innermost scope outward.
    pub fn get(&self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter().rev() {
            if let Some(v) = scope.get(name) { return Some(v); }
        }
        None
    }

    /// Remove a variable from the nearest scope that contains it.
    pub fn remove(&mut self, name: &str) -> Option<Value> {
        for scope in self.scopes.iter_mut().rev() {
            if scope.contains(name) { return scope.remove(name); }
        }
        None
    }

    /// Check whether a variable exists in any scope.
    pub fn contains(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.contains(name) { return true; }
        }
        false
    }

    /// Return a merged view of variables (outer scopes overwritten by inner scopes).
    pub fn list_vars(&self) -> HashMap<String, Value> {
        let mut merged = HashMap::new();
        for scope in &self.scopes {
            for (k, v) in &scope.vars { merged.insert(k.clone(), v.clone()); }
        }
        merged
    }

    /// Snapshot of current variables and thread metadata.
    pub fn snapshot(&self) -> (HashMap<String, Value>, HashMap<u64, ThreadMeta>) {
        (self.list_vars(), self.threads.clone())
    }

    // ── Thread namespace ──────────────────────────────────────────────────────

    /// Define a new thread with an optional name and priority weight.
    /// Returns the assigned thread id.
    pub fn define_thread(&mut self, name: Option<String>, priority_weight: f64) -> u64 {
        // If a name is provided and already exists, reuse the id (idempotent).
        if let Some(ref n) = name {
            if let Some(&existing) = self.thread_names.get(n) {
                // Update priority weight if desired
                if let Some(meta) = self.threads.get_mut(&existing) {
                    meta.priority_weight = priority_weight;
                }
                return existing;
            }
        }

        let id = self.next_thread_id;
        self.next_thread_id += 1;
        let meta = ThreadMeta::new(id, name.clone(), priority_weight);
        if let Some(n) = name.clone() { self.thread_names.insert(n, id); }
        self.threads.insert(id, meta);
        id
    }

    /// Define a thread with a specific id (useful for restoring snapshots).
    /// Returns Err if id already exists.
    pub fn define_thread_with_id(&mut self, id: u64, name: Option<String>, priority_weight: f64) -> Result<u64> {
        if self.threads.contains_key(&id) {
            return Err(anyhow!("thread id {} already exists", id));
        }
        if let Some(ref n) = name {
            if self.thread_names.contains_key(n) {
                return Err(anyhow!("thread name '{}' already exists", n));
            }
        }
        if id >= self.next_thread_id {
            self.next_thread_id = id + 1;
        }
        let meta = ThreadMeta::new(id, name.clone(), priority_weight);
        if let Some(n) = name.clone() { self.thread_names.insert(n, id); }
        self.threads.insert(id, meta);
        Ok(id)
    }

    /// Get a thread meta by id.
    pub fn get_thread(&self, id: u64) -> Option<ThreadMeta> { self.threads.get(&id).cloned() }

    /// Find a thread id by name.
    pub fn find_thread_by_name(&self, name: &str) -> Option<u64> {
        self.thread_names.get(name).cloned()
    }

    /// Remove a thread by id, returning its metadata if present.
    pub fn remove_thread(&mut self, id: u64) -> Option<ThreadMeta> {
        if let Some(meta) = self.threads.remove(&id) {
            if let Some(name) = &meta.name { self.thread_names.remove(name); }
            Some(meta)
        } else { None }
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
        // assign to existing 'a' should update global (ExistingScope(0))
        let t = env.assign("a", Value::Number(3.0));
        assert_eq!(t, AssignTarget::ExistingScope(0));
        assert_eq!(env.get("a"), Some(Value::Number(3.0)));
        // assign to new name writes to local
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
        // defining same name returns same id (idempotent)
        let id2 = env.define_thread(Some("worker".into()), 2.0);
        assert_eq!(id, id2);
        // priority weight updated
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
        let t = RuntimeTensor::new(vec![2, 3], "float32".to_string(), vec![1.0; 6]);
        // row-major: stride[0] = 3, stride[1] = 1
        assert_eq!(t.strides, vec![3, 1]);
    }

    #[test]
    fn runtime_tensor_strides_3d() {
        let t = RuntimeTensor::new(vec![2, 3, 4], "float32".to_string(), vec![0.0; 24]);
        assert_eq!(t.strides, vec![12, 4, 1]);
    }

    #[test]
    fn runtime_tensor_flat_index() {
        let t = RuntimeTensor::new(vec![2, 3], "int32".to_string(), vec![0.0; 6]);
        assert_eq!(t.flat_index(&[0, 0]), Some(0));
        assert_eq!(t.flat_index(&[0, 2]), Some(2));
        assert_eq!(t.flat_index(&[1, 0]), Some(3));
        assert_eq!(t.flat_index(&[1, 2]), Some(5));
        // out of bounds
        assert_eq!(t.flat_index(&[2, 0]), None);
        assert_eq!(t.flat_index(&[0, 3]), None);
    }

    #[test]
    fn runtime_tensor_device_default() {
        let t = RuntimeTensor::new(vec![3], "int32".to_string(), vec![1.0, 2.0, 3.0]);
        assert_eq!(t.device, "cpu");
    }

    #[test]
    fn runtime_tensor_with_device() {
        let t = RuntimeTensor::with_device(vec![3], "float32".to_string(), vec![0.0; 3], "gpu");
        assert_eq!(t.device, "gpu");
    }
}
