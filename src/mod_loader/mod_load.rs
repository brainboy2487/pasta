// src/mod_loader/mod_load.rs
// Concrete ModuleLoader implementation that wires to the public API in mod_api.rs
// This file implements the ModuleLoaderApi trait and provides a default loader factory.
//
// Notes:
// - The implementation is intentionally conservative and focused on a clear, testable API.
// - Core behaviors (parsing/executing module source, fuzzy scoring, sandboxing) are left
//   as TODOs and small, safe placeholders so the loader compiles and can be integrated
//   with the executor immediately. Implement incrementally behind these placeholders.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use anyhow::{Result, anyhow};
use std::fs;

use crate::interpreter::environment::Value;
use crate::mod_loader::mod_api::{
    Module as ApiModule, ModuleMeta as ApiModuleMeta, ResolvedSymbol, LoaderConfig as ApiLoaderConfig,
    MatchKind, ModuleLoaderApi,
};

/// Internal configuration type (parsing helpers and defaults).
#[derive(Debug, Clone)]
pub struct ModLoaderConfig {
    pub version: String,
    pub mode: Mode,
    pub tree_depth: i32,
    pub load_type: LoadType,
    pub caching: bool,
    pub load_state: LoadState,
    pub fuzz_match: u8, // 50-100
    pub search_paths: Vec<PathBuf>,
    pub allowed_paths: Vec<PathBuf>,
    pub verbose: bool,
    pub sandbox_io: bool,
    pub max_module_size_kb: usize,
    pub reload_on_change: bool,
    pub export_policy: ExportPolicy,
    pub symbol_aliasing: bool,
    pub auto_use_on_from: bool,
    pub strict_name_normalization: bool,
    pub log_level: LogLevel,
}

#[derive(Debug, Clone)]
pub enum Mode { Normal, Strict, Fuzzy }
#[derive(Debug, Clone)]
pub enum LoadType { Normal, Lazy }
#[derive(Debug, Clone)]
pub enum LoadState { Once, Continuous, Called }
#[derive(Debug, Clone)]
pub enum ExportPolicy { Explicit, All, None }
#[derive(Debug, Clone)]
pub enum LogLevel { Debug, Info, Warn, Error, None }

impl Default for ModLoaderConfig {
    fn default() -> Self {
        Self {
            version: "1.1".to_string(),
            mode: Mode::Fuzzy,
            tree_depth: 0,
            load_type: LoadType::Lazy,
            caching: true,
            load_state: LoadState::Once,
            fuzz_match: 75,
            search_paths: vec![PathBuf::from("src/stdlib"), PathBuf::from("stdlib")],
            allowed_paths: vec![PathBuf::from("."), PathBuf::from("src"), PathBuf::from("libs")],
            verbose: false,
            sandbox_io: false,
            max_module_size_kb: 512,
            reload_on_change: false,
            export_policy: ExportPolicy::Explicit,
            symbol_aliasing: true,
            auto_use_on_from: true,
            strict_name_normalization: false,
            log_level: LogLevel::Info,
        }
    }
}

/// Internal module entry stored in the registry.
#[derive(Debug)]
struct ModuleEntry {
    pub requested_key: String, // the key used to register (may be fuzzy)
    pub path: PathBuf,         // canonical or best-effort path
    pub loaded: bool,
    pub module: Option<ApiModule>,
    pub registered_at: SystemTime,
}

/// Concrete loader implementation.
/// Implements the public ModuleLoaderApi trait (see mod_api.rs).
pub struct ModuleLoader {
    config: ModLoaderConfig,
    registry: HashMap<String, ModuleEntry>, // keyed by requested_key
}

impl ModuleLoader {
    /// Construct a new ModuleLoader with the given config.
    pub fn new(config: ModLoaderConfig) -> Self {
        Self {
            config,
            registry: HashMap::new(),
        }
    }

