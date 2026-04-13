// src/error_logging/error_messages.rs
//! Typed access to the JSON-backed error message catalog.
//!
//! The runtime already has rich error types in `interpreter::errors`; this
//! module complements them with a catalog of longer explanations and
//! suggestions loaded from `error_messages.json`.

use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Metadata stored at the top level of `error_messages.json`.
#[derive(Debug, Deserialize)]
pub struct ErrorMeta {
    /// Schema/catalog version string.
    pub version: String,
    /// Tool or script that generated the catalog, if any.
    pub generated_by: Option<String>,
    /// Free-form notes about the catalog contents.
    pub notes: Option<String>,
}

/// A single catalog entry keyed by symbolic error name.
#[derive(Debug, Deserialize)]
pub struct ErrorEntry {
    /// Stable external error code shown to users.
    pub code: String,
    /// Severity label such as `error` or `fatal`.
    pub severity: String,
    /// Primary error message with placeholder substitutions.
    pub message: String,
    /// Longer explanation of what went wrong.
    pub explanation: Option<String>,
    /// Suggested next action for the user.
    pub suggestion: Option<String>,
}

/// Full deserialized JSON payload from `error_messages.json`.
#[derive(Debug, Deserialize)]
struct ErrorFile {
    /// Catalog metadata.
    meta: ErrorMeta,
    /// Error entries keyed by symbolic name.
    errors: HashMap<String, ErrorEntry>,
}

/// Loaded error catalog used by the runtime error handler.
pub struct ErrorCatalog {
    meta: ErrorMeta,
    entries: HashMap<String, ErrorEntry>,
}

impl ErrorCatalog {
    /// Load the catalog from a JSON file on disk.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let s = fs::read_to_string(path)?;
        let ef: ErrorFile = serde_json::from_str(&s)?;
        Ok(Self { meta: ef.meta, entries: ef.errors })
    }

    /// Return catalog metadata.
    pub fn meta(&self) -> &ErrorMeta {
        &self.meta
    }

    /// Look up a catalog entry by symbolic key.
    pub fn get(&self, key: &str) -> Option<&ErrorEntry> {
        self.entries.get(key)
    }
}

/// Return the default on-disk catalog path within the repository.
pub fn default_catalog_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src")
        .join("error_logging")
        .join("error_messages.json")
}

/// Load the default catalog bundled with the crate.
pub fn default_catalog() -> anyhow::Result<ErrorCatalog> {
    ErrorCatalog::load_from(&default_catalog_path())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_catalog_loads_known_entry() {
        let catalog = default_catalog().expect("default catalog should load");
        assert_eq!(catalog.meta().version, "0.1");
        let entry = catalog
            .get("STACK_OVERFLOW")
            .expect("STACK_OVERFLOW entry should exist");
        assert_eq!(entry.code, "E001");
        assert_eq!(entry.severity, "fatal");
    }
}
