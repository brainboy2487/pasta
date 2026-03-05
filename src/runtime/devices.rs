// src/runtime/devices.rs
//! Device architecture registry and auto-configuration utilities.
//!
//! This module provides:
//! - An embedded device architecture list (loaded from `device_ls.json`).
//! - Lookup helpers to find a device profile for the current host architecture.
//! - A small registry of device-specific configuration handlers keyed by device id.
//! - A convenience `auto_configure` function that attempts to pick the best device
//!   profile and apply its default configuration to the provided runtime environment.
//!
//! Design notes:
//! - The JSON is embedded at compile time via `include_str!` so the binary is
//!   self-contained. You can replace this with a runtime file read if you prefer
//!   dynamic updates.
//! - Handlers are pluggable: add a new entry to `built_in_handlers()` to customize how a
//!   device id maps to concrete runtime settings (e.g., enabling asm, tuning threads).
//! - Default behavior: apply `default_config` keys as environment variables (via
//!   `Environment::set_local`) so the rest of the runtime can read them.

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value as JsonValue;
use std::collections::HashMap;

use crate::interpreter::environment::{Environment, Value as EnvValue};
pub use crate::interpreter::environment::Value;

const DEVICE_JSON: &str = include_str!("device_ls.json");

#[derive(Debug, Clone, Deserialize)]
pub struct DeviceEntry {
    pub id: String,
    pub name: String,
    pub arch: String,
    pub features: Vec<String>,
    /// Note: default_config stores JSON values (serde_json::Value)
    pub default_config: HashMap<String, JsonValue>,
    pub notes: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct DeviceList {
    devices: Vec<DeviceEntry>,
}

impl DeviceList {
    pub fn load() -> Result<Self> {
        let dl: DeviceList = serde_json::from_str(DEVICE_JSON)?;
        Ok(dl)
    }

    pub fn find_by_arch(&self, arch: &str) -> Option<DeviceEntry> {
        // Prefer exact arch match; if multiple, pick the first.
        self.devices
            .iter()
            .find(|d| d.arch.eq_ignore_ascii_case(arch))
            .cloned()
    }