    /// Convert internal ModLoaderConfig into the public LoaderConfig snapshot.
    fn to_public_config(&self) -> ApiLoaderConfig {
        ApiLoaderConfig {
            version: self.config.version.clone(),
            mode: match &self.config.mode {
                Mode::Normal => "Normal".to_string(),
                Mode::Strict => "Strict".to_string(),
                Mode::Fuzzy => "Fuzzy".to_string(),
            },
            fuzz_match: self.config.fuzz_match,
            caching: self.config.caching,
            load_type: match &self.config.load_type {
                LoadType::Normal => "Normal".to_string(),
                LoadType::Lazy => "Lazy".to_string(),
            },
        }
    }

    /// Internal helper: attempt to canonicalize a candidate path.
    fn canonicalize_candidate(&self, candidate: &Path) -> Option<PathBuf> {
        match fs::canonicalize(candidate) {
            Ok(p) => Some(p),
            Err(_) => None,
        }
    }

    /// Internal helper: create an ApiModule with empty exports (placeholder).
    fn make_empty_module(&self, canonical_path: PathBuf, match_kind: MatchKind) -> ApiModule {
        let meta = ApiModuleMeta {
            canonical_path,
            loaded_at: SystemTime::now(),
            source_hash: None,
            match_kind,
        };
        ApiModule { meta, exports: HashMap::new() }
    }

    /// Attempt a simple exact resolution: if the requested key is registered, return it.
    fn lookup_registered(&self, key: &str) -> Option<&ModuleEntry> {
        self.registry.get(key)
    }

    /// Simple helper to register a canonicalized path (internal).
    fn register_canonical(&mut self, requested_key: String, canonical: PathBuf) {
        let entry = ModuleEntry {
            requested_key: requested_key.clone(),
            path: canonical,
            loaded: false,
            module: None,
            registered_at: SystemTime::now(),
        };
        self.registry.insert(requested_key, entry);
    }
}

/// Implement the public ModuleLoaderApi trait for ModuleLoader.
impl ModuleLoaderApi for ModuleLoader {
    fn config(&self) -> ApiLoaderConfig {
        self.to_public_config()
    }

    fn register_use_block(&mut self, module_paths: Vec<String>) -> Result<()> {
        for p in module_paths {
            // If the path looks like a file path, try to canonicalize; otherwise store as-is.
            let key = p.clone();
            let candidate = Path::new(&p);
            if candidate.exists() {
                if let Some(canon) = self.canonicalize_candidate(candidate) {
                    self.register_canonical(key, canon);
                    continue;
                }
            }
            // store as requested key with best-effort path (non-canonical)
            let entry = ModuleEntry {
                requested_key: key.clone(),
                path: PathBuf::from(&p),
                loaded: false,
                module: None,
                registered_at: SystemTime::now(),
            };
            self.registry.insert(key, entry);
        }

        // If config.load_type == Normal, eagerly load registered modules.
        if let LoadType::Normal = self.config.load_type {
            // Eagerly ensure each registered module is loaded.
            let keys: Vec<String> = self.registry.keys().cloned().collect();
            for k in keys {
                // ignore errors here; caller can inspect list_registered or handle errors later
                let _ = self.ensure_loaded(&k);
            }
        }

        Ok(())
    }

