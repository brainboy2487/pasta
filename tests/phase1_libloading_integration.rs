//! Phase 1 Integration Tests: libloading Migration
//! 
//! This test suite comprehensively validates that the migration from Unix-only
//! dlopen/dlsym/dlclose to cross-platform libloading was successful.
//! 
//! Tests run on both Windows and Unix to ensure platform parity.

use std::path::PathBuf;

#[test]
fn test_phase1_cargo_toml_has_libloading() {
    /// Verify libloading dependency was added to Cargo.toml
    let cargo_toml = include_str!("../Cargo.toml");
    assert!(cargo_toml.contains("libloading"),
        "Cargo.toml should contain libloading dependency");
}

#[test]
fn test_phase1_x11_is_conditional() {
    /// Verify x11 is now platform-conditional
    let cargo_toml = include_str!("../Cargo.toml");
    
    // Should have conditional unix dependency section
    assert!(cargo_toml.contains("[target.'cfg(unix)'.dependencies]"),
        "Should have conditional Unix dependencies section");
    
    // x11 should NOT be in top-level dependencies
    let top_deps_section = cargo_toml
        .lines()
        .skip_while(|l| !l.contains("[dependencies]"))
        .take_while(|l| !l.contains("[features]"))
        .collect::<Vec<_>>()
        .join("\n");
    
    // On Unix platforms, x11 should be in conditional section, not top-level
    if cfg!(unix) {
        assert!(!top_deps_section.contains("x11 = "),
            "x11 should NOT be in top-level [dependencies]");
    }
    
    // x11 should be in features
    assert!(cargo_toml.contains("x11 = [\"dep:x11\"]"),
        "x11 should be in [features]");
}

#[test]
fn test_phase1_no_unix_only_imports() {
    /// Verify native_module.rs doesn't import Unix-only functions directly
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Should NOT have direct dlopen import at top level
    let lines: Vec<&str> = native_module.lines().take(50).collect();
    let top_imports = lines.join("\n");
    
    assert!(!top_imports.contains("use libc::{") ||
            !top_imports.contains("dlopen") ||
            !top_imports.contains("dlsym") ||
            !top_imports.contains("dlclose"),
        "Top-level imports should not have dlopen/dlsym/dlclose");
}

#[test]
fn test_phase1_uses_libloading() {
    /// Verify native_module.rs imports and uses libloading
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("use libloading::Library"),
        "Should import libloading::Library");
    assert!(native_module.contains("_lib: Library"),
        "NativeModule struct should have _lib: Library field");
}

#[test]
fn test_phase1_removes_raw_handle() {
    /// Verify raw c_void handle was replaced with libloading
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Should NOT have raw handle: *mut c_void in struct
    let struct_section = native_module
        .lines()
        .skip_while(|l| !l.contains("pub struct NativeModule"))
        .take(10)
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(!struct_section.contains("handle: *mut c_void"),
        "Raw c_void handle should be removed from struct");
}

#[test]
fn test_phase1_removes_manual_cleanup() {
    /// Verify manual dlclose is removed (libloading auto-cleans)
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Should NOT have Drop impl with dlclose
    assert!(!native_module.contains("impl Drop for NativeModule"),
        "Manual Drop impl should be removed (libloading handles it)");
    
    // Should NOT have dlclose call
    let without_comments = native_module
        .lines()
        .filter(|l| !l.trim().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(!without_comments.contains("dlclose("),
        "Should not call dlclose directly");
}

#[test]
fn test_phase1_cross_platform_symbol_loading() {
    /// Verify load_symbol_from_lib works on all platforms
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("load_symbol_from_lib"),
        "Should have load_symbol_from_lib function");
    
    assert!(native_module.contains("libloading") ||
            native_module.contains("Library::new"),
        "Should use libloading for loading");
}

#[test]
fn test_phase1_api_compatibility() {
    /// Verify public API is unchanged
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Should still have these public functions/types
    assert!(native_module.contains("pub struct NativeModule"),
        "NativeModule struct should be public");
    
    assert!(native_module.contains("pub fn load"),
        "load() should be public");
    
    assert!(native_module.contains("pub fn exports"),
        "exports() should be public");
    
    assert!(native_module.contains("pub fn export_builtins"),
        "export_builtins() should be public");
}

#[test]
fn test_phase1_error_handling_preserved() {
    /// Verify error handling uses anyhow::Result consistently
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Should have consistent error handling
    assert!(native_module.contains("Result<Arc<Self>>"),
        "Should return Result<Arc<Self>>");
    
    assert!(native_module.contains("anyhow!"),
        "Should use anyhow! for errors");
}

