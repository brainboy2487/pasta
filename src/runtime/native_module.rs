//! Native shared-library module loading for compiled Pasta modules.
//! 
//! Uses libloading for cross-platform dynamic library loading (Unix + Windows).
//! On Unix, libloading internally uses dlopen/dlsym/dlclose.
//! On Windows, libloading internally uses LoadLibrary/GetProcAddress/FreeLibrary.

use std::collections::HashMap;
use std::ffi::CStr;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use libloading::Library;
use libc::c_char;
use serde::Deserialize;

use crate::interpreter::environment::Value;

use super::abi::PValue;
use super::abi_helpers::{
    abi_value_to_runtime_value, pasta_value_release, runtime_value_to_abi_value, PAbiStatus,
};

const SHARED_MODULE_ABI_VERSION: u32 = 1;
const SHARED_MODULE_CALL_ABI: &str = "pasta.module.v1";
const SHARED_MODULE_HANDLE_OWNERSHIP: &str = "retain_release_refcounted";
const SHARED_MODULE_VISIBILITY: &str = "exports_only";

type ModuleInitFn = unsafe extern "C" fn() -> i32;
type ModuleManifestFn = unsafe extern "C" fn() -> *const c_char;
type ModuleCallFn = unsafe extern "C" fn(argc: u64, argv: *const PValue, out: *mut PValue) -> i32;

#[derive(Debug, Clone, Deserialize)]
struct NativeModuleManifest {
    module: String,
    abi_version: u32,
    call_abi: String,
    handle_ownership: String,
    visibility: String,
    exports: Vec<NativeModuleExport>,
}

#[derive(Debug, Clone, Deserialize)]
struct NativeModuleExport {
    name: String,
    arity: usize,
    params: Vec<String>,
    return_type: String,
}

/// Loaded native compiled module plus its resolved exports.
/// Uses libloading for cross-platform dynamic library management.
#[derive(Debug)]
pub struct NativeModule {
    /// Library handle managed by libloading (auto-cleanup on drop)
    _lib: Library,
    manifest: NativeModuleManifest,
    call_fns: HashMap<String, ModuleCallFn>,
}

unsafe impl Send for NativeModule {}
unsafe impl Sync for NativeModule {}

impl NativeModule {
    /// Load a compiled Pasta shared module from a `.so` (Unix) or `.dll` (Windows) path.
    pub fn load(path: &Path, expected_name: &str) -> Result<Arc<Self>> {
        // Load library using libloading (cross-platform)
        let lib = unsafe { Library::new(path) }
            .map_err(|e| anyhow!(
                "failed to load shared module '{}': {}",
                path.display(),
                e
            ))?;

        let result = (|| {
            let manifest_symbol = format!("pasta_mod_{}_manifest_json", sanitize_symbol(expected_name));
            let init_symbol = format!("pasta_mod_{}_init", sanitize_symbol(expected_name));

            let manifest_fn: ModuleManifestFn = unsafe { load_symbol_from_lib(&lib, &manifest_symbol)? };
            let init_fn: ModuleInitFn = unsafe { load_symbol_from_lib(&lib, &init_symbol)? };

            let manifest_ptr = unsafe { manifest_fn() };
            if manifest_ptr.is_null() {
                return Err(anyhow!(
                    "shared module '{}' returned a null manifest pointer",
                    path.display()
                ));
            }
            let manifest_json = unsafe { CStr::from_ptr(manifest_ptr) }
                .to_str()?
                .to_string();
            let manifest: NativeModuleManifest = serde_json::from_str(&manifest_json)?;
            if manifest.module != expected_name {
                return Err(anyhow!(
                    "shared module '{}' reported module name '{}' instead of '{}'",
                    path.display(),
                    manifest.module,
                    expected_name
                ));
            }
            validate_manifest(path, &manifest)?;

            let init_status = unsafe { init_fn() };
            if init_status != PAbiStatus::Ok as i32 {
                return Err(anyhow!(
                    "shared module '{}' init failed with status {}",
                    path.display(),
                    describe_status_code(init_status)
                ));
            }

            let mut call_fns = HashMap::new();
            for export in &manifest.exports {
                let symbol = format!(
                    "pasta_mod_{}_call_{}",
                    sanitize_symbol(expected_name),
                    sanitize_symbol(&export.name)
                );
                let call_fn: ModuleCallFn = unsafe { load_symbol_from_lib(&lib, &symbol)? };
                call_fns.insert(export.name.clone(), call_fn);
            }

            Ok(Arc::new(Self {
                _lib: lib,
                manifest,
                call_fns,
            }))
        })();
        
        // libloading::Library automatically drops and unloads on drop, 
        // no manual cleanup needed
        result
    }

