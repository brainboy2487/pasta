# PASTA Windows Implementation Checklist

**Quick Reference for Implementation**

---

## Phase 1: Dynamic Module Loading (CRITICAL) — 4-6 hours

### Cargo.toml Updates
- [ ] Add `libloading = "0.8"` to `[dependencies]`
- [ ] Add conditional `x11` under `[target.'cfg(unix)'.dependencies]`
- [ ] Remove `x11` from default `[features]` (only Unix)

### src/runtime/native_module.rs Refactoring
- [ ] Remove `dlopen, dlsym, dlclose, dlerror, RTLD_LOCAL, RTLD_NOW` from top-level imports
- [ ] Add `#[cfg(unix)]` conditional for POSIX imports
- [ ] Add `use libloading::Library`
- [ ] Replace `handle: *mut c_void` with `lib: Library` in struct
- [ ] Update `load()` method to use `Library::new(path)`
- [ ] Simplify `Drop` impl (libloading auto-drops)
- [ ] Update `load_symbol()` to use libloading APIs

### Build & Test
- [ ] `cargo clean && cargo build --release -v` (should succeed on Windows)
- [ ] Verify no dlopen/dlsym link errors
- [ ] `cargo test --lib runtime` (Unix tests still pass)

**Success**: Compiles on both Windows and Unix, no dynamic loader errors.

---

## Phase 2: Terminal & Line Editor (HIGH) — 12-16 hours

### Cargo.toml: Add Windows Dependencies
- [ ] Add to `[target.'cfg(windows)'.dependencies]`:
  ```toml
  windows-sys = { version = "0.48", features = [
      "Win32_Foundation",
      "Win32_System_Console",
      "Win32_System_LibraryLoader",
  ] }
  ```

### src/stdlib/term.rs — Terminal Control
- [ ] Create `#[cfg(unix)]` mod for Unix termios code
- [ ] Create `#[cfg(windows)]` mod for Windows console code
- [ ] Implement Windows equivalents:
  - [ ] `raw_enable()` → `SetConsoleMode()` + `ENABLE_RAW_INPUT`
  - [ ] `raw_disable()` → `SetConsoleMode()` restore
  - [ ] `read_key()` → `ReadConsoleInput()`
  - [ ] `enter_alt_screen()` → ANSI `\x1b[?1049h` (or `CreateConsoleScreenBuffer`)
  - [ ] `exit_alt_screen()` → ANSI `\x1b[?1049l`

### src/readline.rs — Line Editor
- [ ] Extract platform-specific input polling:
  - [ ] Create `#[cfg(unix)] fn read_byte_with_timeout(...)`
  - [ ] Create `#[cfg(windows)] fn read_byte_with_timeout(...)`
- [ ] Unix: `libc::select()` + `libc::read()`
- [ ] Windows: `ReadConsoleInput()` or `ReadFile()` with timeout
- [ ] Keep ANSI escape sequence parsing (shared code)

### Build & Test
- [ ] `cargo build --release -v` (no console API errors)
- [ ] `cargo test --lib readline` (arrow keys, history, Ctrl+U/K, etc.)
- [ ] `cargo test --lib term` (raw mode transitions)

**Success**: Terminal raw mode works, line editor responds to arrows/keys, history works.

---

## Phase 3: Graphics Backend (MEDIUM) — 8-12 hours or 0 if deferring

### Verify Routing (src/stdlib/graphics/backend/mod.rs)
- [ ] Check `#[cfg(target_os = "linux")]` correctly selects X11
- [ ] Check `#[cfg(windows)]` correctly selects Win32
- [ ] Verify feature gating: `#[cfg(all(target_os = "linux", feature = "x11"))]`

### Option A: Full Win32 Implementation
- [ ] Add Windows graphics dependencies (windows-sys GUI features)
- [ ] Implement `src/stdlib/graphics/backend/win32.rs`:
  - [ ] `Win32Window::new()` → RegisterClass + CreateWindowExW
  - [ ] `blit()` → CreateDIBSection + StretchDIBits
  - [ ] `is_open()` → PeekMessageW message loop
  - [ ] `close()` → DestroyWindow + UnregisterClass
  - [ ] Message pump (WM_CLOSE, WM_PAINT)
- [ ] Test window creation, rendering, event loop

### Option B: Keep Stub (for MVP)
- [ ] Document graphics as "not yet implemented on Windows"
- [ ] Stub compiles without errors
- [ ] Skip graphics tests on Windows (or test X11 via WSL)

### Build & Test
- [ ] `cargo build --release -v` (graphics compiles)
- [ ] `cargo test --lib graphics` (any existing tests pass)

**Success**: No compilation errors; graphics stub or full impl works.

---

## Phase 4: Edge Cases & Polish (MEDIUM/LOW) — 4-6 hours

### Module Loader (src/mod_loader/)
- [ ] Review file extension handling (`.so` vs `.dll`)
- [ ] Add conditional: `#[cfg(unix)] const EXT = "so"; #[cfg(windows)] const EXT = "dll";`
- [ ] Test module loading: `cargo test --lib mod_loader`

