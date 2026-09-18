# PASTA Windows Roadmap & Architecture

**Version**: 1.0  
**Date**: July 15, 2026  
**Target**: Windows 10+ (x86_64)

---

## Architecture Overview

### Current (Unix Only)
```
┌─────────────────────────────────────────────────────────────┐
│                     PASTA Interpreter                         │
├─────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Lexer      │  │   Parser     │  │  Semantics   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│                                                                 │
│  ┌──────────────────────────────────────────────────────┐    │
│  │          Executor / Runtime                          │    │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐       │    │
│  │  │  Graphics  │ │  Threading │ │   Modules  │       │    │
│  │  └──────┬─────┘ └────────────┘ └─────┬──────┘       │    │
│  │         │                             │              │    │
│  │    [X11 Only]                    [dlopen Only]       │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │  Terminal    │  │  Line Editor │  │    Stdlib    │        │
│  │ (termios)    │  │  (readline)  │  │  (term, fs)  │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│                                                                 │
└─────────────────────────────────────────────────────────────┘
         │
         └─→ [POSIX APIs Only] ↔ Linux / macOS only
```

### Target (Cross-Platform)
```
┌─────────────────────────────────────────────────────────────┐
│                     PASTA Interpreter                         │
├─────────────────────────────────────────────────────────────┤
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │   Lexer      │  │   Parser     │  │  Semantics   │        │
│  └──────────────┘  └──────────────┘  └──────────────┘        │
│                                                                 │
│  ┌──────────────────────────────────────────────────────┐    │
│  │          Executor / Runtime                          │    │
│  │  ┌────────────┐ ┌────────────┐ ┌────────────┐       │    │
│  │  │  Graphics  │ │  Threading │ │   Modules  │       │    │
│  │  ├─X11 (Unix)│ │  (portable)│ │ [libload]  │       │    │
│  │  └─Win32(Win)┘ └────────────┘ └────────────┘       │    │
│  │     (Phase 3)     (verified)     (Phase 1)         │    │
│  └──────────────────────────────────────────────────────┘    │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐        │
│  │  Terminal    │  │  Line Editor │  │    Stdlib    │        │
│  │ (sys console)│  │ (console API)│  │  (portable)  │        │
│  │ ├─termios    │  │ ├─termios    │  └──────────────┘        │
│  │ └─Win32 API  │  │ └─Win32 API  │   (Phase 2)              │
│  └──────────────┘  └──────────────┘                           │
│   (Phase 2)          (Phase 2)                                │
│                                                                 │
└─────────────────────────────────────────────────────────────┘
   ┌─────────────┬────────────┐
   ↓             ↓            ↓
[Linux/macOS] [Windows]  [WSL2 + X11]
(Native)      (Native)   (Compat)
```

---

## Implementation Timeline

### Week 1: Phases 1 & 2 (16-22 hours)

```
Mon: Phase 1 Planning & Setup
  └─ Add libloading to Cargo.toml (30 min)
  └─ Create branch, setup test environment (30 min)

Tue-Wed: Phase 1 Implementation (4-5 hours)
  ├─ Refactor native_module.rs
  ├─ Add cfg(unix) guards
  ├─ Replace dlopen with libloading::Library
  ├─ Build & test on Windows
  └─ Build & test on Linux (regression)

Thu-Fri: Phase 2 Implementation (10-12 hours)
  ├─ Day 1: Windows console API research & setup
  │         Add windows-sys dependency
  │         Implement SetConsoleMode wrappers
  │
  ├─ Day 2: Implement src/stdlib/term.rs
  │         ├─ raw_enable() → SetConsoleMode
  │         ├─ read_key() → ReadConsoleInput
  │         └─ Test on Windows
  │
  └─ Day 3: Implement src/readline.rs
             ├─ Platform-specific input polling
             ├─ Windows ReadConsoleInput path
             └─ Full test on both platforms

Fri: Phase 4 Quick Pass (2-3 hours)
  └─ Module loader, threading, paths review & test
```

### Week 2: Phases 3 & 5 (8-14 hours)

