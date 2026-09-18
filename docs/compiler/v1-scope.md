# Pasta Compiler v1 Scope

This bootstrap compiler slice is intentionally narrow so Pasta can gain a real native compilation path without blocking on the full dynamic runtime model.

## Target

- Primary target: `x86_64-linux-gnu`
- Backend pipeline: textual LLVM IR -> `llc` -> object file -> `clang` linker
- Host mode remains unchanged:
  - `pasta script.ps` interprets
  - `pasta script.ps --emit-llvm` emits LLVM IR
  - `pasta script.ps --compile` builds a native executable
  - `pasta module.pm --compile --shared` builds a native shared library for a top-level `MOD`

## Supported in the current bootstrap slice

- Top-level assignments to statically inferred `number`, `bool`, `string`, and opaque ABI `value` values
- Top-level `DEF` functions with:
  - direct named calls only
  - exact argument counts
  - explicit `RET.NOW(...)` returns when a function produces a value
  - implicit fallthrough to `None` when no explicit return is present
- Top-level `PRINT` of:
  - string literals / string variables
  - number literals / numeric variables / simple numeric expressions
  - boolean literals / boolean variables / simple boolean expressions
- Simple expressions:
  - numeric `+`, `-`, `*`, `/`, `%`
  - string `+` concatenation
  - numeric comparisons `==`, `!=`, `<`, `<=`, `>`, `>=`
  - string equality / inequality with `==` and `!=`
  - boolean `AND`, `OR`, `NOT`
  - `str(number)` string conversion
  - builtin `color(r, g, b)` with numeric arguments, lowering to the same packed ARGB number the interpreter returns
- Opaque ABI collection support for the current snake/demo slice:
  - numeric list literals like `[1, 2, 3]`
  - string-key / numeric-value dict literals like `{"x": 1, "y": 2}`
  - numeric list indexing like `snake_x[i]`
  - runtime-bridged `list_len`, `list_slice`, `list_concat`, `dict_get`, and `rand.int(min, max)`
- Runtime-bridged executable builtins for the current graphics slice:
  - `WINDOW(title, width, height)`
  - `WINDOW_POLL(window)`
  - `WINDOW_KEY(window)`
  - `WINDOW_CLOSE(window)`
  - `SET_DRAW_TARGET(window)`
  - `SET_COLOR(packed_color)`
  - `canvas_fill_rect(window, x1, y1, x2, y2)`
  - `SWAP_BUFFER(window)`
  - `fps_init(target)`
  - `fps_begin(target)`
  - `fps_end()`
  - `fps_tick()`
- Structured control flow in the static subset:
  - `IF ... OTHERWISE ... END` / `ELSE`
  - plain `WHILE <bool-expr>:` loops
  - `BREAK` and `CONTINUE` inside compiled `WHILE` loops
- Function-local assignments, prints, nested direct calls, control flow, and explicit returns within the same static subset
- Native executable emission for Linux
- Native shared-library emission for top-level `MOD` files that export compiled functions
- LLVM IR emission for inspection/debugging

## Shared-module slice currently supported

- Requires exactly one top-level `MOD name: ... END`
- Exported symbols must currently be compiled `DEF` functions
- Shared-module exports currently bridge `number`, `bool`, and string values through the runtime ABI
- Shared-module exports can also use an opaque ABI `value` signature so interpreter-managed lists, dicts, tensors, and `None` can pass through compiled locals and nested direct compiled calls
- Shared modules now advertise ABI version, call ABI label, refcounted handle ownership, and typed export signatures in the manifest
- Shared-library visibility is now wrapper-only: manifest/init/call entrypoints are exported, while compiled function bodies and module globals stay hidden
- The interpreter can load `lib<module>.so` from module search paths when source is absent

## Out of scope for this slice

- `BIND_SCOPE` / `UNBIND_SCOPE` block modifiers in compiled mode
- target-driven `WHILE` / `DO ... WHILE` loop forms in compiled mode
- `FOR` loops and other non-`WHILE` iteration forms
- non-function nested scopes
- Exported globals/constants from shared modules
- Direct compiled introspection or mutation of opaque ABI `value` data beyond pass-through/call/return flow
- General string operations beyond concatenation, `str(number)`, literal/variable storage, and printing
- First-class functions and loose-arity calls
- Optimizations beyond whatever `llc`/`clang` do by default

## Why this shape

The goal of this first compiler milestone is a proof-of-concept binary the user can run immediately, while keeping the implementation structured enough to expand toward:

1. a static subset compiler,
2. a runtime ABI for dynamic features,
3. compiled modules,
4. eventual self-hosting in Pasta itself.
