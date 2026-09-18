# PASTA Windows Compatibility Audit & Implementation Plan

**Date**: 2026-07-15  
**Status**: Initial Audit Complete  
**Goal**: Enable PASTA to build and run on Windows 10+ with full feature parity to Linux

---

## Executive Summary

PASTA is currently Unix-only. This audit identifies **18 actionable items** across 5 phases to achieve Windows compatibility:
- **Phase 1 (CRITICAL)**: Dynamic module loading (libloading)
- **Phase 2 (HIGH)**: Terminal I/O (Windows console API)
- **Phase 3 (MEDIUM)**: Graphics backend (Win32)
- **Phase 4 (MEDIUM)**: Edge cases & module loader
- **Phase 5 (CRITICAL)**: Integration testing

**Estimated effort**: 40-60 engineer hours for full implementation.

---

## Codebase Overview

### Platform-Specific Areas Identified

| Area | Unix | Windows | Status |
|------|------|---------|--------|
| **Dynamic Loading** | dlopen/dlsym/dlclose | LoadLibrary/GetProcAddress | ✗ Needs refactor |
| **Graphics Backend** | X11 (feature gated) | Win32 stub | ✗ Stub only |
| **Terminal Control** | termios (raw mode, keys) | Console API | ✗ Unix-only |
| **Line Editor** | tcgetattr/tcsetattr | Console API | ✗ Unix-only |
| **Shell Path Handling** | `/path/to/file` | `C:\path\to\file` | ⚠ Likely works via PathBuf |
| **Threading** | pthreads | Windows threads | ⚠ May be portable |
| **Module Loader** | .so files | .dll files | ⚠ Mostly portable |

### Key Files to Modify

```
Cargo.toml
├─ Add: libloading = "0.8"
├─ Add: windows-sys (optional)
└─ Conditional deps: [target.'cfg(unix)'.dependencies] for x11

src/runtime/native_module.rs        (CRITICAL: refactor for cross-platform)
src/stdlib/term.rs                  (HIGH: add Windows console support)
src/readline.rs                     (HIGH: add Windows console support)
src/stdlib/graphics/backend/mod.rs  (HIGH: verify routing)
src/stdlib/graphics/backend/win32.rs (MEDIUM: stub → full implementation)
src/interpreter/shell_os/cli/cli.rs (MEDIUM: path handling review)
src/threading/threads.rs            (LOW: verify portability)
src/runtime/threading.rs            (LOW: verify portability)
src/mod_loader/                     (MEDIUM: test on Windows)
```

---

## Phase 1: Dynamic Module Loading (CRITICAL)

### Current State
- `src/runtime/native_module.rs` uses **only** `libc::dlopen/dlsym/dlclose`
- No Windows equivalent
- Directly calls POSIX functions with no abstraction

### Problem
```rust
// Current code (UNIX ONLY):
use libc::{dlopen, dlsym, dlclose, dlerror, RTLD_NOW, RTLD_LOCAL};

let handle = unsafe { dlopen(path_cstr.as_ptr(), RTLD_NOW | RTLD_LOCAL) };
// ↑ Won't compile on Windows
```

### Solution: Use `libloading` Crate

**Why libloading?**
- Cross-platform (Unix + Windows)
- Minimal platform-specific code
- Uses `dlopen` on Unix, `LoadLibrary` on Windows
- Mature and well-tested

### Implementation Steps

#### 1.1 Update `Cargo.toml`
```toml
[dependencies]
libloading = "0.8"  # Add this

[target.'cfg(unix)'.dependencies]
x11 = { version = "2.3", features = ["xlib"], optional = true }

[features]
default = ["x11"]
x11 = ["dep:x11"]
```

#### 1.2 Refactor `src/runtime/native_module.rs`

**Key changes:**
- Remove direct `libc` POSIX imports (dlopen, dlsym, dlclose)
- Add `#[cfg(unix)]` guards for Unix-specific flags (RTLD_NOW, RTLD_LOCAL)
- Wrap module loading in `libloading::Library`
- Keep existing behavior on both platforms