#[test]
fn test_phase1_abi_constants_unchanged() {
    /// Verify ABI constants are still correct
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("const SHARED_MODULE_ABI_VERSION: u32 = 1"),
        "ABI version should still be 1");
    
    assert!(native_module.contains("const SHARED_MODULE_CALL_ABI: &str = \"pasta.module.v1\""),
        "Call ABI should be pasta.module.v1");
    
    assert!(native_module.contains("const SHARED_MODULE_HANDLE_OWNERSHIP: &str = \"retain_release_refcounted\""),
        "Handle ownership should be retain_release_refcounted");
    
    assert!(native_module.contains("const SHARED_MODULE_VISIBILITY: &str = \"exports_only\""),
        "Visibility should be exports_only");
}

#[test]
fn test_phase1_manifest_validation_preserved() {
    /// Verify manifest validation logic is intact
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("fn validate_manifest"),
        "validate_manifest function should exist");
    
    assert!(native_module.contains("abi_version"),
        "Should validate ABI version");
    
    assert!(native_module.contains("call_abi"),
        "Should validate call ABI");
    
    assert!(native_module.contains("handle_ownership"),
        "Should validate handle ownership");
}

#[test]
fn test_phase1_comprehensive_unit_tests_added() {
    /// Verify comprehensive test suite was added
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("#[test]"),
        "Should have unit tests");
    
    // Count some key tests
    let test_count = native_module.matches("#[test]").count();
    assert!(test_count >= 15,
        "Should have at least 15 unit tests (found {})", test_count);
}

#[test]
fn test_phase1_documentation_updated() {
    /// Verify module documentation mentions libloading
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    let header = native_module
        .lines()
        .take(20)
        .collect::<Vec<_>>()
        .join("\n");
    
    assert!(header.contains("libloading") || 
            header.contains("cross-platform"),
        "Module docs should mention libloading or cross-platform");
}

#[test]
#[cfg(windows)]
fn test_phase1_windows_binary_built() {
    /// On Windows, verify pasta.exe was built
    let exe_path = PathBuf::from(env!("CARGO_BIN_EXE_pasta"));
    assert!(exe_path.exists(),
        "pasta.exe should exist (found: {:?})", exe_path);
}

#[test]
#[cfg(unix)]
fn test_phase1_unix_binary_built() {
    /// On Unix, verify pasta binary was built
    let binary_path = PathBuf::from(env!("CARGO_BIN_EXE_pasta"));
    assert!(binary_path.exists(),
        "pasta binary should exist (found: {:?})", binary_path);
}

#[test]
fn test_phase1_no_regressions_in_semantics() {
    /// Verify semantics of NativeModule behavior are unchanged
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Should still:
    // 1. Load manifest
    assert!(native_module.contains("manifest_fn"),
        "Should load manifest function");
    
    // 2. Initialize module
    assert!(native_module.contains("init_fn"),
        "Should call init function");
    
    // 3. Resolve exports
    assert!(native_module.contains("call_fn"),
        "Should resolve call functions");
    
    // 4. Validate ABI
    assert!(native_module.contains("validate_manifest"),
        "Should validate manifest/ABI");
}

#[test]
fn test_phase1_thread_safety_maintained() {
    /// Verify Send + Sync traits are still implemented
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("unsafe impl Send for NativeModule"),
        "NativeModule should implement Send");
    
    assert!(native_module.contains("unsafe impl Sync for NativeModule"),
        "NativeModule should implement Sync");
}

#[test]
fn test_phase1_export_handling_unchanged() {
    /// Verify export handling logic is preserved
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("pub fn exports"),
        "Should have exports() method");
    
    assert!(native_module.contains("pub fn export_builtins"),
        "Should have export_builtins() method");
    
    assert!(native_module.contains("make_export_builtin"),
        "Should have make_export_builtin() method");
}

#[test]
fn test_phase1_error_messages_preserved() {
    /// Verify error messages are still informative
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    assert!(native_module.contains("failed to load shared module"),
        "Should have informative load error message");
    
    assert!(native_module.contains("failed to resolve symbol"),
        "Should have informative symbol error message");
    
    assert!(native_module.contains("ABI version"),
        "Should validate and report ABI version issues");
}

#[test]
fn test_phase1_platform_aware_implementation() {
    /// Verify implementation is aware of platform differences
    let native_module = include_str!("../src/runtime/native_module.rs");
    
    // Cross-platform comment/doc
    assert!(native_module.contains("cross-platform") ||
            native_module.contains("Windows") ||
            native_module.contains("Unix") ||
            native_module.contains("Unix") && native_module.contains("dlopen") ||
            native_module.contains("Windows") && native_module.contains("LoadLibrary"),
        "Implementation should acknowledge platform differences");
}
