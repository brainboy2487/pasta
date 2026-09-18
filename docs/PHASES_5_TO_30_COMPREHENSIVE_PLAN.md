# PASTA Phases 5-30: Comprehensive Implementation Guide

**Status baseline:** Phases 1-4 complete.  
**Purpose:** Single actionable roadmap for all remaining phases.  
**Behavior policy:** Use `README.md` as semantic source of truth. If README is unclear/conflicting, stop and confirm with user before implementation.

---

## Execution Rules (applies to every phase)

1. Define behavior in docs first when semantics are changing.
2. Implement minimally first, then harden with tests.
3. Add targeted tests for each deliverable before broad full-suite validation.
4. Use release gates: **Design complete -> Implementation complete -> Tests passing -> Docs updated**.

---

## Master Timeline (high level)

| Phase Range | Theme | Primary Outcome |
|---|---|---|
| 5-6 | Stabilization + delivery | Production-ready Windows baseline and shipping workflow |
| 7-10 | Language maturity core | Tooling, types, performance, ecosystem foundation |
| 11-16 | Language architecture | Modules, errors, memory, traits, generics, collections |
| 17-21 | Runtime capabilities | IO, networking, package manager, concurrency, FFI |
| 22-25 | Developer platform | Formatter/linter, LSP, test runner, docs generator |
| 26-30 | Expansion + launch | WASM, macros, debugger, Pasta Book, ecosystem launch |

---

## Phase-by-Phase Action Plan

## Phase 5 — Stabilization + Sort Builtin + Integration Validation
**Goal:** Close immediate post-Phase-4 quality gaps and ship high-value builtin improvements.

**Scope**
- Implement `sort(...)` builtin plan from `sort_function_guide.txt`.
- Keep `list_sort(...)` backward compatible.
- Run integration/smoke set on Windows and Linux.

**Action items**
1. Add `sort(list|dict[, mode])` in `src/interpreter/executor.rs`.
2. Add fast/stable/slow mode handling and clear error contracts.
3. Update REPL/help text for `sort`.
4. Add targeted builtin tests + regression tests for `list_sort`.
5. Run `cargo test --workspace --all --release`.

**Exit criteria**
- `sort(...)` available and documented.
- All sort tests and full suite pass on Windows.

---

## Phase 6 — Distribution, CI/CD, and Release Automation
**Goal:** Make builds repeatable and distributable across platforms.

**Scope**
- Multi-platform CI jobs.
- Packaging/distribution artifacts.
- Release checklist automation.

**Action items**
1. Add/finish CI for Windows + Linux (+ macOS if available).
2. Produce release artifacts (`pasta.exe`, Linux binary, checksums).
3. Document install/run validation workflow.
4. Add smoke tests for binary invocation and basic script run.

**Exit criteria**
- Green CI on target platforms.
- Reproducible release artifact pipeline.

---

## Phase 7 — Developer Experience & Tooling
**Goal:** Establish first-class editing/development experience.

**Scope (from existing Phase 7 plan)**
- LSP server
- Formatter
- Linter
- Debugger foundation

**Action items**
1. Implement LSP minimal set: diagnostics, hover, go-to-def.
2. Implement formatter with stable output contract.
3. Implement linter rule categories (correctness-first).
4. Add debugger basics (breakpoints + stack/locals readout).

**Exit criteria**
- Editor integration works end-to-end for core language workflows.

---

## Phase 8 — Type System & Safety
**Goal:** Add optional type safety without breaking existing scripts.

**Scope**
- Type annotations
- Advanced error typing
- Nullable controls
- Basic generics

**Action items**
1. Introduce parser + checker support for optional annotations.
2. Add static checks for common type mismatches.
3. Add nullable-aware operators/guards.
4. Add constrained generics for core structures/functions.

**Exit criteria**
- Typed programs compile/check correctly; untyped programs stay compatible.

---

## Phase 9 — Performance & Async
**Goal:** Improve throughput and concurrency model.

**Scope**
- Bytecode compiler
- Async/await integration
- Thread pool/executor hardening

**Action items**
1. Implement bytecode generation for core subset.
2. Add bytecode execution path and performance benchmark harness.
3. Add async syntax and scheduler integration.
4. Add concurrency primitives and runtime tests.

**Exit criteria**
- Measurable performance gains and stable async execution semantics.