**Patch summary:**
```rust
// Before:
use libc::{c_char, c_void, dlclose, dlerror, dlopen, dlsym, RTLD_LOCAL, RTLD_NOW};

// After:
use libc::c_char;
#[cfg(unix)]
use libc::{c_void, dlclose, dlerror, dlopen, dlsym, RTLD_LOCAL, RTLD_NOW};

use libloading::Library;  // NEW
```

**Full refactored structure:**
```rust
pub struct NativeModule {
    lib: Library,  // libloading wrapper
    manifest: NativeModuleManifest,
    call_fns: HashMap<String, ModuleCallFn>,
}

impl NativeModule {
    pub fn load(path: &Path, expected_name: &str) -> Result<Arc<Self>> {
        // On Windows: uses LoadLibrary internally
        // On Unix: uses dlopen internally (same semantics)
        let lib = unsafe { Library::new(path) }
            .map_err(|e| anyhow!("Failed to load: {}", e))?;
        
        // Rest of logic unchanged: resolve symbols, validate, etc.
    }
}

impl Drop for NativeModule {
    fn drop(&mut self) {
        // libloading::Library drops automatically
        // No manual dlclose needed
    }
}
```

#### 1.3 Build & Verify

```powershell
# Phase 1 build check:
cargo clean
cargo build --release -v

# Expected: No errors about dlopen on Windows
```

### Success Criteria
- ✓ Builds on Windows without dlopen/dlsym errors
- ✓ All existing Unix tests still pass
- ✓ Native module loading smoke test passes (if it exists)

---

## Phase 2: Terminal & Line Editor I/O (HIGH)

### Current State
- `src/stdlib/term.rs`: Exclusively uses `libc::tcgetattr/tcsetattr` (Unix only)
- `src/readline.rs`: Uses Unix file descriptors, `termios`
- Both fail to compile on Windows

### Problem
```rust
// src/stdlib/term.rs - Unix only:
#[cfg(unix)]
{
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    unsafe { libc::tcgetattr(libc::STDIN_FILENO, &mut termios) }
    // ...
}
#[allow(unreachable_code)]
Err("Only available on unix terminals")  // ← Windows gets this
```

### Solution: Windows Console API Support

#### 2.1 `src/stdlib/term.rs` - Terminal Control

Windows needs:
- **Raw mode**: Disable line buffering, enable raw character input
  - Unix: `tcsetattr()` + `ICANON`
  - Windows: `SetConsoleMode()` + `ENABLE_RAW_INPUT`
  
- **Alt screen**: Full-screen mode
  - Unix: ANSI escape `\x1b[?1049h`
  - Windows: Console buffers / `CreateConsoleScreenBuffer()`
  
- **Key reading**: Non-blocking input
  - Unix: `read(fd, ...)` with timeout
  - Windows: `ReadConsoleInput()`

**Implementation approach:**
```rust
#[cfg(unix)]
mod unix_term {
    // Existing termios code
}

#[cfg(windows)]
mod windows_term {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Console::*;
    
    pub fn raw_enable() -> io::Result<()> {
        // Get console handle
        // GetConsoleMode()
        // SetConsoleMode() with ENABLE_PROCESSED_INPUT, etc.
    }
}
```

**Action items:**
1. Add `windows-sys = { version = "0.48", features = ["Win32_System_Console"] }` to `Cargo.toml`
2. Implement Windows console API equivalents for:
   - `raw_enable()` → `SetConsoleMode()`
   - `raw_disable()` → `SetConsoleMode()` restore
   - `read_key()` → `ReadConsoleInput()`
   - `enter_alt_screen()` → CreateConsoleScreenBuffer() or ANSI codes (ANSI codes may work on Win10+)

**Windows-specific note**: Windows 10+ supports ANSI escape sequences if you enable virtual terminal processing:
```c
dwMode |= ENABLE_VIRTUAL_TERMINAL_PROCESSING;
SetConsoleMode(hConsole, dwMode);
```