```
Mon-Wed: Phase 3 Decision (0-8 hours)
  ├─ Option A: Defer (keep stub)
  │   └─ Document as Phase 3+ work
  │   └─ Proceed to Phase 5
  │
  └─ Option B: Full Win32 (8-12 hours)
      ├─ Day 1-2: Win32 API research & window creation
      ├─ Day 3: DIB section & rendering
      └─ Day 4: Message loop & events

Thu-Fri: Phase 5 Integration & Testing (4-6 hours)
  ├─ Full build: cargo build --release (both platforms)
  ├─ Full test: cargo test --workspace --all
  ├─ Smoke tests: binary execution, basic features
  └─ Documentation & CI/CD setup (optional)
```

---

## Dependency Graph

```
Phase 1 (libloading)
    ↓
    └──→ Can proceed independently
    
Phase 2 (Windows console)
    ↓
    └──→ Depends on Phase 1 build success only
         (not on libloading functionality)
    
Phase 3 (Graphics)
    ↓
    └──→ Independent; can defer or parallel
    
Phase 4 (Edge cases)
    ↓
    └──→ Can run after Phase 1 builds
    
Phase 5 (Integration)
    ↓
    └──→ Depends on Phases 1, 2, 4 complete
         (Phase 3 optional)
```

**Parallelizable**: Phases 1 & 2 can overlap (different files)  
**Blocking**: Phase 5 blocked until others complete

---

## Risk & Complexity Matrix

```
       Easy              Medium              Hard
Light  ┌─────────────┬──────────────┬──────────────┐
       │             │              │              │
       │ Phase 4.1   │ Phase 4.2    │              │
Effort │ Ext.const   │ Threading    │              │
       │ (1 hr)      │ review       │              │
       │             │ (2 hrs)      │              │
       ├─────────────┼──────────────┼──────────────┤
Medium │ Phase 1     │ Phase 4.3    │ Phase 2      │
       │ libloading  │ Paths review │ Term I/O     │
       │ (4-6 hrs)   │ (1 hr)       │ (12-16 hrs)  │
       ├─────────────┼──────────────┼──────────────┤
Heavy  │             │              │ Phase 3      │
       │             │              │ Win32 graphics
       │             │              │ (8-12 hrs)   │
       └─────────────┴──────────────┴──────────────┘
       
Legend:
├─ Easy: Straightforward, well-documented APIs
├─ Medium: Some new APIs, moderate research
└─ Hard: Complex APIs, requires deep dive

Risk assessment:
├─ Green: Proven solutions (libloading, windows-sys)
├─ Yellow: Medium-complexity platform APIs (console)
└─ Red: Complex graphics (can defer)
```

---

## Detailed Phase Timeline

### Phase 1: Dynamic Loading (Days 1-3)

**Monday: Setup**
- [ ] Clone/checkout repository
- [ ] Create feature branch: `feature/windows-support`
- [ ] Update Cargo.toml with libloading dependency
- [ ] Verify Linux/macOS build still works
- **Deliverable**: Cargo.toml updated, baseline build passes

**Tuesday-Wednesday: Implementation**
- [ ] Analyze current `native_module.rs` (dlopen usage)
- [ ] Identify all `#[cfg]` needs
- [ ] Refactor for cross-platform support:
  - [ ] Add `#[cfg(unix)]` for POSIX imports
  - [ ] Replace `handle: *mut c_void` with `lib: Library`
  - [ ] Update `load()` method
  - [ ] Simplify `Drop` impl
- [ ] Build on both platforms
- [ ] Run native module tests
- **Deliverable**: Cross-platform native module loading

---

### Phase 2: Terminal I/O (Days 3-5)

**Wednesday-Thursday: Term & Readline**

**src/stdlib/term.rs** (3-4 hours)
- [ ] Create Unix conditional module with existing code
- [ ] Create Windows conditional module:
  - [ ] Import Windows Console APIs
  - [ ] Implement `raw_enable()` → `SetConsoleMode()`
  - [ ] Implement `raw_disable()` → `SetConsoleMode()` restore
  - [ ] Implement `read_key()` → `ReadConsoleInput()`
  - [ ] Implement alt screen handling
