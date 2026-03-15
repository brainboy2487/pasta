// src/semantics/resolver.rs
//! Name resolution and basic semantic analysis for PASTA.
//!
//! Responsibilities:
//! - Maintain symbol tables (scoped).
//! - Register variables, thread names, model names, etc.
//! - Detect duplicate definitions.
//! - Provide lookup APIs for parser and runtime planning.
//! - Perform early semantic checks before constraint solving or scheduling.
//!
//! This module is intentionally minimal until the full AST is implemented.
//! It provides a stable API and structure for future expansion.

use std::collections::HashMap;
use anyhow::{anyhow, Result};

/// Represents the type or category of a symbol.
///
/// Covers runtime namespaces added by the stdlib expansion:
/// sys.*, time.*, rand.*, math.*, fs.*, gc.*, debug.*,
/// memory.*, net.*, ffi.*, thread.*, device.*, tensor.*
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SymbolType {
    /// A local or global variable binding.
    Variable,
    /// A named DO thread.
    Thread,
    /// A trained ML model handle.
    Model,
    /// An object family class.
    Class,
    /// A user-defined function.
    Function,
    // ── stdlib namespaces ────────────────────────────────────────────
    /// sys.env, sys.exit, sys.platform, …
    SysBuiltin,
    /// time.now, time.sleep, …
    TimeBuiltin,
    /// rand.int, rand.choice, …
    RandBuiltin,
    /// math.sin, math.cos, math.pi, …
    MathBuiltin,
    /// fs.read, fs.write, fs.exists, …
    FsBuiltin,
    /// gc.collect, gc.stats, …
    GcBuiltin,
    /// debug.print, debug.assert, …
    DebugBuiltin,
    /// memory.alloc, memory.free, …
    MemoryBuiltin,
    /// net.get, net.post, …  (stub)
    NetBuiltin,
    /// ffi.load, ffi.call, … (stub)
    FfiBuiltin,
    /// thread.spawn, thread.join, …
    ThreadBuiltin,
    /// device.arch, device.cores, …
    DeviceBuiltin,
    /// tensor.zeros, tensor.eye, …
    TensorBuiltin,
    /// Symbol kind could not be determined at analysis time.
    Unknown,
}

/// A single symbol entry in the resolver.
#[derive(Debug, Clone)]
pub struct Symbol {
    /// The symbol's identifier string.
    pub name: String,
    /// Resolved symbol kind.
    pub ty: SymbolType,
    /// 1-based source line of the definition.
    pub line: usize,
    /// 1-based source column of the definition.
    pub col: usize,
}

/// A scope frame in the resolver.
#[derive(Debug)]
struct Scope {
    symbols: HashMap<String, Symbol>,
}

impl Scope {
    fn new() -> Self {
        Self {
            symbols: HashMap::new(),
        }
    }

    fn insert(&mut self, sym: Symbol) -> Result<()> {
        if self.symbols.contains_key(&sym.name) {
            return Err(anyhow!(
                "Duplicate symbol '{}' defined at line {}",
                sym.name,
                sym.line
            ));
        }
        self.symbols.insert(sym.name.clone(), sym);
        Ok(())
    }

    fn get(&self, name: &str) -> Option<&Symbol> {
        self.symbols.get(name)
    }
}

/// The main resolver: manages scopes and performs semantic checks.
pub struct Resolver {
    scopes: Vec<Scope>,
    diagnostics: Vec<String>,
}

impl Resolver {
    /// Create a new resolver with a global scope.
    pub fn new() -> Self {
        Self {
            scopes: vec![Scope::new()],
            diagnostics: Vec::new(),
        }
    }

    /// Push a new scope (e.g., entering a block).
    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    /// Pop the current scope.
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// Register a symbol in the current scope.
    pub fn define(
        &mut self,
        name: impl Into<String>,
        ty: SymbolType,
        line: usize,
        col: usize,
    ) -> Result<()> {
        let sym = Symbol {
            name: name.into(),
            ty,
            line,
            col,
        };
        let scope = self.scopes.last_mut().unwrap();
        scope.insert(sym)
    }

    /// Look up a symbol by name, searching from innermost to outermost scope.
    pub fn lookup(&self, name: &str) -> Option<&Symbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.get(name) {
                return Some(sym);
            }
        }
        None
    }

    /// Record a diagnostic message.
    pub fn error(&mut self, msg: impl Into<String>) {
        self.diagnostics.push(msg.into());
    }

    /// Retrieve all diagnostics.
    pub fn diagnostics(&self) -> &[String] {
        &self.diagnostics
    }

    /// True if any diagnostics have been recorded.
    pub fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Placeholder: resolve an AST (once AST is implemented).
    ///
    /// For now, this simply returns Ok.
    pub fn resolve(&self) {
        // Future:
        // - Walk AST
        // - Resolve identifiers
        // - Validate thread names
        // - Validate model references
        // - Validate DO X OVER Y references
        // - Validate LEARN blocks
        // - Validate class definitions
        // - Validate function signatures
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn define_and_lookup() {
        let mut r = Resolver::new();
        r.define("x", SymbolType::Variable, 1, 1).unwrap();
        let sym = r.lookup("x").unwrap();
        assert_eq!(sym.name, "x");
        assert_eq!(sym.ty, SymbolType::Variable);
    }

    #[test]
    fn duplicate_symbol_error() {
        let mut r = Resolver::new();
        r.define("x", SymbolType::Variable, 1, 1).unwrap();
        let err = r.define("x", SymbolType::Variable, 2, 1).unwrap_err();
        assert!(err.to_string().contains("Duplicate symbol"));
    }

    #[test]
    fn nested_scopes() {
        let mut r = Resolver::new();
        r.define("x", SymbolType::Variable, 1, 1).unwrap();

        r.push_scope();
        r.define("y", SymbolType::Variable, 2, 1).unwrap();

        assert!(r.lookup("y").is_some());
        assert!(r.lookup("x").is_some());

        r.pop_scope();
        assert!(r.lookup("y").is_none());
        assert!(r.lookup("x").is_some());
    }
}
