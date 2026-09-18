
# plan.md

---

## Session Plan (auto-updated)

Current todos (from session SQLite):

- translate-executor: in_progress — Translate src/interpreter/executor.rs to src/interpreter/executor.ps line-for-line (current focus).
- translate-large-files: pending — After executor complete, translate other large modules.
- verify-equivalence: pending — Build harness and run differential tests once runtime helpers implemented.
- iterate-small-files: pending — Translate remaining small Rust files.
- ci-integration: pending — Add CI to build & test Pasta self-hosted compiler.

Next actions:
1. Continue translating the next ~200-line slice of src/interpreter/executor.rs into src/interpreter/executor.ps (no per-slice confirmations unless a blocker appears).
2. Keep translate-executor todo in 'in_progress' until the entire file is translated; update SQL todo status accordingly.
3. After executor translation, run the established test harness (smoke/golden) once required host helpers are implemented.

Notes:
- Exclude Mage_AI directory from source reviews as requested.
- All future Rust (.rs) → Pasta (.ps) translations must be written in pure Pasta script. Avoid using host-language constructs (Python, shell, etc.) inside .ps files. When a feature or library cannot yet be implemented in Pasta, add an explicit TODO comment in the .ps file describing: the missing feature, the recommended Pasta API surface to implement, and a minimal, documented fallback that preserves runtime semantics. This policy ensures translations are self-hostable and avoids later sanitization sweeps.
- Missing host bindings (ex_frame*, env_get/set, NativeModule_*, fs_*, time_*, argv_get, exit_process, value_to_string) are tracked separately and will be stubbed during translation.
- Enforcement: Add a CI lint step and pre-commit hook to scan .ps files for disallowed patterns (e.g. "__import__", "print(", "input(", "try:", "except:", references to Python modules such as "re", "json", or other non-Pasta constructs). Failing builds should block merges until offending code is converted. A session todo 'enforce-pure-ps-policy' tracks adding the CI/lint check and rollout plan.

## Objective
Translate the existing Rust-based Pasta compiler and runtime into pure Pasta, verify behavioral equivalence, and bootstrap a fully self‑hosted Pasta compiler. Maintain deterministic, auditable, reversible workflows throughout.

---

## High-Level Stages

### Stage 0 — Rust Compiler as Specification
- Freeze feature development; only bug fixes allowed.
- Treat Rust implementation as the behavioral oracle.
- Establish stable serialization formats for AST, IR, and error outputs.
- Generate golden artifacts for all existing tests.

### Stage 1 — Pasta Implementation Compiled by Rust
- Translate compiler subsystems from Rust to Pasta in small vertical slices.
- Compile Pasta source using the Rust compiler to produce `pasta_p1`.
- Maintain a runtime flag to switch between Rust and Pasta implementations per subsystem.

### Stage 2 — Self-Compilation
- Use `pasta_p1` to compile the same Pasta compiler source into `pasta_p2`.
- Compare `pasta_p1` and `pasta_p2` for determinism or behavioral equivalence.
- Run full differential test suite across both compilers.

### Stage 3 — Fixed-Point Verification
- Perform triple-compilation: `pasta_p1 → pasta_p2 → pasta_p3`.
- Verify `pasta_p2` and `pasta_p3` are identical or behaviorally equivalent.
- Promote the Pasta compiler to primary once stable.

---

## Translation Strategy

### Vertical Slice Approach
Translate one subsystem at a time:
- Lexer
- Parser
- AST builder
- Type checker
- IR generator
- Optimizer (if present)
- Code generator
- Runtime library

For each slice:
- Keep Rust implementation active as fallback.
- Add a runtime switch: `PASTA_IMPL=rust|pasta`.
- Run equivalence tests before moving to the next slice.

### Interface Stability
- Preserve function signatures and data structure shapes.
- Keep enums, variants, and field names isomorphic to Rust.
- If a structural change is required, apply it to Rust first, stabilize, then port.

### Behavioral Specification
For each subsystem define:
- Accepted inputs
- Rejected inputs
- Error kinds and locations
- AST/IR shape invariants
- Runtime semantics
- Exit codes and stderr behavior

---

## Problem Areas and Required Safeguards

### Numeric Semantics
- Integer overflow rules
- Signed/unsigned comparisons
- Division and modulo with negative operands
- Bit shifts and masking

### Memory and Aliasing
- Avoid accidental shared mutable state
- Prevent dangling references
- Define clear lifetime rules for pointer-like structures
- Ensure deterministic initialization of global state

### Error Handling
- Map Rust panics to Pasta exceptions consistently
- Preserve error messages and locations
- Maintain identical exit codes and stderr formatting

### Recursion and Stack Behavior
- Ensure recursion depth matches Rust behavior
- Validate tail-call behavior if Pasta optimizes differently
- Detect stack overflows deterministically

### Concurrency (if applicable)
- Match Rust’s memory model where required
- Validate lock behavior and atomic operations
- Run stress tests to detect race conditions

### I/O and Environment
- Normalize paths consistently
- Match newline and encoding behavior
- Ensure deterministic stdout/stderr ordering

---

## Test Strategy