- [ ] Build & test on Windows
- [ ] Verify Unix behavior unchanged

**src/readline.rs** (3-4 hours)
- [ ] Identify Unix input polling code
- [ ] Extract to platform-specific functions:
  - [ ] `#[cfg(unix)] fn read_byte_with_timeout() { libc::read(...) }`
  - [ ] `#[cfg(windows)] fn read_byte_with_timeout() { ReadConsoleInput(...) }`
- [ ] Keep ANSI escape parsing shared
- [ ] Build & test on Windows
- [ ] Verify arrow keys, history, Ctrl+U/K/etc work

**Friday: Testing & Polish**
- [ ] Full terminal feature tests
- [ ] History persistence test
- [ ] Key binding verification
- **Deliverable**: Functional terminal & line editor on Windows

---

### Phase 3: Graphics (Optional, Days 6-7)

**Option A: Defer (1 hour)**
- [ ] Document phase 3 as "Not yet implemented on Windows"
- [ ] Verify stub compiles
- [ ] Add skip_on_windows for graphics tests

**Option B: Full Implementation (8-12 hours)**

**Monday-Wednesday: Win32 API**
- [ ] Research Win32 window creation:
  - [ ] WNDCLASSEX registration
  - [ ] CreateWindowExW parameters
  - [ ] DIB section creation
  - [ ] Message pump
- [ ] Implement `Win32Window::new()`:
  - [ ] Register window class
  - [ ] Create window with CreateWindowExW
  - [ ] Setup DIB section for rendering
  - [ ] Return initialized struct
- [ ] Implement `blit()`:
  - [ ] Copy canvas data to DIB
  - [ ] Call StretchDIBits or BitBlt
  - [ ] Update display
- [ ] Implement message loop and close handling

**Deliverable**: Native Win32 graphics window (or documented defer)

---

### Phase 4: Edge Cases (Day 4, 2-4 hours)

**Quick Review Pass**
- [ ] `src/mod_loader/`: Add extension conditionals
- [ ] `src/threading/`: Verify portability, run tests
- [ ] `src/interpreter/shell_os/`: Verify path handling
- [ ] Full test suite: `cargo test --workspace --all`
- **Deliverable**: All tests pass on both platforms

---

### Phase 5: Integration (Days 8-9, 4-6 hours)

**Full Build & Test**
- [ ] `cargo clean && cargo build --release` (Windows)
- [ ] `cargo clean && cargo build --release` (Linux/macOS)
- [ ] `cargo test --workspace --all --release` (Windows)
- [ ] `cargo test --workspace --all --release` (Linux/macOS)
- [ ] Manual smoke tests:
  - [ ] Run basic PASTA script
  - [ ] Test interactive CLI
  - [ ] Verify graphics (if implemented)

**Optional: CI/CD Setup**
- [ ] Add Windows build to GitHub Actions
- [ ] Document Windows build/test process

**Deliverable**: Windows MVP complete, all tests pass

---

## File Modification Summary

### Phase 1 Changes

**Cargo.toml**
```toml
[dependencies]
+libloading = "0.8"

[target.'cfg(unix)'.dependencies]
+x11 = { version = "2.3", features = ["xlib"], optional = true }

[features]
-default = ["x11"]
+default = []
+x11 = ["dep:x11"]  # Unix-only feature
```

**src/runtime/native_module.rs** (~50 line changes)
- Remove: `use libc::{dlopen, dlsym, dlclose, dlerror, RTLD_LOCAL, RTLD_NOW};`
- Add: `#[cfg(unix)] use libc::{...};`
- Add: `use libloading::Library;`
- Change struct: `handle: *mut c_void` → `lib: Library`
- Refactor: `load()` method to use `Library::new()`

### Phase 2 Changes

**Cargo.toml**
```toml
[target.'cfg(windows)'.dependencies]
+windows-sys = { version = "0.48", features = [
+    "Win32_Foundation",
+    "Win32_System_Console",
+] }
```

**src/stdlib/term.rs** (~100 line additions)
- Conditional `#[cfg(unix)]` / `#[cfg(windows)]` modules
- Windows Console API implementations