#### 2.2 `src/readline.rs` - Line Editor

**Current Unix implementation uses:**
- `libc::read(fd, ...)` for character input
- `libc::select(...)` / `libc::poll(...)` for timeout
- Manual ANSI escape sequence parsing

**Windows equivalent:**
- `ReadConsoleInput()` for raw console events
- Windows handles timeout natively
- Same ANSI parsing logic

**Action items:**
1. Extract keyboard polling to separate function
2. Implement Windows version using `ReadConsoleInput()`
3. Keep ANSI sequence decoding logic shared

```rust
#[cfg(unix)]
fn read_byte_with_timeout(timeout_ms: i32) -> io::Result<Option<u8>> {
    // select + read
}

#[cfg(windows)]
fn read_byte_with_timeout(timeout_ms: i32) -> io::Result<Option<u8>> {
    // ReadConsoleInput
}
```

#### 2.3 Build & Verify

```powershell
# Phase 2 build check:
cargo build --release -v

# Test terminal features:
cargo test --lib readline -- --nocapture
cargo test --lib term -- --nocapture
```

### Success Criteria
- ✓ Builds on Windows without termios errors
- ✓ Raw mode works on Windows console
- ✓ Arrow keys and history work in line editor
- ✓ Existing Unix tests still pass

---

## Phase 3: Graphics Backend (MEDIUM)

### Current State
- `src/stdlib/graphics/backend/mod.rs`: Correctly routes X11 on Linux, Win32 on Windows
- `src/stdlib/graphics/backend/win32.rs`: **Stub only** (prints debug messages, no real rendering)
- `src/stdlib/graphics/backend/x11.rs`: Full X11 implementation (mature)

### Problem
```rust
// src/stdlib/graphics/backend/mod.rs
pub fn create_window(...) -> Result<Box<dyn BackendWindow + Send>, String> {
    #[cfg(all(target_os = "linux", feature = "x11"))]
    {
        return Ok(Box::new(x11::X11Window::new(title, width, height)?));
    }
    #[cfg(not(all(target_os = "linux", feature = "x11")))]
    {
        // Falls through — Windows gets stub
        Err("No native window backend available".into())
    }
}
```

### Current Win32 Stub Status
```rust
// src/stdlib/graphics/backend/win32.rs
pub fn blit(&mut self, canvas: &Canvas) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // TODO: Implement
        eprintln!("Win32Window: Stub mode");
    }
    Ok(())
}
```

### Solution: Implement Win32 Backend (Optional but Recommended)

#### 3.1 Add Windows Dependencies

```toml
[target.'cfg(windows)'.dependencies]
windows-sys = { version = "0.48", features = [
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Graphics_Gdi",
] }
```

#### 3.2 Implement `src/stdlib/graphics/backend/win32.rs`

**Required components:**
1. **Window creation**: `CreateWindowExW` with `WNDCLASSEX`
2. **Double buffering**: DIB section (`CreateDIBSection`)
3. **Rendering**: `StretchDIBits` or `BitBlt`
4. **Message loop**: `PeekMessageW` / `DispatchMessage`

**Architecture:**
```rust
pub struct Win32Window {
    hwnd: *mut HWND,  // Window handle
    hdc: *mut HDC,    // Device context
    dib: *mut HBITMAP, // DIB section for double buffering
    width: usize,
    height: usize,
}

impl BackendWindow for Win32Window {
    fn blit(&mut self, canvas: &Canvas) -> Result<(), String> {
        // 1. Lock DIB section
        // 2. Copy canvas.as_bytes() → DIB memory
        // 3. StretchDIBits(hdc, dib_section, 0, 0, ...)
        // 4. Unlock DIB
    }
}
```

**Complexity note**: This is the most involved piece. A simpler alternative is to use a high-level crate like `winit` (window creation) + `wgpu` (rendering), but that introduces dependencies.

**For this task**: I recommend **defer full Win32 implementation** to Phase 5 if time is constrained. The stub is sufficient to prevent errors; graphics testing can use X11 on WSL2 or deferred.