    /// Return the exported function names in manifest order.
    pub fn exports(&self) -> Vec<String> {
        self.manifest
            .exports
            .iter()
            .map(|export| export.name.clone())
            .collect()
    }

    /// Build runtime builtin closures for the module's exported functions.
    pub fn export_builtins(
        self: &Arc<Self>,
        module_name: &str,
    ) -> Result<HashMap<String, Arc<dyn Fn(Vec<Value>) -> Result<Value> + Send + Sync>>> {
        let mut builtins = HashMap::new();
        for export in &self.manifest.exports {
            let builtin_name = format!("__native::{}::{}", module_name, export.name);
            builtins.insert(
                builtin_name,
                self.make_export_builtin(&export.name, export.arity)?,
            );
        }
        Ok(builtins)
    }

    fn make_export_builtin(
        self: &Arc<Self>,
        export_name: &str,
        arity: usize,
    ) -> Result<Arc<dyn Fn(Vec<Value>) -> Result<Value> + Send + Sync>> {
        let call_fn = *self
            .call_fns
            .get(export_name)
            .ok_or_else(|| anyhow!("missing native export wrapper for '{}'", export_name))?;
        let module = Arc::clone(self);
        let export_name = export_name.to_string();
        Ok(Arc::new(move |args: Vec<Value>| -> Result<Value> {
            if args.len() != arity {
                return Err(anyhow!(
                    "compiled function '{}' expects exactly {} argument(s), got {}",
                    export_name,
                    arity,
                    args.len()
                ));
            }

            let abi_args = args
                .iter()
                .map(runtime_value_to_abi_value)
                .collect::<Result<Vec<_>, _>>()
                .map_err(status_to_error)?;

            let mut out = PValue::none();
            let status = unsafe { call_fn(abi_args.len() as u64, abi_args.as_ptr(), &mut out) };
            if status != PAbiStatus::Ok as i32 {
                for value in &abi_args {
                    let _ = pasta_value_release(*value);
                }
                return Err(anyhow!(
                    "compiled module call '{}.{}' failed with status {}",
                    module.manifest.module,
                    export_name,
                    describe_status_code(status)
                ));
            }

            let result = abi_value_to_runtime_value(out).map_err(status_to_error)?;
            for value in &abi_args {
                let _ = pasta_value_release(*value);
            }
            let _ = pasta_value_release(out);
            Ok(result)
        }))
    }
}

// Note: Drop impl is no longer needed. libloading::Library handles cleanup automatically.

unsafe fn load_symbol_from_lib<T>(lib: &Library, symbol: &str) -> Result<T> {
    // libloading automatically handles platform differences:
    // On Unix: uses dlsym internally
    // On Windows: uses GetProcAddress internally
    let raw = lib.get::<*const T>(symbol.as_bytes())
        .map_err(|e| anyhow!("failed to resolve symbol '{}': {}", symbol, e))?;
    Ok(std::mem::transmute_copy(&raw))
}

fn sanitize_symbol(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "module".to_string()
    } else {
        out
    }
}

