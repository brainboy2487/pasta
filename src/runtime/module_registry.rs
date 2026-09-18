use std::collections::HashMap;
use std::path::Path;
use anyhow::{anyhow, Result};
use crate::mod_loader::mod_api::{ModuleLoaderApi, default_loader_with_config};

/// Load state for a module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModuleState {
    /// Module has not been loaded yet.
    Unloaded,
    /// Module loading is currently in progress.
    Loading,
    /// Module loaded successfully.
    Loaded,
    /// Module load failed.
    Failed,
}

/// Minimal module registry for Pasta modules.
/// Records known module names and metadata (path, export list, loaded flag).
#[derive(Debug, Clone)]
pub struct ModuleMeta {
    /// Filesystem path for the module.
    pub path: String,
    /// Exported symbol names known for the module.
    pub exports: Vec<String>,
    /// Current load state for the module.
    pub state: ModuleState,
    /// Last load error, if the module failed to load.
    pub error: Option<String>,
}

/// Registry of known modules plus an optional lazy loader adapter.
pub struct ModuleRegistry {
    modules: HashMap<String, ModuleMeta>,
    // Optional adapter to the mod_loader subsystem for lazy loading and resolution.
    loader: Option<Box<dyn ModuleLoaderApi>>,
}

impl Default for ModuleRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for ModuleRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ModuleRegistry")
            .field("modules", &self.modules)
            .field("loader", &"ModuleLoaderApi")
            .finish()
    }
}

impl Clone for ModuleRegistry {
    fn clone(&self) -> Self {
        Self {
            modules: self.modules.clone(),
            // Do not attempt to clone the loader trait object; drop it when cloning.
            loader: None,
        }
    }
}

impl ModuleRegistry {
    /// Create an empty module registry with no attached loader.
    pub fn new() -> Self {
        Self { modules: HashMap::new(), loader: None }
    }

    /// Create a ModuleRegistry and attach the default ModuleLoader configured
    /// from an optional config path. This wires the `mod_loader` subsystem into
    /// the runtime registry so callers can lazily load modules on first access.
    pub fn new_with_loader(config_path: Option<&Path>) -> Self {
        let loader = default_loader_with_config(config_path);
        Self { modules: HashMap::new(), loader: Some(loader) }
    }

    /// Resolve a module name to a filesystem path using the search order:
    /// 1) ./<name>.pm
    /// 2) ./modules/<name>.pm
    /// 3) ./stdlib/<name>.pm
    pub fn resolve_and_register(&mut self, name: &str) -> Result<String> {
        use std::path::PathBuf;
        let cwd = std::env::current_dir()?;
        let candidates = vec![
            cwd.join(format!("{}.pm", name)),
            cwd.join("modules").join(format!("{}.pm", name)),
            PathBuf::from("stdlib").join(format!("{}.pm", name)),
        ];
        for p in candidates.iter() {
            if p.exists() {
                let s = p.to_string_lossy().to_string();
                self.register(name.to_string(), s.clone());
                return Ok(s);
            }
        }
        Err(anyhow!("ModuleNotFound: '{}' (tried standard locations)", name))
    }

    /// Register a module path if the module is not already known.
    pub fn register(&mut self, name: impl Into<String>, path: impl Into<String>) {
        let n = name.into();
        let p = path.into();
        self.modules.entry(n.clone()).or_insert(ModuleMeta { path: p, exports: Vec::new(), state: ModuleState::Unloaded, error: None });
    }

    /// Register module names with the underlying loader if present.
    pub fn register_with_loader(&mut self, module_names: Vec<String>) {
        if let Some(loader) = &mut self.loader {
            let _ = loader.register_use_block(module_names);
        }
    }

    /// Return immutable metadata for a registered module.
    pub fn get(&self, name: &str) -> Option<&ModuleMeta> {
        self.modules.get(name)
    }

    /// Return mutable metadata for a registered module.
    pub fn get_mut(&mut self, name: &str) -> Option<&mut ModuleMeta> {
        self.modules.get_mut(name)
    }

    /// Replace the exported symbol list for a module.
    pub fn set_exports(&mut self, name: &str, exports: Vec<String>) {
        if let Some(m) = self.modules.get_mut(name) {
            m.exports = exports;
        }
    }

    /// Mark a module as loaded and clear any prior error.
    pub fn mark_loaded(&mut self, name: &str) {
        if let Some(m) = self.modules.get_mut(name) {
            m.state = ModuleState::Loaded;
            m.error = None;
        }
    }

    /// Mark a module as currently loading, creating a placeholder entry if needed.
    pub fn mark_loading(&mut self, name: &str) {
        if let Some(m) = self.modules.get_mut(name) {
            m.state = ModuleState::Loading;
            m.error = None;
        } else {
            self.modules.insert(name.into(), ModuleMeta { path: String::new(), exports: Vec::new(), state: ModuleState::Loading, error: None });
        }
    }

    /// Mark a module load as failed and store the failure message.
    pub fn mark_failed(&mut self, name: &str, err: impl Into<String>) {
        if let Some(m) = self.modules.get_mut(name) {
            m.state = ModuleState::Failed;
            m.error = Some(err.into());
        }
    }

    /// Attempt to ensure the named module is loaded via the underlying loader
    /// and update the registry state accordingly. Returns true on success.
    pub fn ensure_loaded_via_loader(&mut self, name: &str) -> bool {
        if let Some(loader) = &mut self.loader {
            match loader.ensure_loaded(name) {
                Ok(m) => {
                    // update registry entry with path and exported symbols
                    let path = m.meta.canonical_path.to_string_lossy().to_string();
                    let exports = m.exports.keys().cloned().collect();
                    self.register(name.to_string(), path);
                    self.set_exports(name, exports);
                    self.mark_loaded(name);
                    return true;
                }
                Err(e) => {
                    self.mark_failed(name, e.to_string());
                    return false;
                }
            }
        }
        false
    }
}
