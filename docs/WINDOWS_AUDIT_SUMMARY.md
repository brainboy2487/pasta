# PASTA Windows Compatibility Audit — Executive Summary

**Completed**: July 15, 2026  
**Reviewed by**: GitHub Copilot CLI  
**Status**: ✅ Full audit complete; implementation plan ready

---

## Audit Findings

### Codebase Status: **MOSTLY UNIX-ONLY** ⚠️

PASTA is currently Unix-only with minimal Windows compatibility. However, the architecture is **well-positioned** for cross-platform support:

- **GOOD**: Most application logic is platform-independent
- **GOOD**: Graphics backend has conditional compilation structure
- **CONCERN**: Dynamic module loading uses only POSIX APIs
- **CONCERN**: Terminal I/O is Unix-specific
- **CONCERN**: No Windows CI/build testing

---

## Key Findings by Component

### 1. Dynamic Module Loading 🔴 CRITICAL
**Status**: Windows incompatible  
**Current**: `src/runtime/native_module.rs` uses **only** `dlopen/dlsym/dlclose`  
**Impact**: Cannot load native modules on Windows  
**Effort**: 4-6 hours  
**Solution**: Migrate to `libloading` crate (cross-platform wrapper)

```rust
// BEFORE (Unix only):
use libc::{dlopen, dlsym, dlclose, dlerror, RTLD_NOW, RTLD_LOCAL};
let handle = unsafe { dlopen(path.as_ptr(), RTLD_NOW | RTLD_LOCAL) };

// AFTER (Cross-platform):
use libloading::Library;
let lib = unsafe { Library::new(path) }?;
```

**Recommendation**: 🟢 **IMPLEMENT** — Essential for Windows support.

---

### 2. Terminal & Line Editor 🔴 HIGH PRIORITY
**Status**: Windows incompatible  
**Current**: `src/stdlib/term.rs` and `src/readline.rs` use **only** `termios` (Unix)  
**Impact**: No terminal features on Windows (raw mode, history, arrow keys)  
**Effort**: 12-16 hours  
**Solution**: Add Windows Console API implementations via `windows-sys`

**Files affected**:
- `src/stdlib/term.rs` (100 lines of Windows code)
- `src/readline.rs` (80 lines of Windows code)

**What needs porting**:
| Feature | Unix | Windows |
|---------|------|---------|
| Raw mode | `tcgetattr` | `SetConsoleMode` |
| Key input | `read(fd)` | `ReadConsoleInput` |
| Alt screen | ANSI codes | Console buffer |

**Recommendation**: 🟢 **IMPLEMENT** — High value, moderate complexity.

---

### 3. Graphics Backend 🟡 MEDIUM PRIORITY
**Status**: Partially implemented (X11 full, Win32 stub)  
**Current**: `src/stdlib/graphics/backend/win32.rs` is a **TODO stub**  
**Impact**: No native graphics on Windows (can fall back to X11 on WSL)  
**Effort**: 8-12 hours (full impl) or 0 hours (keep stub)  
**Solution**: Implement Win32 backend using `windows-sys`

**Current code**:
```rust
pub fn blit(&mut self, _canvas: &Canvas) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        // TODO: Use winapi or windows-rs...
        eprintln!("Win32Window: Stub mode");
    }
    Ok(())
}
```

**Recommendation**: 🟡 **DEFER FOR MVP** — Keep stub, defer full implementation to Phase 5. Graphics testing can use X11 on WSL2.

---

### 4. Module Loader 🟢 MEDIUM PRIORITY
**Status**: Mostly portable  
**Current**: `src/mod_loader/` uses portable `PathBuf` (good!)  
**Impact**: Minor — needs `.so` → `.dll` extension handling  
**Effort**: 1-2 hours  
**Solution**: Add conditional extension constant

```rust
#[cfg(unix)]
const NATIVE_EXT: &str = "so";
#[cfg(windows)]
const NATIVE_EXT: &str = "dll";
```

**Recommendation**: 🟢 **IMPLEMENT** — Quick win, part of Phase 4.

---

### 5. Threading 🟢 LOW PRIORITY
**Status**: Likely portable  
**Current**: `src/threading/threads.rs` has `#[cfg(unix)]` guards but mostly uses `std::thread`  
**Impact**: Should work on Windows (Rust stdlib handles it)  
**Effort**: 1-2 hours (review + test)  
**Solution**: Verify no `pthread_*` direct calls, test on Windows