---

## Phase 10 — Ecosystem & Distribution Layer
**Goal:** Enable package/build workflows and broader adoption.

**Scope**
- Package manager (`pasta pkg`)
- Registry protocol
- Stdlib expansion
- Build workflow (`pasta build`)

**Action items**
1. Define package manifest and lockfile.
2. Implement init/add/build/install flows.
3. Implement minimal registry client protocol.
4. Add package integration tests and docs.

**Exit criteria**
- Users can create, install, and build projects via first-party tooling.

---

## Phase 11 — Module System
**Goal:** Scalable code organization.

**Action items**
1. Finalize import/export syntax.
2. Implement relative/absolute/package resolution.
3. Add module graph construction and cycle detection.
4. Enforce namespace and visibility rules.

**Exit criteria**
- Deterministic module resolution + cycle diagnostics + visibility enforcement.

---

## Phase 12 — Error & Result Model
**Goal:** Stable failure semantics across runtime + stdlib.

**Action items**
1. Decide canonical model (exceptions, Result, or hybrid).
2. Define propagation syntax and panic behavior.
3. Add standard error hierarchy.
4. Add stack-unwinding/traceback behavior tests.

**Exit criteria**
- One documented model used consistently across language and stdlib.

---

## Phase 13 — Memory Model
**Goal:** Predictable ownership/lifetime/performance model.

**Action items**
1. Choose model (ARC/GC/manual/hybrid).
2. Define mutability and ownership semantics.
3. Integrate allocator/runtime hooks.
4. Add compiler/runtime checks and stress tests.

**Exit criteria**
- Documented memory semantics with validated runtime behavior.

---

## Phase 14 — Traits / Interfaces
**Goal:** Polymorphism beyond inheritance.

**Action items**
1. Trait/interface syntax and implementation rules.
2. Static vs dynamic dispatch policy.
3. Trait bounds for generic APIs.
4. Dispatch performance/correctness tests.

**Exit criteria**
- Trait-based abstractions usable in stdlib and user code.

---

## Phase 15 — Generics
**Goal:** Reusable type-safe abstractions.

**Action items**
1. Generic functions and generic types.
2. Constraint solving via trait bounds.
3. Monomorphization/type-erasure strategy.
4. Compiler diagnostics for generic misuse.

**Exit criteria**
- Generic APIs compile and type-check with predictable behavior.

---

## Phase 16 — Standard Collections
**Goal:** Core data-structure backbone.

**Action items**
1. Implement `Vec<T>`, `Map<K,V>`, `Set<T>`.
2. Add iterator protocol and slices.
3. Add complexity/performance baseline tests.
4. Integrate with generics + traits.

**Exit criteria**
- Production-usable collections and iterator ecosystem.

---

## Phase 17 — Filesystem & IO
**Goal:** Reliable real-world app I/O.

**Action items**
1. File read/write/append APIs.
2. Directory traversal and path utilities.
3. Stream abstractions.
4. Cross-platform parity tests (Windows/Linux/macOS).

**Exit criteria**
- Stable, tested IO API with unified error behavior.

---

## Phase 18 — Networking
**Goal:** Client/server capability.

**Action items**
1. TCP and UDP primitives.
2. Minimal HTTP client.
3. Integrate with async model.
4. Add deterministic integration tests.

**Exit criteria**
- Networking APIs usable for real services/tools.

---

## Phase 19 — Package Manager (Ecosystem Core)
**Goal:** Ecosystem growth multiplier.

**Action items**
1. `pasta init`, `pasta add`, `pasta build`.
2. Lockfile + dependency resolution.
3. Registry protocol and auth story.
4. Package publish/install workflow.

**Exit criteria**
- End-to-end package lifecycle works in CI and locally.

---

## Phase 20 — Concurrency Model
**Goal:** Modern parallel/async execution model.

**Action items**
1. Finalize model (async/await, threads, actors, etc.).
2. Scheduler design and runtime integration.
3. Futures/promises and synchronization primitives.
4. IO/network interoperability tests.

**Exit criteria**
- Clear model, no deadlock-prone defaults, proven runtime behavior.

---

## Phase 21 — FFI to C
**Goal:** Access existing native ecosystems.

**Action items**
1. Define FFI syntax and ABI mapping.
2. Type conversion and memory ownership boundaries.
3. Safety model and unsafe boundaries.
4. Reference integrations (libc/SDL-like examples).