    pub fn find_by_id(&self, id: &str) -> Option<DeviceEntry> {
        self.devices.iter().find(|d| d.id == id).cloned()
    }
}

/// Type for device-specific handler functions.
///
/// Handlers receive the `DeviceEntry` and a mutable reference to the `Environment`.
/// They should apply any device-specific configuration and return Ok(()) or an error.
pub type DeviceHandler = fn(&DeviceEntry, &mut Environment) -> Result<()>;

/// Convert a serde_json::Value into the interpreter's EnvValue.
///
/// This mapping is intentionally conservative:
/// - Null -> EnvValue::String("null") if no Null variant exists, otherwise EnvValue::Null
/// - Bool -> EnvValue::Bool
/// - Number -> EnvValue::Number (f64)
/// - String -> EnvValue::String
/// - Array/Object -> serialized JSON string if the environment has no structured variant,
///   otherwise converted recursively into List/Map variants.
fn json_to_env_value(j: &JsonValue) -> EnvValue {
    match j {
        JsonValue::Null => {
            // If your EnvValue has a Null variant, prefer that. Otherwise use String("null").
            // Here we try to use EnvValue::String("null") as a safe fallback.
            // Adjust if your EnvValue enum defines a Null or None variant.
            EnvValue::String("null".into())
        }
        JsonValue::Bool(b) => EnvValue::Bool(*b),
        JsonValue::Number(n) => {
            // Convert to f64 when possible
            if let Some(f) = n.as_f64() {
                EnvValue::Number(f)
            } else if let Some(i) = n.as_i64() {
                EnvValue::Number(i as f64)
            } else if let Some(u) = n.as_u64() {
                EnvValue::Number(u as f64)
            } else {
                EnvValue::Number(0.0)
            }
        }
        JsonValue::String(s) => EnvValue::String(s.clone()),
        JsonValue::Array(arr) => {
            // Try to convert to a list of EnvValue if your EnvValue supports it.
            // Many environment implementations don't have a List variant; in that case
            // serialize to a compact JSON string to preserve structure.
            // We'll attempt to create a list-like string representation.
            let mut elems: Vec<EnvValue> = Vec::with_capacity(arr.len());
            for e in arr {
                elems.push(json_to_env_value(e));
            }
            // If EnvValue has a List variant, replace the following line with that variant.
            // Fallback: serialize to JSON string.
            match serde_json::to_string(arr) {
                Ok(s) => EnvValue::String(s),
                Err(_) => EnvValue::String("[]".into()),
            }
        }
        JsonValue::Object(map) => {
            // Similar to arrays: try to convert to a map if supported; otherwise serialize.
            match serde_json::to_string(map) {
                Ok(s) => EnvValue::String(s),
                Err(_) => EnvValue::String("{}".into()),
            }
        }
    }
}

/// Apply the `default_config` map from a `DeviceEntry` into the `Environment`.
///
/// The default behavior maps JSON values to `Environment::set_local` with simple conversions:
/// - JSON number -> EnvValue::Number
/// - JSON string -> EnvValue::String
/// - JSON bool -> EnvValue::Bool
/// - JSON array/object -> EnvValue::String (serialized) for now
fn apply_default_config(dev: &DeviceEntry, env: &mut Environment) -> Result<()> {
    for (k, v) in &dev.default_config {
        let val = json_to_env_value(v);
        env.set_local(k.clone(), val);
    }
    Ok(())
}

/// Registry of built-in handlers for known device IDs.
///
/// Add new handlers here to customize configuration logic for a device id.
fn built_in_handlers() -> HashMap<String, DeviceHandler> {
    let mut m: HashMap<String, DeviceHandler> = HashMap::new();

    // Generic x86_64 handler: enable asm flag and set thread count
    m.insert(
        "intel_x86_64".to_string(),
        |dev: &DeviceEntry, env: &mut Environment| -> Result<()> {
            // Apply default_config keys to environment as local variables
            apply_default_config(dev, env)?;
            // Example: if use_asm is true, set a runtime flag variable
            if let Some(v) = dev.default_config.get("use_asm") {
                if v.as_bool().unwrap_or(false) {
                    env.set_local("runtime_use_asm".to_string(), EnvValue::Bool(true));
                }
            }
            Ok(())
        },
    );

    // Apple ARM handler: prefer compiler intrinsics (no asm)
    m.insert(
        "apple_arm64".to_string(),
        |dev: &DeviceEntry, env: &mut Environment| -> Result<()> {
            apply_default_config(dev, env)?;
            env.set_local("runtime_use_asm".to_string(), EnvValue::Bool(false));
            env.set_local("platform".to_string(), EnvValue::String("apple".into()));
            Ok(())
        },
    );

    // Raspberry Pi handler: conservative thread count
    m.insert(
        "raspi_arm64".to_string(),
        |dev: &DeviceEntry, env: &mut Environment| -> Result<()> {
            apply_default_config(dev, env)?;
            env.set_local("runtime_use_asm".to_string(), EnvValue::Bool(false));
            env.set_local("platform".to_string(), EnvValue::String("raspi".into()));
            Ok(())
        },
    );

    // WASM handler: single-threaded, no asm
    m.insert(
        "wasm32".to_string(),
        |dev: &DeviceEntry, env: &mut Environment| -> Result<()> {
            apply_default_config(dev, env)?;
            env.set_local("runtime_use_asm".to_string(), EnvValue::Bool(false));
            env.set_local("platform".to_string(), EnvValue::String("wasm".into()));
            Ok(())
        },
    );

    m
}

/// Detect the host architecture string (e.g., "x86_64", "aarch64", "wasm32").
///
/// Uses `std::env::consts::ARCH` which is set at compile time for the target.
/// For cross-compiled binaries this reflects the target architecture; for
/// runtime detection on the host, you may prefer to probe `/proc/cpuinfo` or
/// use platform-specific APIs. This function intentionally returns the Rust
/// target arch string for simplicity.
pub fn detect_host_arch() -> String {
    std::env::consts::ARCH.to_string()
}

/// Find the best device entry for the current host architecture.
///
/// Returns `Ok(Some(device))` if a matching device profile was found,
/// `Ok(None)` if none matched, or `Err` on JSON/parse errors.
pub fn find_device_for_current_host() -> Result<Option<DeviceEntry>> {
    let dl = DeviceList::load()?;
    let arch = detect_host_arch();
    Ok(dl.find_by_arch(&arch))
}

/// Find a device by id.
pub fn find_device_by_id(id: &str) -> Result<Option<DeviceEntry>> {
    let dl = DeviceList::load()?;
    Ok(dl.find_by_id(id))
}

/// Apply device configuration automatically to the provided `Environment`.
///
/// This function:
/// 1. Attempts to find a device profile for the current host arch.
/// 2. If found, looks up a built-in handler for the device id and calls it.
/// 3. If no handler exists, falls back to applying `default_config` generically.
/// 4. Returns the `DeviceEntry` that was applied (if any).
pub fn auto_configure(env: &mut Environment) -> Result<Option<DeviceEntry>> {
    let dl = DeviceList::load()?;
    let arch = detect_host_arch();
    if let Some(dev) = dl.find_by_arch(&arch) {
        let handlers = built_in_handlers();
        if let Some(handler) = handlers.get(&dev.id) {
            handler(&dev, env)?;
        } else {
            // Generic fallback
            apply_default_config(&dev, env)?;
        }
        // Also set a canonical variable for the chosen device id
        env.set_local("device_id".to_string(), EnvValue::String(dev.id.clone()));
        env.set_local("device_arch".to_string(), EnvValue::String(dev.arch.clone()));
        Ok(Some(dev))
    } else {
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::environment::Environment;

    #[test]
    fn load_device_list() {
        let dl = DeviceList::load().unwrap();
        assert!(!dl.devices.is_empty());
    }

    #[test]
    fn find_by_arch_known() {
        // We can test by explicitly looking up a known arch from the JSON
        let dl = DeviceList::load().unwrap();
        let maybe = dl.find_by_arch("x86_64");
        assert!(maybe.is_some());
        let dev = maybe.unwrap();
        assert_eq!(dev.arch, "x86_64");
        assert!(dev.id.contains("x86") || dev.id.contains("intel"));
    }

    #[test]
    fn auto_configure_applies_defaults() {
        let mut env = Environment::new();
        // Simulate applying for the arch present in the JSON; use detect_host_arch()
        if let Ok(Some(dev)) = find_device_for_current_host() {
            let res = {
                // call auto_configure which will apply config for current arch
                auto_configure(&mut env)
            };
            // Should succeed (may return Some or None depending on test host)
            assert!(res.is_ok());
            // If a device was applied, device_id should be set
            if env.get("device_id").is_some() {
                assert_eq!(env.get("device_arch").unwrap(), Value::String(dev.arch));
            }
        } else {
            // If no device for current host, ensure auto_configure returns Ok(None)
            let r = auto_configure(&mut env).unwrap();
            assert!(r.is_none());
        }
    }

    #[test]
    fn find_device_by_id_works() {
        let maybe = find_device_by_id("intel_x86_64").unwrap();
        assert!(maybe.is_some());
        let dev = maybe.unwrap();
        assert_eq!(dev.id, "intel_x86_64");
    }
}