### Unit-Level Equivalence Tests
For each translated function:
- Call Rust version and Pasta version with identical inputs.
- Compare outputs and observable side effects.
- Use property-based tests for pure functions.

### Integration Tests
Subsystem-level tests:
- Parser: compare serialized ASTs.
- Type checker: compare error kinds and locations.
- IR generator: compare serialized IR.
- Codegen: compare emitted bytecode or machine code.

### Whole-Compiler Differential Tests
For each program `P`:
1. Compile with Rust compiler → `P_rust.bin`
2. Compile with Pasta compiler → `P_pasta.bin`
3. Run both binaries
4. Compare:
   - Exit code
   - Stdout
   - Stderr
   - Optional: performance envelope

### Golden Artifact Tests
- Maintain golden files for AST, IR, error outputs, and binary hashes.
- Regenerate only when intentionally updating semantics.

### Determinism Tests
- Recompile the same source multiple times.
- Validate identical outputs across runs.
- Validate identical outputs across machines if possible.

---

## Bootstrapping Verification

### Stage1 vs Stage2 Comparison
- Compare binary hashes if deterministic.
- If not deterministic, compare:
  - AST outputs
  - IR outputs
  - Emitted binaries for a large test suite

### Triple-Compilation Fixed Point
1. `pasta_p1` → `pasta_p2`
2. `pasta_p2` → `pasta_p3`
3. Compare `pasta_p2` and `pasta_p3`

A fixed point indicates stable self-hosting.

### Oracle Retention
- Keep Rust compiler available as a long-term oracle.
- Periodically re-run full differential tests.
- Use Rust implementation to bisect regressions.

---

## Required Tooling and Infrastructure

### Test Harness
- Script to run Rust and Pasta compilers on all test programs.
- Capture stdout, stderr, exit codes.
- Diff results and produce PASS/FAIL summary.

### Artifact Serialization
- Stable textual formats for:
  - AST
  - IR
  - Error reports
  - Runtime traces (optional)

### Logging and Diagnostics
- Deterministic logging for debugging mismatches.
- Ability to dump intermediate representations at each stage.

### Build Automation
- Reproducible builds with pinned dependencies.
- Clean environment for each test run.
- Automatic regeneration of golden files when approved.

---

## Workflow Loop

1. Run full test suite with Rust compiler.
2. Translate a small subsystem to Pasta.
3. Run unit equivalence tests for that subsystem.
4. Run integration tests for affected components.
5. Run full differential compiler tests.
6. Approve and commit only when all tests pass.
7. Move to next subsystem.

---

## Completion Criteria

- Pasta compiler can compile itself into a stable fixed point.
- All differential tests pass across:
  - Rust compiler
  - Stage1 Pasta compiler
  - Stage2/Stage3 Pasta compilers
- All golden artifacts match expected outputs.
- No behavioral regressions across the entire test suite.
- Deterministic builds across multiple runs.

---

## Imported: updated_plan.txt

(The following plan content was imported from updated_plan.txt and appended to the master plan to ensure post-completion steps and release phases are tracked.)

## Phase 1 — Prepare for v1.7 (Initial Self-Hosting)

### 1. Freeze Rust as the Oracle
- Stop adding new language features to Pasta until v1.7 is self-hosting.
- Only allow bug fixes that can be mirrored in both Rust and Pasta.
- Document that the Rust compiler is the canonical implementation for v1.7.

### 2. Stabilize Intermediate Representations
- Define a stable textual format for AST, IR, and Error reports.
- Implement serialization in the Rust compiler for AST, IR, Errors.
- Add tests that generate artifacts for existing programs and store as golden files.

### 3. Build the Differential Test Harness
- Create a tests/programs/ directory with core, edge-case, and error-case programs.
- Write a harness script to compile/run Rust and Pasta outputs, capture exit codes/stdout/stderr, and diff results.
- Extend the harness to dump and compare AST/IR/error outputs.

### 4. Translate Compiler Subsystems Rust → Pasta (for v1.7)
- Vertical-slice translation with runtime switch PASTA_IMPL=rust|pasta per subsystem.
- Unit-level equivalence tests and integration tests before moving on.

### 5. Produce the First Self-Hosted Compiler (v1.7)
- Use Rust to produce pasta_p1; use pasta_p1→pasta_p2→pasta_p3 and compare for fixed-point.

---

## Phase 2 — v1.7.1–v1.9.9 (Feature Expansion Under Dual Compilers)

- Enforce feature addition rule: implement in Rust first, port to Pasta, and test thoroughly.
- Rebuild Pasta compiler with updated Rust to produce new pasta_p1 and validate.

---

## Phase 3 — v2.0.0 (Fully Self-Hosted Fixed Point)

- Define v2.0.0 readiness criteria: all core subsystems in Pasta, full test suite passing, fixed-point bootstrap stable, deterministic builds.
- Final bootstrap cycle: pasta_p1→pasta_p2→pasta_p3 and verify fixed point, archive artifacts and hashes.

---

## Phase 4 — Post-2.0.0 (Future Features in Pure Pasta)

- After v2.0.0, implement new features directly in Pasta; extend tests and rebuild with current Pasta compiler.
- Optionally retain Rust as a long-term oracle for periodic sanity checks and archival.


