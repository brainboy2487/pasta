# Pasta Language Semantics

This document records the **current implemented semantics** that the compiler and eventual self-hosted toolchain must preserve unless intentionally changed.

## Functions and returns

- `DEF name(...): ... END` creates a function with its own `Function` scope.
- A defined function name is also bound as a first-class callable value in the current lexical scope.
- Nested function values capture enclosing **function-scope** bindings at definition time, but globals are still resolved dynamically rather than copied into the closure.
- Plain `DEF` functions do **not** mutate caller or global bindings just because a name matches. Assignment stops searching when it reaches the current function boundary.
- If a function body finishes without `RET.NOW`, the interpreter returns the **last statement value** it observed. If nothing produced a value, the function returns `None`.
- `RET.NOW(value)` evaluates `value`, dereferences it to a concrete runtime value, sets return control flow immediately, and exits the current function.
- In pipeline execution, `RET.NOW(value)` also forwards the concrete value downstream.
- `RET.LATE(WHEN fn)` and `RET.LATE(<ms>ms)` snapshot the value immediately and return a `Pending` value unless the delay is `0ms`.
- `RET.LATE(0ms)` is treated as `RET.NOW(...)` with a warning.
- The parser currently accepts bare `RET.NOW()` and normalizes it to the numeric literal `0.0`.

## Control flow

- `IF ... OTHERWISE ... END` executes the first body when any parsed condition is truthy; otherwise it executes the `OTHERWISE`/`ELSE` body when present.
- `OTHERWISE` and `ELSE` are accepted as equivalent syntax for the false branch.
- `WHILE` re-evaluates its condition before each iteration and enforces the executor loop limit.
- `BREAK` exits the nearest enclosing loop.
- `CONTINUE` skips to the next iteration of the nearest enclosing loop.
- `TRY ... OTHERWISE ... END` catches execution errors from the try body and runs the fallback body.
- `ATTEMPT(err): ... OTHERWISE: ... END` behaves like `TRY`, but binds the error string to `err` for the fallback body.
- `TRY` currently runs both its try body and fallback body in fresh block scopes.
- `ATTEMPT` runs its try body in a fresh block scope, but binds `err` into the surrounding scope before running the fallback body there.
- As with other block-scoped forms, ordinary new assignments inside `TRY` still land in the nearest function/global scope and remain visible afterward.

## Scope rules

Pasta uses three scope kinds:

- `Global`: root scope for the full program.
- `Function`: hard boundary for function and lambda calls.
- `Block`: soft boundary used by scoped control-flow forms.

Default assignment rules:

1. If a name already exists before the current function boundary, assignment updates that binding.
2. If a name is new, it is created in the nearest `Function` or `Global` scope.

This means:

- variables created inside default `IF` and default `WHILE` bodies remain visible after the block,
- plain function-local variables do not escape to callers,
- plain `DEF` functions do not mutate globals unless an explicit future global-mutation mechanism is used.

Scope modifiers:

- default `IF` / plain `WHILE`: no extra block scope is pushed,
- `UNBIND_SCOPE`: push a block scope,
- `BIND_SCOPE`: push a block scope and hoist that block on exit.

Important current behavior: ordinary assignments still go through scope-aware assignment, which creates new bindings in the nearest `Function` or `Global` scope. In practice, that means **plain assignments inside `IF(UNBIND_SCOPE)` and `WHILE(UNBIND_SCOPE)` still remain visible after the block today**. The modifiers mainly affect explicitly local bindings and block teardown, not ordinary assignment isolation.

`FOR IN` always uses a block scope for its loop variable binding; `BIND_SCOPE` hoists that block on exit, while default and `UNBIND_SCOPE` remove the loop variable binding itself. Ordinary assignments inside the loop body still follow the normal assignment rules above.

## Operator precedence

Higher numbers bind tighter in the parser:

| Precedence | Operators |
| --- | --- |
| 45 | `@` |
| 42 | `^` (right-associative) |
| 40 | `*`, `/`, `//`, `\`, `%` |
| 30 | `+`, `-` |
| 25 | `<<`, `>>` |
| 20 | `==`, `!=`, `<`, `>`, `<=`, `>=`, `~=` , `!==`, `===` |
| 15 | `&` |
| 10 | `AND` |
| 5 | `OR` |
| 4 | `|`, `||`, `|&|`, `|:|` |
| 3 | `|>` |

Current parser-specific behavior:

- top-level implicit juxtaposition such as `"Hello " name` is rewritten as `+`,
- that implicit rewrite only happens at the top expression level (`min_prec == 0`),
- unary `-expr` is parsed as `0 - expr`,
- unary `NOT expr` is parsed as a dedicated boolean-negation binary form.

## Current operator behavior

- `+` adds numbers.
- `+` concatenates strings.
- `+` concatenates lists.
- `+` will also coerce numeric-looking strings into numbers before falling back to string concatenation.
- Empty strings count as numeric `0` for numeric-string coercion.
- Equality uses direct value equality first, then numeric string/number coercion for `==` and `!=`.
- Ordered comparisons prefer numeric coercion when possible; otherwise they fall back to lexicographic string-like comparison.
- `^` is right-associative in the parser.

## Type and truthiness behavior

- This runtime is dynamically typed; the current compiler subset uses static inference only for the subset it can prove.
- Numeric-looking strings participate in arithmetic and numeric comparisons in many operators.
- For ordered comparisons, booleans coerce to `1`/`0`, `None` coerces to `0`, and lists coerce to their length when a numeric comparison path is taken.
- Truthiness currently works as follows:
  - `false`, `0`, `0.0`, `""`, `None`, empty lists, empty dicts, and zero-element tensors are falsey,
  - lambdas, builtins, pointers, pending values, heap references, family nodes, and lazy imports are truthy,
  - non-empty strings, lists, dicts, and non-zero numbers are truthy.

## Truthiness and iteration

- `IF` and `WHILE` use the executor truthiness rules when testing conditions.
- `FOR IN` currently iterates:
  - lists by element,
  - strings by character,
  - numbers as `0..n-1`.
- In `FOR IN`, the loop variable itself is block-scoped and disappears after the loop unless that block is hoisted with `BIND_SCOPE`.

## Error model

- Runtime execution errors use structured interpreter error kinds and codes.
- `TRY` and `ATTEMPT` catch ordinary execution errors, but the exception module still treats obviously fatal conditions like stack overflow and out-of-memory as non-recoverable.
- `ATTEMPT(err)` binds the caught error text as a string in the fallback branch's outer scope.
- The caught error payload is currently the runtime error's formatted text, not a structured error object.
- The bootstrap compiler is stricter than the interpreter in some areas. For example, compiled functions currently require explicit `RET.NOW(...)` on every reachable control-flow path.