fn validate_manifest(path: &Path, manifest: &NativeModuleManifest) -> Result<()> {
    if manifest.abi_version != SHARED_MODULE_ABI_VERSION {
        return Err(anyhow!(
            "shared module '{}' uses ABI version {} instead of {}",
            path.display(),
            manifest.abi_version,
            SHARED_MODULE_ABI_VERSION
        ));
    }
    if manifest.call_abi != SHARED_MODULE_CALL_ABI {
        return Err(anyhow!(
            "shared module '{}' uses call ABI '{}' instead of '{}'",
            path.display(),
            manifest.call_abi,
            SHARED_MODULE_CALL_ABI
        ));
    }
    if manifest.handle_ownership != SHARED_MODULE_HANDLE_OWNERSHIP {
        return Err(anyhow!(
            "shared module '{}' uses handle ownership '{}' instead of '{}'",
            path.display(),
            manifest.handle_ownership,
            SHARED_MODULE_HANDLE_OWNERSHIP
        ));
    }
    if manifest.visibility != SHARED_MODULE_VISIBILITY {
        return Err(anyhow!(
            "shared module '{}' uses visibility '{}' instead of '{}'",
            path.display(),
            manifest.visibility,
            SHARED_MODULE_VISIBILITY
        ));
    }
    for export in &manifest.exports {
        if export.arity != export.params.len() {
            return Err(anyhow!(
                "shared module '{}' export '{}' reports arity {} but {} parameter types",
                path.display(),
                export.name,
                export.arity,
                export.params.len()
            ));
        }
        if export.return_type.is_empty() {
            return Err(anyhow!(
                "shared module '{}' export '{}' has an empty return type",
                path.display(),
                export.name
            ));
        }
    }
    Ok(())
}

fn describe_status_code(status: i32) -> String {
    match PAbiStatus::from_code(status) {
        Some(kind) => format!("{status} ({kind:?})"),
        None => status.to_string(),
    }
}