### Threading (src/threading/, src/runtime/threading.rs)
- [ ] Review for direct `pthread_*` calls
- [ ] Verify `std::thread::spawn` used everywhere
- [ ] Check for portability: `cargo test --lib threading`

### Path Handling (src/interpreter/shell_os/cli/cli.rs)
- [ ] Verify `PathBuf` joins work (already portable)
- [ ] Test path handling with `\` on Windows

### Full Test Suite
- [ ] `cargo test --workspace --all --release` on Windows
- [ ] `cargo test --workspace --all --release` on Linux (regression)

**Success**: All tests pass on both platforms; no obvious Windows-specific errors.

---

## Phase 5: Integration & Smoke Tests (CRITICAL) — 4-6 hours

### Native Module Smoke Test
- [ ] Add to `src/runtime/native_module.rs`:
  ```rust
  #[test]
  fn smoke_load_kernel32_or_libc() {
      // Load system library to verify dynamic loading
  }
  ```

### Binary Build & Run
- [ ] `cargo build --release` produces `pasta.exe` on Windows
- [ ] Test manual invocation: `./target/release/pasta.exe`
- [ ] Run simple PASTA script: `echo 'print("Hello!")' > test.ps && pasta test.ps`
- [ ] Test interactive mode: `pasta` → `help` → `exit`

### Feature Tests
- [ ] Graphics: `cargo test --lib graphics -- --nocapture`
- [ ] Terminal: `cargo test --lib term readline -- --nocapture`
- [ ] Module loading: `cargo test --lib mod_loader -- --nocapture`
- [ ] Threading: `cargo test --lib threading -- --nocapture`

### Optional: CI/CD
- [ ] Add `.github/workflows/windows.yml` for automated Windows builds
- [ ] Or document manual testing procedure

**Success**: All features work end-to-end on Windows.

---

## Quick Build Commands

```powershell
# Full clean build
cargo clean
cargo build --release -v

# Run all tests
cargo test --workspace --all --release

# Test specific module
cargo test --lib runtime
cargo test --lib term
cargo test --lib readline

# Build binary only
cargo build --release

# Run tests with output
cargo test --lib module_name -- --nocapture
```

---

## Common Errors & Fixes

| Error | Phase | Fix |
|-------|-------|-----|
| `error: unresolved import 'libc::dlopen'` | 1 | Add `#[cfg(unix)]` guard or use libloading |
| `error: use of undeclared type 'termios'` | 2 | Add `#[cfg(windows)]` console code |
| `error: 'ReadConsoleInput' is not in scope` | 2 | Ensure windows-sys features include Win32_System_Console |
| `error: function 'tcgetattr' not found` | 2 | Already conditionally compiled; verify cfg guards |
| Link error on Windows | 1 | Check libloading is in Cargo.toml |

---

## Validation Checklist (Final)

Before declaring success, verify:

- [ ] Phase 1: `cargo build --release` succeeds
- [ ] Phase 2: `cargo build --release` succeeds
- [ ] Phase 3: `cargo build --release` succeeds
- [ ] Phase 4: `cargo test --workspace --all` passes
- [ ] Phase 5: Binary runs, smoke tests pass
- [ ] All Linux tests still pass (regression check)
- [ ] Windows and Linux can coexist in same codebase
- [ ] No Windows-specific code has `#[cfg(unix)]` (should be `#[cfg(windows)]` or conditional)

---

## Reference: Files Modified Summary

| File | Changes | Lines | Phase |
|------|---------|-------|-------|
| `Cargo.toml` | Add libloading, windows-sys, conditional deps | ~10 | 1 |
| `src/runtime/native_module.rs` | Replace dlopen with libloading | ~50 | 1 |
| `src/stdlib/term.rs` | Add Windows console code | ~100 | 2 |
| `src/readline.rs` | Add Windows ReadConsoleInput | ~80 | 2 |
| `src/stdlib/graphics/backend/win32.rs` | Full implementation (or keep stub) | 100-300 | 3 |
| `src/mod_loader/*.rs` | Add EXT conditional | ~5 | 4 |
| `src/runtime/native_module.rs` | Add smoke test | ~20 | 5 |

**Total estimated changes**: ~400-700 lines of code

---

## Effort Breakdown

- **Phase 1** (Dynamic loading): 4-6 hrs
- **Phase 2** (Terminal I/O): 12-16 hrs ← Most complex
- **Phase 3** (Graphics): 8-12 hrs (or 0 if deferred)
- **Phase 4** (Edge cases): 4-6 hrs
- **Phase 5** (Integration): 4-6 hrs

**Total**: 40-60 hours for full implementation (or 32-50 hours if graphics deferred)

---

## Status Tracking

- [ ] **Audit Complete**: 2026-07-15
- [ ] **Phase 1 Complete**: _______
- [ ] **Phase 2 Complete**: _______
- [ ] **Phase 3 Complete**: _______
- [ ] **Phase 4 Complete**: _______
- [ ] **Phase 5 Complete**: _______
- [ ] **Windows MVP Ready**: _______