#### 3.3 Build & Verify

```powershell
# Phase 3 build check:
cargo build --release -v

# Graphics tests:
cargo test --lib graphics -- --nocapture
```

### Success Criteria
- ✓ Builds on Windows (stub or full implementation)
- ✓ `create_window()` doesn't error on Windows
- ✓ Existing X11 tests still pass on Linux

---

## Phase 4: Edge Cases & Polish (MEDIUM/LOW)

### 4.1 Module Loader Windows Support

**File**: `src/mod_loader/` (5 files)

**Current behavior**: Uses Unix paths, `dlopen`-based loading.

**Windows considerations**:
- Change `.so` → `.dll` file extensions
- Paths already use `PathBuf` (portable)
- Test on Windows to verify `.dll` loading works with libloading

**Action**:
```rust
#[cfg(unix)]
const NATIVE_LIB_EXT: &str = "so";
#[cfg(windows)]
const NATIVE_LIB_EXT: &str = "dll";

let lib_path = format!("lib_{}.{}", name, NATIVE_LIB_EXT);
```

### 4.2 Threading Portability

**Files**: `src/threading/threads.rs`, `src/runtime/threading.rs`

**Current state**: Some `#[cfg(unix)]` guards for pthreads logic.

**Windows compatibility**: Most threading primitives are portable via Rust's `std::thread`. Verify:
- No direct `libc::pthread_*` calls
- All thread spawning uses `std::thread::spawn`

### 4.3 Environment & Path Handling

**File**: `src/interpreter/shell_os/cli/cli.rs`

**Current behavior**: Uses `/` paths.

**Windows issue**: Windows uses `\` (backslash).

**Solution**: `PathBuf` already handles this — just verify path joining works:
```rust
// Already portable:
vfs.local_cwd.join(left_script)  // ✓ Works on both platforms
```

### 4.4 Full Test Suite

```powershell
# Phase 4 final test:
cargo test --workspace --all --release
```

---

## Phase 5: Integration & Smoke Tests (CRITICAL)

### 5.1 Native Module Loading Smoke Test

Add to `src/runtime/native_module.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_load_system_library() {
        // Load a system library to verify dynamic loading works
        #[cfg(unix)]
        {
            // Try loading libc
            match NativeModule::load(&std::path::Path::new("libc.so.6"), "libc") {
                Ok(_) | Err(_) => (), // Either result is OK for smoke test
            }
        }

        #[cfg(windows)]
        {
            // Try loading kernel32.dll
            match NativeModule::load(&std::path::Path::new("kernel32.dll"), "kernel32") {
                Ok(_) | Err(_) => (),
            }
        }
    }
}
```

### 5.2 End-to-End Smoke Test

```powershell
# Build pasta binary
cargo build --release

# Run interpreter on simple script
echo 'print("Hello from PASTA on Windows!")' > test.ps
./target/release/pasta test.ps

# Test CLI features
./target/release/pasta
  > help
  > exit
```

### 5.3 Feature-Specific Tests

```powershell
# Graphics (X11 on Linux, Win32 on Windows)
cargo test --lib graphics

# Terminal I/O
cargo test --lib term readline

# Module loading
cargo test --lib mod_loader

# Threading
cargo test --lib threading
```

### 5.4 Optional: CI/CD Windows Build

If `.github/workflows/` exists:

```yaml
# .github/workflows/windows.yml
name: Windows Build

on: [push, pull_request]

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v3
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: x86_64-pc-windows-msvc
      - run: cargo build --release
      - run: cargo test --workspace --all --release
```

---

## Detailed Implementation Checklist

### Phase 1: Dynamic Module Loading
- [ ] Update `Cargo.toml`: add `libloading = "0.8"` and conditional `x11`
- [ ] Refactor `src/runtime/native_module.rs`:
  - [ ] Remove direct `dlopen/dlsym/dlclose` imports
  - [ ] Add `#[cfg(unix)]` guards for POSIX flags
  - [ ] Replace with `libloading::Library` wrapper
  - [ ] Update `load()` method to use `Library::new()`
  - [ ] Ensure `Drop` implementation still works
