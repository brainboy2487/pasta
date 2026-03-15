// src/mod_loader/mod_api.rs
//! Public API for the PASTA module loader.
//! Keep this file stable; implementation details belong in mod_load.rs.

use std::path::PathBuf;
use std::time::SystemTime;
use anyhow::Result;
use crate::interpreter::environment::Value;

/// How the loader matched a module path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchKind {
    Exact,
    Fuzzy { score: u8 },
    Canonicalized,
}

/// Metadata about a loaded module.
#[derive(Debug, Clone)]
pub struct ModuleMeta {
    pub canonical_path: PathBuf,
    pub loaded_at: SystemTime,
    pub source_hash: Option<String>,
    pub match_kind: MatchKind,
}

/// Public representation of a loaded module.
/// `exports` maps exported symbol names to runtime Values.
#[derive(Debug, Clone)]
pub struct Module {
    pub meta: ModuleMeta,
    pub exports: std::collections::HashMap<String, Value>,
}

/// Minimal view returned when resolving a symbol.
#[derive(Debug, Clone)]
pub struct ResolvedSymbol {
    pub value: Value,
    pub module_meta: ModuleMeta,
}

/// Loader configuration exposed to callers (read-only).
#[derive(Debug, Clone)]
pub struct LoaderConfig {
    pub version: String,
    pub mode: String,
    pub fuzz_match: u8,
    pub caching: bool,
    pub load_type: String,
    // Add other read-only fields as needed
}

/// Trait describing the module loader behavior.
/// Implementations must be thread-safe if used concurrently.
pub trait ModuleLoaderApi: Send + Sync {
    /// Return a read-only snapshot of the effective loader config.
    fn config(&self) -> LoaderConfig;

    /// Register a list of module paths from a USE block.
    /// Paths may be relative or fuzzy names; registration does not necessarily load.
    fn register_use_block(&mut self, module_paths: Vec<String>) -> Result<()>;

    /// Ensure the named module is loaded and return its Module.
    /// The `module_key` may be a canonical path or a fuzzy name.
    fn ensure_loaded(&mut self, module_key: &str) -> Result<Module>;

    /// Resolve a single symbol from a module, loading the module if necessary.
    fn resolve_symbol(&mut self, module_key: &str, symbol: &str) -> Result<ResolvedSymbol>;

    /// Try to fuzzy-find a module path by name without loading it.
    fn fuzzy_find(&self, name: &str) -> Option<(PathBuf, MatchKind)>;

    /// Clear the loader cache (unload modules). Useful for tests and reloads.
    fn clear_cache(&mut self);

    /// List registered modules and whether they are loaded.
    fn list_registered(&self) -> Vec<(String, bool)>;
}

/// Factory to create the default loader implementation.
/// The concrete type implements ModuleLoaderApi and is returned as a boxed trait object.
pub fn default_loader_with_config(config_path: Option<&std::path::Path>) -> Box<dyn ModuleLoaderApi>;