**src/readline.rs** (~80 line additions)
- Platform-specific `read_byte_with_timeout()` implementations
- Conditional `libc::read()` vs `ReadConsoleInput()`

### Phase 3 Changes (Optional)

**src/stdlib/graphics/backend/win32.rs** (~100-300 lines)
- Full Win32 API implementation
- Window creation, rendering, message loop

### Phase 4 Changes (Minimal)

**src/mod_loader/mod_load.rs** (~5 lines)
- Add EXT constant conditional

### Phase 5 Changes

**src/runtime/native_module.rs** (~20 lines)
- Add smoke test for kernel32.dll / libc.so

---

## Success Metrics

### Phase 1 Success
- ✅ `cargo build --release` succeeds on Windows
- ✅ No `dlopen`/`dlsym`/`dlclose` link errors
- ✅ All Unix tests still pass (regression test)

### Phase 2 Success
- ✅ Terminal features work on Windows (raw mode, keys, history)
- ✅ `cargo build --release` succeeds
- ✅ All Unix terminal tests still pass

### Phase 3 Success (if implemented)
- ✅ Windows graphics window appears and closes cleanly
- ✅ Canvas rendering works (pixels displayed)
- ✅ Or: Stub compiles without errors (if deferred)

### Phase 4 Success
- ✅ `cargo test --workspace --all` passes on Windows
- ✅ `cargo test --workspace --all` passes on Linux (regression)
- ✅ No platform-specific test failures

### Phase 5 Success (MVP Complete)
- ✅ `cargo build --release` produces `pasta.exe`
- ✅ Binary runs: `pasta.exe` → `> help` → `> exit`
- ✅ Basic script works: `pasta.exe script.ps`
- ✅ All major features functional
- ✅ Full test suite passes

---

## Rollback Plan

If at any phase a blocker is hit:

1. **Phase 1 blocker**: Revert to Unix-only (undo libloading, keep dlopen)
2. **Phase 2 blocker**: Keep term/readline as Unix-only (document Windows limitation)
3. **Phase 3 blocker**: Keep graphics stub (defer implementation)
4. **Phase 4 blocker**: Cherry-pick non-blocking fixes
5. **Phase 5 blocker**: Fix failing tests, don't advance CI until green

**Recommendation**: Commit to main branch only after Phase 5 complete.

---

## Communication Checkpoints

| Checkpoint | Stakeholders | Update |
|------------|--------------|--------|
| After Phase 1 | Build team | "Dynamic loading works cross-platform" |
| After Phase 2 | Product team | "Terminal I/O ready on Windows" |
| After Phase 4 | QA team | "Ready for testing" |
| After Phase 5 | Release team | "Windows MVP complete" |

---

## References & Resources

### Documentation
- [libloading docs](https://docs.rs/libloading/)
- [windows-sys docs](https://docs.rs/windows-sys/)
- [Windows Console API](https://learn.microsoft.com/en-us/windows/console/)
- [Rust cfg attribute](https://doc.rust-lang.org/reference/conditional-compilation.html)

### Code References (in PASTA)
- **X11 backend**: `src/stdlib/graphics/backend/x11.rs` (Unix example)
- **Current term**: `src/stdlib/term.rs` (Unix-only code to port)
- **Current readline**: `src/readline.rs` (Unix-only code to port)

### External Examples
- [libloading examples](https://github.com/nagisa/rust_libloading/tree/master/examples)
- [windows-sys samples](https://github.com/microsoft/windows-rs/tree/master/examples)

---

## Final Notes

- **Windows 10+** is the minimum target (for Virtual Terminal Processing support in console)
- **x86-64** architecture is the initial target (ARM64 Windows possible but not tested here)
- **Rust 1.70+** recommended (stable, good toolchain support)
- **Precompiled modules (.dll)** are out of scope (compilation setup separate)
- **Graphics on Windows** is optional for MVP (can use X11 on WSL2 for testing)

**Key principle**: Preserve Linux behavior while adding Windows support. All changes use conditional compilation (`#[cfg(...)]`) to ensure cross-platform safety.