**Recommendation**: 🟢 **VERIFY** — Low risk, include in Phase 4 review.

---

### 6. Path Handling 🟢 LOW PRIORITY
**Status**: Likely portable  
**Current**: `src/interpreter/shell_os/cli/cli.rs` uses `PathBuf` (good!)  
**Impact**: Should work (`PathBuf` handles `/` vs `\` automatically)  
**Effort**: 0.5 hours (quick test)  
**Solution**: Verify path joins work on Windows during Phase 4

**Recommendation**: 🟢 **VERIFY** — Low risk, test during Phase 4.

---

## Implementation Phases

### Phase 1: Dynamic Loading (CRITICAL) — 4-6 hours
**Must-have** for Windows MVP.  
Add `libloading` crate, refactor `native_module.rs`.  
Single file change, big impact.

### Phase 2: Terminal I/O (HIGH) — 12-16 hours
**Must-have** for usable Windows interpreter.  
Add Windows console support, mirror Unix termios API.  
Two files, moderate complexity.

### Phase 3: Graphics (MEDIUM) — 8-12 hours or DEFER
**Nice-to-have** for MVP (can use X11 on WSL).  
Full Win32 implementation is complex; stub sufficient for now.

### Phase 4: Edge Cases (MEDIUM/LOW) — 4-6 hours
Module loader, threading, paths verification.  
Quick wins, mostly review & testing.

### Phase 5: Integration (CRITICAL) — 4-6 hours
Build + smoke tests on Windows.  
Validate all phases work end-to-end.

---

## Risk Assessment

| Risk | Probability | Mitigation |
|------|-------------|------------|
| libloading incompatibility | Low | Well-established crate, used in production |
| Windows console API errors | Medium | Use `windows-sys` (official Microsoft crate) |
| Graphics complexity | High | Defer to Phase 3; use stub for MVP |
| Regression on Linux | Low | Run all tests on both platforms |
| Module compilation | Low | Out of scope; only loading portability |

**Overall Risk**: 🟢 **LOW** — Clear scope, proven libraries, minimal platform-specific complexity.

---

## Effort & Timeline

### Best Case (Graphics deferred)
- **Phases 1, 2, 4, 5**: 32 hours
- **Timeline**: 4-6 days (1 engineer, focused work)

### Full Implementation (Graphics included)
- **All 5 phases**: 52 hours
- **Timeline**: 6-8 days (1 engineer, focused work)
- **Or**: 2-3 weeks (part-time, 1-2 hours/day)

### Parallelizable Work
- Phase 1 ✓ can start independently
- Phase 2 ✓ can start during Phase 1 testing
- Phase 3 ✓ independent of 1 & 2 (if full impl needed)
- Phase 4 ✓ can run in parallel as long as Phase 1 builds
- Phase 5 ✓ final integration only

---

## Deliverables

Upon completion, PASTA will have:

1. ✅ **Cross-platform dynamic loading** (libloading)
2. ✅ **Windows terminal I/O** (Console API)
3. ✅ **Graphics backend routing** (X11 on Linux, Win32 on Windows)
4. ✅ **Portable module loader** (`.so` and `.dll` support)
5. ✅ **Cross-platform test suite** (passes on Windows + Linux)
6. ✅ **Windows binary** (`pasta.exe`) that runs full interpreter
7. ⚠️ **Graphics implementation** (optional; can defer to Phase 3)

---

## Recommended Next Steps

### Immediate (This Week)
1. ✅ **Approve audit** (this document)
2. **Start Phase 1**: Add `libloading`, refactor `native_module.rs`
3. **Parallel**: Begin Phase 2 Windows console API research

### Short-term (Next 1-2 Weeks)
4. **Complete Phase 2**: Implement Windows terminal support
5. **Complete Phase 4**: Verify threading, paths, module loader
6. **Phase 3 decision**: Full Win32 graphics or defer?

### Integration (End of Week 2-3)
7. **Phase 5**: Full build, test suite, smoke tests
8. **CI/CD**: Optional Windows build pipeline

### Post-MVP (Optional)
9. **Phase 3**: Implement Win32 graphics if deferred
10. **Performance tuning**: Windows-specific optimizations

---

## Files to Modify (Summary)

| Priority | File | Changes | Effort |
|----------|------|---------|--------|
| 🔴 P0 | `Cargo.toml` | Add deps, conditional features | 15 min |
| 🔴 P0 | `src/runtime/native_module.rs` | Replace dlopen with libloading | 2-3 hrs |
| 🔴 P0 | `src/stdlib/term.rs` | Add Windows console support | 3-4 hrs |
| 🔴 P0 | `src/readline.rs` | Add Windows console input | 3-4 hrs |
| 🟡 P1 | `src/mod_loader/*.rs` | Add EXT conditional | 1 hr |
| 🟡 P1 | `src/threading/` | Verify & test | 1 hr |
| 🟡 P2 | `src/stdlib/graphics/backend/win32.rs` | Full impl or stub | 8-12 hrs |

**Total changes**: ~400-700 lines of code

---

## Success Criteria

### Minimum Viable Product (MVP)
- [ ] `cargo build --release` succeeds on Windows
- [ ] All existing tests pass on Linux (no regression)
- [ ] All existing tests pass on Windows (or gracefully skip unsupported features)
- [ ] Interpreter runs on Windows (`pasta.exe` executes basic scripts)
- [ ] Terminal features work (raw mode, history, arrow keys)

### Full Implementation
- [ ] All MVP criteria
- [ ] Graphics backend works on Windows (native Win32 or X11 on WSL)
- [ ] Module loading works on Windows (`.dll` files)
- [ ] Full test suite passes on both platforms
- [ ] Windows CI/CD pipeline (optional)

---

## Questions & Clarifications

**Q: Do we need full Win32 graphics for MVP?**  
A: No. Stub is sufficient. Users can test graphics on Linux or WSL2 with X11. Full Win32 implementation is Phase 3 (optional).

**Q: Will this break Linux compatibility?**  
A: No. All changes use `#[cfg(unix)]` / `#[cfg(windows)]` guards. Linux behavior is unchanged.

**Q: What about macOS?**  
A: macOS uses Unix APIs (same as Linux for dynamic loading & terminal). Should work without changes. Not explicitly tested in this audit but should be compatible.

**Q: Can we parallelize implementation?**  
A: Yes. Phase 1 (libloading) is independent. Phase 2 (terminal) can start in parallel. Phase 4-5 can overlap.

**Q: What about compiled PASTA modules on Windows?**  
A: Out of scope for this audit. Module *loading* will work (via libloading). Module *compilation* (generating `.dll` files) requires separate build setup (LLVM, linker, etc.). Document as future work.

---

## Appendix: Platform API Comparison

### Dynamic Loading
| Operation | Unix | Windows |
|-----------|------|---------|
| Load library | `dlopen()` | `LoadLibrary()` |
| Get symbol | `dlsym()` | `GetProcAddress()` |
| Unload | `dlclose()` | `FreeLibrary()` |
| Error | `dlerror()` | `GetLastError()` |
| **Wrapper** | **libloading** | **libloading** |

### Terminal Control
| Feature | Unix | Windows |
|---------|------|---------|
| Raw mode | `tcgetattr()` | `SetConsoleMode()` |
| Disable | `tcsetattr()` | `SetConsoleMode()` |
| Read key | `read(fd)` | `ReadConsoleInput()` |
| Alt screen | ANSI ESC | Buffer / ANSI |
| Error model | errno | GetLastError() |

### Path Handling
| Operation | Unix | Windows | Rust |
|-----------|------|---------|------|
| Path join | `/a/b` | `C:\a\b` | `PathBuf::join()` ✓ |
| Absolute? | `/path` | `C:\path` | `Path::is_absolute()` ✓ |
| Separator | `/` | `\` | `std::path` ✓ |
| **Result** | Portable | Portable | Portable |

---

## Document References

- **Related**: `windows_implement.txt` (original implementation notes)
- **Detailed**: `WINDOWS_COMPATIBILITY_AUDIT.md` (full audit document)
- **Checklist**: `WINDOWS_IMPLEMENTATION_CHECKLIST.md` (step-by-step guide)

---

## Sign-Off

**Audit completed by**: GitHub Copilot CLI  
**Date**: July 15, 2026  
**Status**: ✅ Ready for implementation  
**Confidence**: 🟢 HIGH (clear scope, proven approach)

**Recommendation**: Proceed with Phase 1 & 2 implementation immediately. Phase 3 (graphics) can be deferred without blocking Windows MVP.

---

## Contact & Support

For questions during implementation, refer to:
- **Phase 1**: libloading docs, native_module.rs structure
- **Phase 2**: windows-sys docs, Console API reference
- **Phase 3**: Win32 API reference, existing x11.rs as example
- **All phases**: Platform cfg() macro documentation