**Exit criteria**
- Stable C interop with documented safety/ownership contracts.

---

## Phase 22 — Tooling: Formatter + Linter
**Goal:** Consistent style and early issue detection.

**Action items**
1. `pasta fmt` command and style profile.
2. `pasta lint` warning/error categories.
3. Project-level config and ignore controls.
4. CI integration.

**Exit criteria**
- Formatter/linter become default project workflow.

---

## Phase 23 — Language Server (LSP)
**Goal:** Full IDE/editor support.

**Action items**
1. Completion and hover improvements.
2. Symbol indexing and cross-file definitions.
3. Diagnostics pipeline with quick fixes.
4. Performance tuning on medium/large projects.

**Exit criteria**
- Stable editor experience for day-to-day dev.

---

## Phase 24 — Testing Framework
**Goal:** First-party testing workflow.

**Action items**
1. `pasta test` command and test discovery.
2. Assertions, fixtures, and setup/teardown hooks.
3. Integration-test support.
4. Reporting output + CI summary format.

**Exit criteria**
- Reliable first-party test runner with CI-ready output.

---

## Phase 25 — Documentation Generator
**Goal:** Rustdoc-like API documentation flow.

**Action items**
1. Doc comment syntax conventions.
2. HTML doc generation + search index.
3. Package-aware docs output.
4. Publish pipeline integration.

**Exit criteria**
- Docs generated automatically for language + libraries.

---

## Phase 26 — WASM Backend
**Goal:** Browser/WASI runtime target.

**Action items**
1. WASM codegen path.
2. WASI support baseline.
3. JS interop adapter layer.
4. Browser + CLI wasm examples.

**Exit criteria**
- Runnable wasm output with documented constraints.

---

## Phase 27 — Macros / Compile-Time Features
**Goal:** Metaprogramming and DSL capabilities.

**Action items**
1. Macro syntax and expansion pipeline.
2. Hygiene and scope rules.
3. Compile-time evaluation hooks.
4. AST transform API boundaries.

**Exit criteria**
- Macros usable without destabilizing compile determinism.

---

## Phase 28 — Debugger
**Goal:** Professional runtime debugging tooling.

**Action items**
1. Breakpoints/step-in/step-out.
2. Variable and call-stack inspection.
3. Error trace integration.
4. IDE/LSP debugging integration.

**Exit criteria**
- Debugger supports core app-debug workflows.

---

## Phase 29 — The Pasta Book
**Goal:** Canonical onboarding + language reference.

**Action items**
1. Language reference chapters.
2. Tutorial tracks (beginner to advanced).
3. Cookbook and best practices.
4. Versioned examples tied to releases.

**Exit criteria**
- Complete first-party learning path for new users/contributors.

---

## Phase 30 — Ecosystem Launch
**Goal:** Public ecosystem readiness and growth.

**Action items**
1. Official starter templates and examples.
2. First-party package set.
3. Community channels/governance/process docs.
4. v1.0 launch readiness and adoption metrics.

**Exit criteria**
- Public launch package with clear contribution and growth model.

---

## Cross-Phase Dependency Map (critical)

1. **Phase 11 -> 19, 23, 25** (module system is prerequisite)
2. **Phase 12 + 13 -> 17, 18, 20, 21** (error + memory contracts underpin runtime features)
3. **Phase 14 + 15 -> 16** (traits/generics required for robust collections)
4. **Phase 17 + 18 + 20 -> 19** (package manager/network/async interop)
5. **Phase 22 + 23 + 24 + 25 -> 30** (tooling/test/docs readiness for launch)

---

## Immediate Start Queue (implementation-ready now)

1. **Phase 5:** implement `sort(...)` builtin and tests (`sort_function_guide.txt`).
2. **Phase 6:** finalize CI/release automation.
3. **Phase 7:** start LSP minimal feature slice.
4. **Phase 11 (design track in parallel):** finalize module semantics document before coding.

---

## Review Checklist (for next planning pass)

- Identify any phase with ambiguous semantics and resolve in README/docs before coding.
- Confirm each phase has measurable acceptance tests.
- Confirm dependencies are realistic for team capacity.
- Split each phase into issue-sized tasks (1-2 day units) before implementation sprint starts.