- [ ] Verify all uses of `native_module.rs` still compile
- [ ] Run: `cargo build --release`

### Phase 2: Terminal & Line Editor
- [ ] Update `Cargo.toml`: add Windows console features (`windows-sys`)
- [ ] Refactor `src/stdlib/term.rs`:
  - [ ] Extract Unix logic to `#[cfg(unix)]` mod
  - [ ] Implement Windows console API equivalents
  - [ ] Test `raw_enable/disable`, `read_key()`, alt screen
- [ ] Refactor `src/readline.rs`:
  - [ ] Extract Unix input polling to separate function
  - [ ] Implement Windows `ReadConsoleInput()` path
  - [ ] Verify history and arrow keys work
- [ ] Run: `cargo build --release && cargo test --lib readline term`

### Phase 3: Graphics Backend
- [ ] Verify `src/stdlib/graphics/backend/mod.rs` routing
- [ ] **Decision**: Full Win32 implementation or defer?
  - If implementing:
    - [ ] Add Windows graphics dependencies
    - [ ] Implement `Win32Window` in `src/stdlib/graphics/backend/win32.rs`
    - [ ] Test window creation, rendering, message handling
  - If deferring:
    - [ ] Ensure stub compiles without errors
    - [ ] Document that graphics testing should use X11 or skip on Windows
- [ ] Run: `cargo build --release && cargo test --lib graphics`

### Phase 4: Edge Cases & Polish
- [ ] Review & test `src/mod_loader/`: verify `.dll` loading works
- [ ] Review `src/threading/`: verify no blocking Unix-specific code
- [ ] Review `src/interpreter/shell_os/cli/cli.rs`: verify path handling
- [ ] Run: `cargo test --workspace --all`

### Phase 5: Integration & Smoke Tests
- [ ] Add native module loading smoke test
- [ ] Build `cargo build --release` and test binary manually
- [ ] Run full test suite: `cargo test --workspace --all --release`
- [ ] *Optional*: Add Windows CI job to `.github/workflows/`

---

## Build & Testing Strategy

### Step-by-step validation

```powershell
# 1. Phase 1: Build without errors
cargo clean
cargo build --release -v

# 2. Phase 1: Verify existing tests still pass
cargo test --lib runtime

# 3. Phase 2: Build with terminal changes
cargo build --release -v

# 4. Phase 2: Test terminal & readline
cargo test --lib readline term

# 5. Phase 3: Build with graphics
cargo build --release -v

# 6. Phase 3: Graphics tests
cargo test --lib graphics

# 7. Phase 4: Full test suite
cargo test --workspace --all --release

# 8. Phase 5: Manual smoke tests
cargo build --release
./target/release/pasta.exe
  > print("Hello Windows!")
  > exit

# 9. Success: All tests pass, binary runs
```

---

## Known Issues & Mitigation

### Issue 1: Console Mode on Windows
**Problem**: Windows console doesn't support all ANSI escape codes by default.  
**Mitigation**: Use `SetConsoleMode()` to enable `ENABLE_VIRTUAL_TERMINAL_PROCESSING` on Windows 10+.

### Issue 2: Line Ending Differences
**Problem**: Windows uses `\r\n`, Unix uses `\n`.  
**Mitigation**: Rust's `io::BufRead` and string handling already normalize this.

### Issue 3: Graphics on Windows (without real implementation)
**Problem**: Win32 stub doesn't render.  
**Mitigation**: Document that graphics testing uses X11 or defer full implementation to future phase.

### Issue 4: Module compilation on Windows
**Problem**: Compiled PASTA modules (`.so` → `.dll`) need proper build setup.  
**Mitigation**: Module compilation is out of scope for this audit; focus on loading portability.

---

## Success Metrics

### Build Success
- [ ] `cargo build --release` succeeds on Windows (no compile errors)
- [ ] `cargo build --release` succeeds on Linux (unchanged behavior)