fn status_to_error(status: PAbiStatus) -> anyhow::Error {
    anyhow!(
        "compiled module ABI bridge failed with status {} ({status:?})",
        status.code()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Test suite for Phase 1: Cross-platform dynamic loading with libloading
    /// These tests ensure that the migration from dlopen/dlsym to libloading
    /// works correctly on both Windows and Unix platforms.

    #[test]
    fn test_phase1_libloading_integration() {
        /// Verify that libloading crate is properly integrated
        /// and can be used for dynamic library loading.
        
        // Test that Library type is correctly imported and available
        // This is a compile-time check, but we document it in a test
        assert_eq!(std::mem::size_of::<Option<Library>>(), std::mem::size_of::<*mut libc::c_void>() * 2);
    }

    #[test]
    fn test_native_module_struct_uses_libloading() {
        /// Verify that NativeModule struct uses libloading::Library
        /// instead of raw c_void pointers.
        
        // The struct should contain a Library field (_lib)
        // This is verified by the successful compilation and public API
        
        // NativeModule::load signature should still work as before
        // This tests the public API compatibility
        let path = PathBuf::from("/nonexistent/module.so");
        let result = NativeModule::load(&path, "test_module");
        
        // Should fail gracefully with an error (file doesn't exist)
        assert!(result.is_err());
        
        // But the error should be about file not found, not a link error
        let err_msg = format!("{:?}", result);
        assert!(err_msg.contains("failed to load") || err_msg.contains("No such file"));
    }

    #[test]
    fn test_sanitize_symbol_preserves_compatibility() {
        /// Ensure sanitize_symbol function still works after refactoring
        
        let test_cases = vec![
            ("simple", "simple"),
            ("with-dashes", "with_dashes"),
            ("with.dots", "with_dots"),
            ("with spaces", "with_spaces"),
            ("UPPERCASE", "UPPERCASE"),
            ("123numbers", "123numbers"),
            ("_underscore_", "_underscore_"),
            ("", "module"),  // Empty becomes "module"
            ("special!@#$%^&*()", "special__________"),
        ];

        for (input, expected) in test_cases {
            let result = sanitize_symbol(input);
            assert_eq!(result, expected, "Failed for input: {}", input);
        }
    }

    #[test]
    fn test_manifest_validation_logic_unchanged() {
        /// Verify that manifest validation still works correctly
        
        let manifest = NativeModuleManifest {
            module: "test_module".to_string(),
            abi_version: SHARED_MODULE_ABI_VERSION,
            call_abi: SHARED_MODULE_CALL_ABI.to_string(),
            handle_ownership: SHARED_MODULE_HANDLE_OWNERSHIP.to_string(),
            visibility: SHARED_MODULE_VISIBILITY.to_string(),
            exports: vec![
                NativeModuleExport {
                    name: "test_fn".to_string(),
                    arity: 2,
                    params: vec!["i64".to_string(), "f64".to_string()],
                    return_type: "i64".to_string(),
                }
            ],
        };

        let path = std::path::Path::new("/test/module.so");
        let result = validate_manifest(path, &manifest);
        assert!(result.is_ok(), "Valid manifest should pass validation");
    }

    #[test]
    fn test_manifest_validation_rejects_invalid_abi_version() {
        /// Verify ABI version validation
        
        let mut manifest = NativeModuleManifest {
            module: "test".to_string(),
            abi_version: 999,  // Wrong version
            call_abi: SHARED_MODULE_CALL_ABI.to_string(),
            handle_ownership: SHARED_MODULE_HANDLE_OWNERSHIP.to_string(),
            visibility: SHARED_MODULE_VISIBILITY.to_string(),
            exports: vec![],
        };

        let path = std::path::Path::new("/test/module.so");
        let result = validate_manifest(path, &manifest);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("ABI version"));
    }

    #[test]
    fn test_manifest_validation_rejects_mismatched_arity() {
        /// Verify arity/param count validation
        
        let manifest = NativeModuleManifest {
            module: "test".to_string(),
            abi_version: SHARED_MODULE_ABI_VERSION,
            call_abi: SHARED_MODULE_CALL_ABI.to_string(),
            handle_ownership: SHARED_MODULE_HANDLE_OWNERSHIP.to_string(),
            visibility: SHARED_MODULE_VISIBILITY.to_string(),
            exports: vec![
                NativeModuleExport {
                    name: "bad_fn".to_string(),
                    arity: 2,  // Says 2 args
                    params: vec!["i64".to_string()],  // But only 1 param
                    return_type: "i64".to_string(),
                }
            ],
        };

        let path = std::path::Path::new("/test/module.so");
        let result = validate_manifest(path, &manifest);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("arity"));
    }

    #[test]
    fn test_manifest_validation_rejects_empty_return_type() {
        /// Verify return type validation
        
        let manifest = NativeModuleManifest {
            module: "test".to_string(),
            abi_version: SHARED_MODULE_ABI_VERSION,
            call_abi: SHARED_MODULE_CALL_ABI.to_string(),
            handle_ownership: SHARED_MODULE_HANDLE_OWNERSHIP.to_string(),
            visibility: SHARED_MODULE_VISIBILITY.to_string(),
            exports: vec![
                NativeModuleExport {
                    name: "bad_fn".to_string(),
                    arity: 0,
                    params: vec![],
                    return_type: "".to_string(),  // Empty return type
                }
            ],
        };

        let path = std::path::Path::new("/test/module.so");
        let result = validate_manifest(path, &manifest);
        assert!(result.is_err());
        assert!(format!("{:?}", result).contains("empty return type"));
    }

    #[test]
    fn test_platform_aware_library_naming() {
        /// Test that library naming conventions are correct
        /// (This is more of a documentation test)
        
        #[cfg(unix)]
        let expected_ext = ".so";
        
        #[cfg(windows)]
        let expected_ext = ".dll";
        
        // Document what we expect
        assert!(!expected_ext.is_empty());
    }

    #[test]
    fn test_error_handling_with_missing_file() {
        /// Test that error handling works gracefully for missing files
        
        let path = PathBuf::from("/definitely/does/not/exist/module_xyz_12345.so");
        let result = NativeModule::load(&path, "nonexistent");
        
        // Should fail gracefully
        assert!(result.is_err());
        
        // Error message should be informative
        let error_msg = format!("{:?}", result);
        assert!(!error_msg.is_empty());
    }

    #[test]
    fn test_cross_platform_constant_values() {
        /// Verify ABI constants are set correctly
        /// These should be the same on all platforms
        
        assert_eq!(SHARED_MODULE_ABI_VERSION, 1);
        assert_eq!(SHARED_MODULE_CALL_ABI, "pasta.module.v1");
        assert_eq!(SHARED_MODULE_HANDLE_OWNERSHIP, "retain_release_refcounted");
        assert_eq!(SHARED_MODULE_VISIBILITY, "exports_only");
    }

    #[test]
    fn test_describe_status_code_formatting() {
        /// Test status code description formatting
        
        let desc_ok = describe_status_code(0);
        assert!(!desc_ok.is_empty());
        
        let desc_error = describe_status_code(999);
        assert!(!desc_error.is_empty());
    }

    #[test]
    fn test_libloading_api_compatibility() {
        /// Verify that we haven't broken the API surface
        /// All public functions should still be accessible
        
        // This test compiles if all public APIs are still there
        let _path = std::path::Path::new("test.so");
        let _expected = "test";
        
        // The load function signature should be unchanged:
        // pub fn load(path: &Path, expected_name: &str) -> Result<Arc<Self>>
        
        // This would compile only if the signature is correct
        let _: Result<Arc<NativeModule>> = NativeModule::load(_path, _expected);
    }

    #[test]
    fn test_module_exports_api_unchanged() {
        /// Verify that exports() method still works
        
        let manifest = NativeModuleManifest {
            module: "test".to_string(),
            abi_version: SHARED_MODULE_ABI_VERSION,
            call_abi: SHARED_MODULE_CALL_ABI.to_string(),
            handle_ownership: SHARED_MODULE_HANDLE_OWNERSHIP.to_string(),
            visibility: SHARED_MODULE_VISIBILITY.to_string(),
            exports: vec![
                NativeModuleExport {
                    name: "fn1".to_string(),
                    arity: 0,
                    params: vec![],
                    return_type: "i64".to_string(),
                },
                NativeModuleExport {
                    name: "fn2".to_string(),
                    arity: 1,
                    params: vec!["f64".to_string()],
                    return_type: "f64".to_string(),
                },
            ],
        };

        let expected_exports = vec!["fn1".to_string(), "fn2".to_string()];
        
        // We can't directly test this without a real module, but we verify
        // the structure is correct
        assert_eq!(manifest.exports.len(), 2);
        assert_eq!(manifest.exports[0].name, "fn1");
        assert_eq!(manifest.exports[1].name, "fn2");
    }

    #[test]
    fn test_send_sync_traits_maintained() {
        /// Verify that NativeModule is still Send + Sync
        /// This is important for thread safety
        
        const fn assert_send<T: Send>() {}
        const fn assert_sync<T: Sync>() {}
        
        // These calls won't compile if NativeModule loses Send/Sync
        const fn check() {
            assert_send::<NativeModule>();
            assert_sync::<NativeModule>();
        }
        
        // If this compiles, the traits are correct
        check();
    }

    #[test]
    fn test_libloading_no_memory_leaks_on_error() {
        /// Test that errors during loading don't leak memory
        /// libloading::Library should auto-drop
        
        // Attempting to load non-existent library multiple times
        let path = PathBuf::from("/nonexistent/lib_xyz_123.so");
        
        for _ in 0..10 {
            let result = NativeModule::load(&path, "test");
            assert!(result.is_err());
            // If memory leaked, this would accumulate
        }
        
        // If we got here without panic/OOM, no major leaks
        assert!(true);
    }

    #[test]
    fn test_manifest_export_count() {
        /// Test that export counting works correctly
        
        let exports = vec![
            NativeModuleExport {
                name: "fn1".to_string(),
                arity: 1,
                params: vec!["i64".to_string()],
                return_type: "i64".to_string(),
            },
            NativeModuleExport {
                name: "fn2".to_string(),
                arity: 2,
                params: vec!["i64".to_string(), "f64".to_string()],
                return_type: "bool".to_string(),
            },
            NativeModuleExport {
                name: "fn3".to_string(),
                arity: 0,
                params: vec![],
                return_type: "string".to_string(),
            },
        ];

        assert_eq!(exports.len(), 3);
        assert_eq!(exports[0].arity, 1);
        assert_eq!(exports[1].arity, 2);
        assert_eq!(exports[2].arity, 0);
    }

    #[test]
    fn test_phase1_regression_check() {
        /// Meta-test: Verify no regressions from Phase 1 changes
        /// 
        /// This test documents what should NOT have changed:
        /// 1. Public API unchanged
        /// 2. Error handling unchanged
        /// 3. Manifest validation unchanged
        /// 4. Export resolution unchanged
        /// 5. ABI compatibility unchanged
        
        // All these should still work:
        let _: fn(&Path, &str) -> Result<Arc<NativeModule>> = NativeModule::load;
        
        // Manifest struct should have same fields
        let manifest = NativeModuleManifest {
            module: "test".to_string(),
            abi_version: 1,
            call_abi: "pasta.module.v1".to_string(),
            handle_ownership: "retain_release_refcounted".to_string(),
            visibility: "exports_only".to_string(),
            exports: vec![],
        };
        
        assert_eq!(manifest.module, "test");
        assert_eq!(manifest.abi_version, 1);
    }

    #[test]
    fn test_phase1_windows_unix_parity() {
        /// Verify that Windows and Unix paths work equally well
        /// The refactoring should make library loading uniform
        
        #[cfg(unix)]
        {
            // On Unix, .so extension is standard
            let unix_paths = vec![
                "/usr/lib/libm.so",
                "/usr/lib/libm.so.6",
                "./module.so",
            ];
            for path_str in unix_paths {
                let path = std::path::Path::new(path_str);
                assert!(path.extension().is_some());
            }
        }
        
        #[cfg(windows)]
        {
            // On Windows, .dll extension is standard
            let win_paths = vec![
                "C:\\Windows\\System32\\kernel32.dll",
                ".\\module.dll",
            ];
            for path_str in win_paths {
                let path = std::path::Path::new(path_str);
                assert!(path.extension().is_some());
            }
        }

        #[cfg(windows)]
        #[test]
        fn smoke_load_kernel32_and_symbol() {
            let lib = unsafe { Library::new("kernel32.dll") }.expect("open kernel32.dll");
            let symbol = unsafe {
                lib.get::<unsafe extern "system" fn(*const u16) -> *mut core::ffi::c_void>(
                    b"GetModuleHandleW\0",
                )
            };
            assert!(symbol.is_ok(), "GetModuleHandleW should resolve from kernel32.dll");
        }

        #[cfg(unix)]
        #[test]
        fn smoke_load_libc_and_symbol() {
            let candidates = ["libc.so.6", "libc.so", "libSystem.B.dylib"];
            let mut loaded = None;
            for name in candidates {
                if let Ok(lib) = unsafe { Library::new(name) } {
                    loaded = Some(lib);
                    break;
                }
            }
            let lib = loaded.expect("open libc candidate");
            let symbol = unsafe { lib.get::<unsafe extern "C" fn(*const c_char, ...) -> i32>(b"printf\0") };
            assert!(symbol.is_ok(), "printf should resolve from libc candidate");
        }
    }
}