    fn ensure_loaded(&mut self, module_key: &str) -> Result<ApiModule> {
        // 1) If registered and loaded, return cached module.
        if let Some(entry) = self.registry.get(module_key) {
            if entry.loaded {
                if let Some(m) = &entry.module {
                    return Ok(m.clone());
                }
            }
        }

        // 2) If registered but not loaded, attempt to load from entry.path
        if let Some(entry) = self.registry.get_mut(module_key) {
            // Security checks: enforce allowed paths in Strict mode.
            if let Mode::Strict = self.config.mode {
                let allowed = self.config.allowed_paths.iter().any(|ap| {
                    // simple prefix check; canonicalization recommended in full impl
                    entry.path.starts_with(ap)
                });
                if !allowed {
                    return Err(anyhow!("module path '{}' is not allowed by ALLOWED_PATHS", entry.path.display()));
                }
            }

            // Check file size limit if file exists
            if entry.path.exists() {
                if let Ok(meta) = fs::metadata(&entry.path) {
                    let size_kb = meta.len() / 1024;
                    if self.config.max_module_size_kb > 0 && size_kb as usize > self.config.max_module_size_kb {
                        return Err(anyhow!("module '{}' exceeds MAX_MODULE_SIZE_KB", entry.path.display()));
                    }
                }
            }

            // Placeholder: in a full implementation we would parse and execute the module here.
            // For now create an empty module and mark loaded.
            let canonical = if entry.path.exists() {
                self.canonicalize_candidate(&entry.path).unwrap_or(entry.path.clone())
            } else {
                entry.path.clone()
            };
            let module = self.make_empty_module(canonical.clone(), MatchKind::Exact);
            entry.module = Some(module.clone());
            entry.loaded = true;
            return Ok(module);
        }

        // 3) Not registered: attempt fuzzy find if allowed by config.
        if let Mode::Strict = self.config.mode {
            return Err(anyhow!("module '{}' not registered and strict mode forbids fuzzy lookup", module_key));
        }

        if let Some((path, match_kind)) = self.fuzzy_find(module_key) {
            // register and load
            let requested = module_key.to_string();
            self.register_canonical(requested.clone(), path.clone());
            if let Some(entry) = self.registry.get_mut(&requested) {
                let module = self.make_empty_module(path.clone(), match_kind.clone());
                entry.module = Some(module.clone());
                entry.loaded = true;
                return Ok(module);
            }
        }

        Err(anyhow!("module '{}' not found", module_key))
    }

    fn resolve_symbol(&mut self, module_key: &str, symbol: &str) -> Result<ResolvedSymbol> {
        let module = self.ensure_loaded(module_key)?;
        match module.exports.get(symbol) {
            Some(v) => {
                let meta = module.meta.clone();
                Ok(ResolvedSymbol { value: v.clone(), module_meta: meta })
            }
            None => {
                // If EXPORT_POLICY == All, we might still allow access (placeholder).
                match self.config.export_policy {
                    ExportPolicy::All => {
                        // In a full implementation, exports would include all top-level defs.
                        Err(anyhow!("symbol '{}' not found in module '{}' (exports policy=All but symbol missing)", symbol, module_key))
                    }
                    _ => Err(anyhow!("symbol '{}' not found in module '{}'", symbol, module_key)),
                }
            }
        }
    }

    fn fuzzy_find(&self, name: &str) -> Option<(PathBuf, MatchKind)> {
        // Minimal fuzzy finder:
        // - Try exact filename in search_paths
        // - Try filename with common extensions (.ps, .pa, .ph)
        // - Return first match as Exact or Canonicalized
        let candidates = vec![
            name.to_string(),
            format!("{}.ps", name),
            format!("{}.pa", name),
            format!("{}.ph", name),
        ];

        for sp in &self.config.search_paths {
            for cand in &candidates {
                let p = sp.join(cand);
                if p.exists() {
                    if let Ok(canon) = fs::canonicalize(&p) {
                        return Some((canon, MatchKind::Exact));
                    } else {
                        return Some((p, MatchKind::Exact));
                    }
                }
            }
        }

        // No match found; in a full implementation compute fuzzy scores and return best candidate.
        None
    }

    fn clear_cache(&mut self) {
        for (_k, e) in self.registry.iter_mut() {
            e.loaded = false;
            e.module = None;
        }
    }

    fn list_registered(&self) -> Vec<(String, bool)> {
        self.registry.iter().map(|(k, v)| (k.clone(), v.loaded)).collect()
    }
}