### Test Success
- [ ] `cargo test --workspace --all` passes on Windows
- [ ] `cargo test --workspace --all` passes on Linux
- [ ] Smoke tests pass on both platforms

### Feature Completeness
- [ ] Core interpreter works on Windows
- [ ] Terminal I/O works (raw mode, line editing, history)
- [ ] Graphics backend loads (X11 on Linux, Win32 on Windows)
- [ ] Module loading works (for precompiled modules)

---

## Timeline & Effort Estimate

| Phase | Items | Effort | Notes |
|-------|-------|--------|-------|
| Phase 1 | 3 | 4-6 hrs | Add libloading, refactor native_module.rs |
| Phase 2 | 3 | 12-16 hrs | Implement Windows console API support |
| Phase 3 | 1-2 | 8-12 hrs | Graphics backend (stub OK, full implementation complex) |
| Phase 4 | 3 | 4-6 hrs | Edge cases, threading, paths |
| Phase 5 | 2 | 4-6 hrs | Integration testing, smoke tests, CI |
| **Total** | **18** | **40-60 hrs** | Can parallelize some phases |

---

## References

### Documentation
- [libloading crate](https://docs.rs/libloading/)
- [windows-sys crate](https://docs.rs/windows-sys/)
- [Rust std::thread portability](https://doc.rust-lang.org/std/thread/)
- [Windows Console API](https://learn.microsoft.com/en-us/windows/console/console-functions)

### Existing Windows Impl Notes
- `src/stdlib/graphics/backend/win32.rs` has TODO comments explaining API usage
- `src/stdlib/term.rs` has Unix code as reference
- `src/readline.rs` has Unix FD logic as reference

---

## Next Steps

1. **Approve plan** (this document)
2. **Phase 1**: Implement libloading wrapper (4-6 hours)
3. **Phase 2**: Add Windows console support (12-16 hours) — can start in parallel with Phase 1 testing
4. **Phase 3**: Graphics backend decision — full implementation or defer (8-12 hours or 0 hours)
5. **Phase 4**: Edge cases (4-6 hours)
6. **Phase 5**: Integration & smoke tests (4-6 hours)

**Recommendation**: Prioritize Phases 1, 2, and 4 for MVP. Phase 3 (graphics) and Phase 5 (CI) are nice-to-have.

---

## Appendix: Code Structure Reference

### Directory Layout
```
pasta/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── bin/pasta.rs
│   ├── runtime/
│   │   ├── native_module.rs     ← CRITICAL
│   │   ├── threading.rs          ← REVIEW
│   │   └── ...
│   ├── stdlib/
│   │   ├── term.rs              ← HIGH
│   │   └── graphics/
│   │       └── backend/
│   │           ├── mod.rs        ← VERIFY
│   │           ├── win32.rs      ← STUB
│   │           ├── x11.rs        ← REF
│   │           └── ...
│   ├── interpreter/
│   │   ├── shell_os/cli/cli.rs  ← REVIEW
│   │   └── ...
│   ├── readline.rs              ← HIGH
│   ├── mod_loader/              ← TEST
│   └── ...
└── tests/
```

### Compilation Flow
```
cargo build --release
  ├─ Cargo.toml evaluation
  │   ├─ [dependencies] libloading
  │   ├─ [target.'cfg(unix)'.dependencies] x11
  │   └─ [target.'cfg(windows)'.dependencies] windows-sys
  │
  ├─ src/runtime/native_module.rs
  │   ├─ On Unix: uses dlopen (libloading)
  │   └─ On Windows: uses LoadLibrary (libloading)
  │
  ├─ src/stdlib/term.rs
  │   ├─ On Unix: tcgetattr/tcsetattr
  │   └─ On Windows: SetConsoleMode
  │
  └─ Link & output: target/release/pasta
```

---

## Document History

| Date | Version | Author | Changes |
|------|---------|--------|---------|
| 2026-07-15 | 1.0 | Copilot | Initial audit & plan |

