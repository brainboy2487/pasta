// src/mod_loader/mod.rs
//! Public re-exports for the module loader subsystem.
//! - Keep mod_api.rs as the stable public surface.
//! - mod_load.rs contains the concrete implementation.
//! - This file re-exports the API and the default factory for easy use.

pub mod mod_api;
mod mod_load;
pub mod binary;

pub use mod_api::{
    MatchKind,
    ModuleMeta,
    Module,
    ResolvedSymbol,
    LoaderConfig,
    ModuleLoaderApi,
    default_loader_with_config as default_loader_factory,
};

pub use mod_load::{
    ModuleLoader,           // concrete implementation (if callers need it)
    default_loader_with_config, // concrete factory (same name as API factory)
    parse_config_from_file, // helper to read module_loader.conf into internal config
};

pub use binary::{
    compile_to_binary,
    load_from_binary,
    compile_file,
    load_file as load_binary_file,
    load_module,
    is_binary_current,
    BinaryError,
    BinaryResult,
};