/// Factory function exposed by the public API.
/// The mod_api::default_loader_with_config signature expects a boxed trait object.
/// This function reads an optional config path and returns a boxed ModuleLoaderApi.
pub fn default_loader_with_config(config_path: Option<&Path>) -> Box<dyn ModuleLoaderApi> {
    let cfg = if let Some(p) = config_path {
        parse_config_from_file(p)
    } else {
        ModLoaderConfig::default()
    };
    Box::new(ModuleLoader::new(cfg))
}

/// Parse a textual config file into ModLoaderConfig.
/// This is a minimal parser that tolerates comments and missing keys.
pub fn parse_config_from_file(path: &Path) -> ModLoaderConfig {
    use std::io::Read;
    let mut cfg = ModLoaderConfig::default();
    if let Ok(mut f) = fs::File::open(path) {
        let mut s = String::new();
        if f.read_to_string(&mut s).is_ok() {
            for raw in s.lines() {
                let line = raw.trim();
                if line.is_empty() || line.starts_with('#') { continue; }
                if let Some(idx) = line.find(':') {
                    let key = line[..idx].trim().to_lowercase();
                    let val = line[idx+1..].trim();
                    match key.as_str() {
                        "version" => cfg.version = val.to_string(),
                        "mode" => {
                            cfg.mode = match val.to_lowercase().as_str() {
                                "strict" => Mode::Strict,
                                "fuzzy" => Mode::Fuzzy,
                                _ => Mode::Normal,
                            }
                        }
                        "tree_depth" => {
                            if let Ok(n) = val.parse::<i32>() { cfg.tree_depth = n; }
                        }
                        "load_type" => {
                            cfg.load_type = if val.to_lowercase().starts_with('n') { LoadType::Normal } else { LoadType::Lazy };
                        }
                        "caching" => {
                            cfg.caching = matches!(val.to_lowercase().as_str(), "on" | "true" | "yes");
                        }
                        "fuzz_match" => {
                            let v = val.trim_end_matches('%');
                            if let Ok(n) = v.parse::<u8>() { cfg.fuzz_match = n.clamp(50,100); }
                        }
                        "search_paths" => {
                            cfg.search_paths = val.split(';').map(|p| PathBuf::from(p.trim())).collect();
                        }
                        "allowed_paths" => {
                            cfg.allowed_paths = val.split(';').map(|p| PathBuf::from(p.trim())).collect();
                        }
                        "verbose" => {
                            cfg.verbose = matches!(val.to_lowercase().as_str(), "true" | "on" | "yes");
                        }
                        "sandbox_io" => {
                            cfg.sandbox_io = matches!(val.to_lowercase().as_str(), "true" | "on" | "yes");
                        }
                        "max_module_size_kb" => {
                            if let Ok(n) = val.parse::<usize>() { cfg.max_module_size_kb = n; }
                        }
                        "reload_on_change" => {
                            cfg.reload_on_change = matches!(val.to_lowercase().as_str(), "true" | "on" | "yes");
                        }
                        "export_policy" => {
                            cfg.export_policy = match val.to_lowercase().as_str() {
                                "all" => ExportPolicy::All,
                                "none" => ExportPolicy::None,
                                _ => ExportPolicy::Explicit,
                            }
                        }
                        "symbol_aliasing" => {
                            cfg.symbol_aliasing = matches!(val.to_lowercase().as_str(), "true" | "on" | "yes");
                        }
                        "auto_use_on_from" => {
                            cfg.auto_use_on_from = matches!(val.to_lowercase().as_str(), "true" | "on" | "yes");
                        }
                        "strict_name_normalization" => {
                            cfg.strict_name_normalization = matches!(val.to_lowercase().as_str(), "true" | "on" | "yes");
                        }
                        "log_level" => {
                            cfg.log_level = match val.to_lowercase().as_str() {
                                "debug" => LogLevel::Debug,
                                "warn" => LogLevel::Warn,
                                "error" => LogLevel::Error,
                                "none" => LogLevel::None,
                                _ => LogLevel::Info,
                            }
                        }
                        _ => { /* ignore unknown keys for forward compatibility */ }
                    }
                }
            }
        }
    }
    cfg
}
