# PASTA — Program for Assignment, Statements, Threading, and Allocation

> **Version 1.4.1** · Scripting Language Interpreter written in Rust  
> Platform: Arch Linux · Build: `cargo build --release` · Root: `/home/travis/pasta`

[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#18-configuration--build)
[![Tests](https://img.shields.io/badge/tests-50%2F50-brightgreen)](#17-test-suite)
[![Language](https://img.shields.io/badge/language-Rust-orange)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-1.4.1-blue)](#19-changelog)
[![Graphics](https://img.shields.io/badge/graphics-X11%20native-purple)](#graphics-subsystem)

---

## Table of Contents

1. [Overview](#1-overview)
2. [Quick Start](#2-quick-start)
3. [Project Structure](#3-project-structure)
4. [Language Reference](#4-language-reference)
   - 4.1 [Literals](#41-literals)
   - 4.2 [Variables](#42-variables)
   - 4.3 [Operators](#43-operators)
   - 4.4 [Strings](#44-strings)
   - 4.5 [Lists](#45-lists)
   - 4.6 [Control Flow — IF / OTHERWISE](#46-control-flow--if--otherwise)
   - 4.7 [WHILE Loops](#47-while-loops)
   - 4.8 [FOR IN Loops](#48-for-in-loops)
   - 4.9 [Functions — DEF / DO](#49-functions--def--do)
   - 4.10 [Lambda Expressions](#410-lambda-expressions)
   - 4.11 [Return Semantics — RET.NOW / RET.LATE](#411-return-semantics--retnow--retlate)
   - 4.12 [Error Handling — ATTEMPT](#412-error-handling--attempt)
   - 4.13 [Priority Declarations](#413-priority-declarations)
   - 4.14 [Brace Blocks](#414-brace-blocks)
5. [Keywords Reference](#5-keywords-reference)
6. [Built-in Functions](#6-built-in-functions)
7. [Standard Library Modules](#7-standard-library-modules)
8. [Graphics Subsystem](#8-graphics-subsystem)
   - 8.1 [Core Graphics Builtins](#81-core-graphics-builtins)
   - 8.2 [pasta_G Standard Graphics Library](#82-pasta_g-standard-graphics-library)
   - 8.3 [X11 Native Window Backend](#83-x11-native-window-backend)
   - 8.4 [Graphics Examples](#84-graphics-examples)
9. [Type System](#9-type-system)
10. [Shell / OS Layer](#10-shell--os-layer)
11. [Async Runtime — pasta_async](#11-async-runtime--pasta_async)
12. [AI / ML Operations](#12-ai--ml-operations)
13. [Meatball Runtime Architecture (MRA)](#13-meatball-runtime-architecture-mra)
14. [REPL & CLI](#14-repl--cli)
15. [readline Module](#15-readline-module)
16. [Architecture Overview](#16-architecture-overview)
17. [Typing System Internals](#17-typing-system-internals)
18. [Error System](#18-error-system)
19. [Test Suite](#19-test-suite)
20. [Configuration & Build](#20-configuration--build)
21. [Changelog](#21-changelog)
22. [Roadmap / To-Do](#22-roadmap--to-do)

---

## 1. Overview

PASTA is a full-featured, embeddable scripting language interpreter written entirely in Rust. It is designed for expressiveness, safety, and extensibility — combining a clean high-level syntax with direct access to OS primitives, a virtual filesystem, async I/O, AI/ML tensor operations, **native X11 graphics**, and a novel Meatball Runtime Architecture (MRA) for agent-based execution.

**Core design goals:**

- Readable, colon-terminated block syntax with optional C-style brace blocks
- Rust-native performance with a safe ownership model under the hood
- First-class 2D graphics: WINDOW, CANVAS, PIXEL, BLIT, WINDOW_OPEN — pixel to screen natively
- First-class support for stdlib namespaces: `sys`, `fs`, `net`, `time`, `rand`, `gc`, `debug`, `ffi`, `thread`, `device`, `tensor`, `memory`
- Integrated shell/OS layer with a virtual filesystem (VFS) and `shell_os` subsystem
- Pluggable typing system with configurable numeric coercion, promotion, and rounding
- Production-ready REPL with raw-mode readline, 50-entry history ring, and full cursor navigation
- Scaffold for a Meatball Runtime Architecture (MRA) enabling agent-based and multi-backend workloads
- Structured error system with numeric error codes (E2xxx–E9xxx), ANSI color output, and inline hints
- Full 50-section regression suite passing cleanly as of v1.4.1

---

## 2. Quick Start

### Build (headless — no display required)

```bash
cd /home/travis/pasta
cargo build --release
```

### Build with native X11 window support

```bash
cargo build --release --features x11
sudo cp target/release/pasta /usr/bin/pasta
```

### Run a script

```bash
pasta tests/09_big_test.ps          # 30-section regression suite
pasta tests/10_full_suite.ps        # 50-section full suite
pasta tests/test_graphics.ps        # gradient render → out.ppm
DISPLAY=:0 pasta tests/test_shapes.ps  # live X11 window with shapes
```

### Interactive REPL

```bash
pasta
# PASTA interpreter — :help for commands, exit to quit
pasta> PRINT "hello world"
hello world
pasta> x = 10
pasta> PRINT x * 2
20
pasta> w = WINDOW("test", 200, 120)
pasta> exit
Goodbye.
```

### Graphics quick start

```pasta
w = WINDOW("hello", 320, 240)
c = CANVAS(320, 240)
# draw a red pixel
PIXEL(c, 100, 100, 255, 0, 0)
BLIT(w, c)
WINDOW_SAVE(w, "hello.ppm")
while WINDOW_OPEN(w):
  BLIT(w, c)
CLOSE(w)
```

---

## 3. Project Structure

```
pasta/
├── src/
│   ├── ai/                        # AI/ML subsystem
│   │   ├── autograd.rs            # Automatic differentiation
│   │   ├── datasets.rs            # Dataset loading helpers
│   │   ├── generate.rs            # Text/tensor generation
│   │   ├── learn.rs               # Training loops
│   │   ├── models.rs              # Model definitions
│   │   ├── tensor.rs              # Core tensor type
│   │   └── tokenizer.rs           # Tokenization utilities
│   ├── bin/
│   │   └── pasta.rs               # Binary entry point
│   ├── interpreter/
│   │   ├── shell_os/              # Shell / VFS integration
│   │   │   ├── cli/               # cli.rs, mod.rs
│   │   │   ├── commands/          # fs_commands.rs, mod.rs
│   │   │   └── vfs/               # fs.rs, node.rs, path.rs, mod.rs
│   │   ├── ai_network.rs          # AI network hooks for interpreter
│   │   ├── environment.rs         # Variable scopes & env stack
│   │   ├── errors.rs              # Runtime error types + error codes
│   │   ├── ex_eval.rs             # Expression/statement evaluator
│   │   ├── ex_frame.rs            # Stack frame management
│   │   ├── executor.rs            # Core interpreter dispatch loop
│   │   ├── int_api.rs             # Internal interpreter API
│   │   ├── mod.rs
│   │   ├── repl.rs                # Interactive REPL
│   │   └── shell.rs               # Shell integration shim
│   ├── lexer/
│   │   ├── alias.rs               # Keyword alias table (AliasTable)
│   │   ├── lexer.rs               # Tokenizer
│   │   ├── tokens.rs              # Token type definitions
│   │   ├── mod.rs
│   │   └── unicode.rs             # Unicode normalization helpers
│   ├── meatballs/                 # MRA scaffold
│   │   ├── agent/                 # agent.rs, Cargo.toml
│   │   ├── api/                   # meatball_api.rs, mod.rs
│   │   ├── backends/              # Backend stubs (local, pseudo-vm, vm)
│   │   ├── cli/                   # cli.rs
│   │   ├── phase0/                # mra_schema.json, objective.md.txt
│   │   └── runtime/               # runtime.rs
│   ├── parser/
│   │   ├── ast.rs                 # AST node definitions
│   │   ├── grammar.rs             # Grammar rules
│   │   ├── mod.rs
│   │   └── parser.rs              # Recursive descent parser
│   ├── pasta_async/               # Async runtime (sub-crate)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── api.rs, io.rs, lib.rs, runtime.rs
│   │       ├── serialize.rs, sync.rs, testing.rs
│   ├── runtime/
│   │   ├── asm.rs                 # AsmRuntime / AsmBlock
│   │   ├── bitwise.rs             # Bitwise operations
│   │   ├── devices.rs             # Device detection & profiles
│   │   ├── device_ls.json         # Known device list
│   │   ├── mod.rs                 # auto_configure, detect_host_arch
│   │   ├── rng.rs                 # RNG utilities
│   │   ├── scheduler.rs           # Task scheduler
│   │   ├── strainer.rs            # GC strainer
│   │   └── threading.rs           # Thread primitives
│   ├── semantics/
│   │   ├── constraints.rs         # Type & value constraints
│   │   ├── priority.rs            # PRIORITY semantic pass
│   │   └── resolver.rs            # Name & scope resolution
│   ├── stdlib/                    # Standard library
│   │   ├── graphics/              # ★ NEW: Native graphics subsystem
│   │   │   ├── backend/
│   │   │   │   ├── mod.rs         # BackendWindow trait
│   │   │   │   ├── x11.rs         # X11 native backend (XPutImage pipeline)
│   │   │   │   └── win32.rs       # Win32 stub (future)
│   │   │   ├── builtins.rs        # Builtin adapter helpers
│   │   │   ├── canvas.rs          # Canvas (ARGB pixel buffer)
│   │   │   ├── draw.rs            # Bresenham line/circle/rect
│   │   │   ├── mod.rs
│   │   │   └── window.rs          # Window struct
│   │   ├── debug.ph, device.ph, ffi.ph, fs.ph, gc.ph
│   │   ├── math.ph, memory.ph, net.ph
│   │   ├── pasta_G.ph             # ★ NEW: Standard graphics library (pure Pasta)
│   │   ├── rand.ph, stdio.ph, stdlib.pa
│   │   ├── sys.ph, tensor.ph, thread.ph, time.ph
│   │   └── mod.rs
│   ├── typing/                    # Type system
│   │   ├── bool.rs, bool_coerce.rs
│   │   ├── float.rs               # Float helpers, rounding
│   │   ├── int.rs, lib.rs, mod.rs
│   │   ├── operands.rs            # compute_numeric_op
│   │   ├── string.rs, string_coerce.rs
│   │   ├── tensor_type.rs
│   │   ├── types.rs               # Core Value enum
│   │   └── util.rs
│   ├── utils/
│   │   ├── errors.rs              # ★ EXPANDED: ErrorKind enum
│   │   ├── helpers.rs, logging.rs, mod.rs
│   ├── lib.rs                     # Crate root — public API re-exports
│   └── readline.rs                # Raw-mode line editor
├── tests/
│   ├── 09_big_test.ps             # 30-section regression suite
│   ├── 10_full_suite.ps           # 50-section full suite
│   ├── test_graphics.ps           # ★ NEW: Gradient render test
│   ├── test_shapes.ps             # ★ NEW: Triangle/circle/rect in X11 window
│   ├── mand_test.ps               # Mandelbrot stress test
│   └── ...
├── examples/
│   ├── graphics_test.pasta        # Graphics example
│   └── mandelbrot_gui.pasta       # Mandelbrot GUI target
├── tools/                         # Dev tooling scripts
├── artifacts/                     # Build logs and EATME diagnostic files
├── Cargo.toml
└── Cargo.lock
```

---

## 4. Language Reference

PASTA scripts use the `.ps` extension. Comments begin with `#`. Blocks may be opened with a colon (`:`) and closed with `END`, or delimited with C-style braces `{ }`.

### 4.1 Literals

| Type    | Example           | Notes                       |
|---------|-------------------|-----------------------------|
| Integer | `42`, `0`, `-1`   | 64-bit float internally     |
| Float   | `3.14`, `-0.5`    | f64 internally              |
| Bool    | `true`, `false`   | Lowercase required          |
| String  | `"hello pasta"`   | Double-quoted UTF-8         |
| List    | `[1, 2, 3]`       | Heterogeneous values ok     |
| None    | implicit / unset  | Unassigned variable         |

### 4.2 Variables

Variables are assigned with `=`. No declaration keyword is required. Rebinding is allowed. The `set` / `let` / `make` keyword prefix is optional.

```pasta
x = 10
set y = x + 5
PRINT x        # 10
PRINT y        # 15
x = x * 2
PRINT x        # 20
```

### 4.3 Operators

**Arithmetic**

| Operator | Description        | Example     | Result |
|----------|--------------------|-------------|--------|
| `+`      | Addition           | `2 + 3`     | `5`    |
| `-`      | Subtraction        | `10 - 4`    | `6`    |
| `*`      | Multiplication     | `3 * 7`     | `21`   |
| `/`      | Division (float)   | `15 / 4`    | `3.75` |
| `//`     | Floor division     | `15 // 4`   | `3`    |
| `%`      | Modulo             | `17 % 5`    | `2`    |
| `^`      | Exponentiation     | `2 ^ 8`     | `256`  |
| `**`     | Power (alternate)  | `2 ** 8`    | `256`  |
| `+=`     | Add-assign         | `x += 1`    | —      |
| `-=`     | Sub-assign         | `x -= 1`    | —      |
| `*=`     | Mul-assign         | `x *= 2`    | —      |
| `/=`     | Div-assign         | `x /= 2`    | —      |

**Comparison**

| Operator | Description      |
|----------|------------------|
| `==`     | Equal            |
| `!=`     | Not equal        |
| `>`      | Greater than     |
| `<`      | Less than        |
| `>=`     | Greater or equal |
| `<=`     | Less or equal    |

**Boolean**

| Operator | Description  |
|----------|--------------|
| `&&`     | Logical AND  |
| `\|\|`   | Logical OR   |
| `NOT`    | Logical NOT  |

**Bitwise (new in v1.4.1)**

| Operator | Description   |
|----------|---------------|
| `&`      | Bitwise AND   |
| `\|`     | Bitwise OR    |
| `~`      | Bitwise NOT   |
| `<<`     | Left shift    |
| `>>`     | Right shift   |

### 4.4 Strings

```pasta
s = "hello"
PRINT len(s)                       # 5
PRINT upper(s)                     # HELLO
PRINT lower("WORLD")               # world
PRINT concat(s, " world")          # hello world
PRINT s[0]                         # h
PRINT s[-1]                        # o  (negative indexing)
PRINT trim("  hi  ")               # hi
PRINT starts_with("pasta", "pas")  # true
PRINT ends_with("pasta", "sta")    # true
PRINT replace("hello", "l", "r")   # herro
parts = split("a,b,c", ",")
PRINT list_first(parts)            # a
PRINT list_last(parts)             # c
```

### 4.5 Lists

```pasta
nums = [10, 20, 30, 40, 50]
PRINT nums[0]                     # 10
PRINT list_len(nums)              # 5
PRINT list_sum(nums)              # 150
PRINT list_sort(nums)             # [10, 20, 30, 40, 50]
PRINT list_rev(nums)              # [50, 40, 30, 20, 10]
PRINT list_slice(nums, 1, 3)      # [20, 30]
PRINT list_concat([1,2], [3,4])   # [1, 2, 3, 4]
PRINT list_contains(nums, 30)     # true
PRINT range(0, 5)                 # [0, 1, 2, 3, 4]
PRINT range(0, 10, 2)             # [0, 2, 4, 6, 8]
PRINT range(10, 1, -3)            # [10, 7, 4, 1]
```

### 4.6 Control Flow — IF / OTHERWISE

```pasta
x = 10
IF x > 5:
    PRINT "big"
OTHERWISE:
    PRINT "small"
END

# Brace style (new in v1.4.1)
IF x > 5 {
    PRINT "big"
}
```

### 4.7 WHILE Loops

Both indent-block and brace-block styles are supported:

```pasta
# Colon / indent style
i = 0
WHILE i < 5:
    PRINT i
    i = i + 1
END

# Brace style (new in v1.4.1)
i = 0
while i < 5 {
    PRINT i
    i = i + 1
}
```

### 4.8 FOR IN Loops

```pasta
FOR x IN [10, 20, 30]:
    PRINT x
END

FOR i IN range(0, 5):
    PRINT i
END

FOR ch IN "hello":
    PRINT ch
END
```

### 4.9 Functions — DEF / DO

```pasta
DEF add a b:
    RET.NOW a + b
END
result = add(3, 4)
PRINT result    # 7

# Alternate call syntax
result = DO add 3 4
PRINT result    # 7
```

### 4.10 Lambda Expressions

```pasta
square = LAMBDA x: x * x END
PRINT square(6)         # 36

multiply = LAMBDA a b: a * b END
PRINT multiply(7, 6)    # 42

apply = LAMBDA f x: f(x) END
PRINT apply(square, 5)  # 25
```

### 4.11 Return Semantics — RET.NOW / RET.LATE

```pasta
DEF sign x:
    IF x < 0: RET.NOW "negative" END
    IF x == 0: RET.NOW "zero" END
    RET.NOW "positive"
END

# RET.LATE — body continues executing after return value is set
DEF deferred_example:
    RET.LATE 42
    PRINT "body continued after RET.LATE"
END
result = DO deferred_example   # prints message, returns 42
```

### 4.12 Error Handling — ATTEMPT

```pasta
ATTEMPT:
    x = 1 / 0
OTHERWISE:
    PRINT "caught an error"
END

result = ATTEMPT: 2 OTHERWISE: 0 END        # 2
result = ATTEMPT: 1/0 OTHERWISE: 42 END     # 42
```

### 4.13 Priority Declarations

```pasta
PRIORITY high:
    DO critical_task
END

PRIORITY low:
    DO background_task
END
```

### 4.14 Brace Blocks

New in v1.4.1 — C/Rust-style brace-delimited blocks work alongside the traditional colon/indent style. Both are fully supported and can be mixed:

```pasta
# Traditional style still works
WHILE x < 10:
    x = x + 1
END

# Brace style — same semantics
while x < 10 {
    x = x + 1
}

# Single-line brace
while x < 10 { x = x + 1 }
```

Brace blocks work for `while`, `if`, `for`, and function bodies. Semicolons are consumed as optional statement terminators, making C-style code natural:

```pasta
x = 0;
while x < 5 {
    PRINT(x);
    x = x + 1;
}
```

---

## 5. Keywords Reference

### Control & Block Keywords

| Keyword      | Category   | Description                                              |
|--------------|------------|----------------------------------------------------------|
| `PRINT`      | I/O        | Print a value or expression to stdout                    |
| `IF`         | Control    | Conditional branch                                       |
| `OTHERWISE`  | Control    | Else clause of `IF` or `ATTEMPT`                         |
| `UNLESS`     | Control    | Inverted IF (new v1.4.1)                                 |
| `WHILE`      | Loop       | Condition-checked loop                                   |
| `UNTIL`      | Loop       | Loop until condition (new v1.4.1)                        |
| `FOR`        | Loop       | Iteration loop                                           |
| `IN`         | Loop       | Separates loop variable from iterable                    |
| `MATCH`      | Control    | Pattern matching keyword (new v1.4.1)                    |
| `WHEN`       | Control    | Match arm condition (new v1.4.1)                         |
| `END`        | Block      | Closes any open block                                    |
| `DEF`        | Function   | Define a named function                                  |
| `DO`         | Function   | Call a named function                                    |
| `LAMBDA`     | Function   | Anonymous function expression                            |
| `RET.NOW`    | Return     | Immediate return                                         |
| `RET.LATE`   | Return     | Deferred return                                          |
| `ATTEMPT`    | Error      | Opens a try/catch block                                  |
| `PRIORITY`   | Scheduler  | Attach priority metadata to a block                      |
| `WITH`       | Context    | Context/scope block (new v1.4.1)                         |
| `FROM`       | Import     | Import source (new v1.4.1)                               |
| `YIELD`      | Generator  | Emit a value (new v1.4.1)                                |
| `AWAIT`      | Async      | Async wait (new v1.4.1)                                  |
| `ASSERT`     | Debug      | Runtime assertion (new v1.4.1)                           |
| `PASS`       | Control    | Explicit no-op (new v1.4.1)                              |
| `CONST`      | Variable   | Constant binding (new v1.4.1)                            |
| `NOT`        | Boolean    | Logical negation                                         |

### New Operator Tokens (v1.4.1)

| Symbol   | Description             |
|----------|-------------------------|
| `{` `}`  | Brace block delimiters  |
| `;`      | Statement terminator    |
| `::`     | Namespace separator     |
| `\|>`    | Forward pipe operator   |
| `//`     | Floor division          |
| `**`     | Power (alternate `^`)   |
| `+=` `-=` `*=` `/=` `%=` | Compound assignment |
| `->`     | Return type arrow       |
| `=>`     | Fat arrow / match arm   |
| `&` `\|` `~` `<<` `>>` | Bitwise operators |
| `?`      | Optional / ternary      |

---

## 6. Built-in Functions

### Math

| Function                        | Description                        |
|---------------------------------|------------------------------------|
| `abs(x)`                        | Absolute value                     |
| `floor(x)`                      | Floor to integer                   |
| `ceil(x)`                       | Ceiling to integer                 |
| `round(x)`                      | Round to nearest integer           |
| `sqrt(x)`                       | Square root                        |
| `pow(x, y)`                     | Power (also `^` / `**`)            |
| `min(a, b)` / `max(a, b)`       | Min / max of two values            |
| `log(x)` / `log2(x)`           | Natural / base-2 logarithm         |
| `sin(x)` / `cos(x)` / `tan(x)` | Trigonometric functions            |
| `sign(x)`                       | Returns -1, 0, or 1                |

### String

| Function                    | Description                                 |
|-----------------------------|---------------------------------------------|
| `len(s)`                    | String or list length                       |
| `upper(s)` / `lower(s)`     | Case conversion                             |
| `concat(a, b)`              | Concatenate two strings                     |
| `trim(s)`                   | Strip leading/trailing whitespace           |
| `starts_with(s, prefix)`    | Boolean prefix check                        |
| `ends_with(s, suffix)`      | Boolean suffix check                        |
| `replace(s, old, new)`      | Replace all occurrences                     |
| `split(s, delim)`           | Split into list by delimiter                |
| `substr(s, start, end)`     | Extract substring                           |
| `contains(s, sub)`          | Boolean containment check                   |
| `to_string(x)`              | Convert any value to string                 |

### Type Conversion

| Function       | Description                                                   |
|----------------|---------------------------------------------------------------|
| `int(x)`       | Convert to integer (truncates float)                          |
| `float(x)`     | Convert to float                                              |
| `bool(x)`      | Convert to boolean                                            |
| `to_string(x)` | Convert to string                                             |
| `type_of(x)`   | Returns `"number"`, `"string"`, `"bool"`, `"list"`, `"heap"` |

### List

| Function                  | Description                              |
|---------------------------|------------------------------------------|
| `list_len(lst)`           | Length                                   |
| `list_sum(lst)`           | Sum of all numeric elements              |
| `list_min(lst)` / `list_max(lst)` | Min / max element               |
| `list_avg(lst)`           | Average                                  |
| `list_sort(lst)`          | Sort ascending — returns new list        |
| `list_rev(lst)`           | Reverse — returns new list               |
| `list_slice(lst, s, e)`   | Slice from s to e (exclusive end)        |
| `list_push(lst, x)`       | Append element, return new list          |
| `list_pop(lst)`           | Remove and return last element           |
| `list_first(lst)` / `list_last(lst)` | First / last element          |
| `list_concat(a, b)`       | Concatenate two lists                    |
| `list_contains(lst, x)`   | Boolean element presence check           |
| `list_flatten(lst)`       | Flatten one level of nesting             |
| `range(s, e)`             | List `[s, s+1, ..., e-1]`               |
| `range(s, e, step)`       | List with step (positive or negative)    |

---

## 7. Standard Library Modules

All modules use dotted-namespace dispatch through `call_builtin`. The lexer absorbs dots into identifier tokens (`sys.env` → single `Identifier` token), enabling seamless dispatch.

### `sys.*`

| Function          | Description                           |
|-------------------|---------------------------------------|
| `sys.env(key)`    | Read environment variable             |
| `sys.exit(code)`  | Exit interpreter                      |
| `sys.platform()`  | Host platform string                  |
| `sys.pid()`       | Current process ID                    |

### `time.*`

| Function               | Description                           |
|------------------------|---------------------------------------|
| `time.now()`           | Current Unix timestamp (float secs)   |
| `time.sleep(secs)`     | Sleep for N seconds                   |
| `time.format(ts, fmt)` | Format a timestamp string             |
| `time.elapsed()`       | Elapsed time since interpreter start  |

### `rand.*`

| Function             | Description                           |
|----------------------|---------------------------------------|
| `rand.int(lo, hi)`   | Random integer in [lo, hi]            |
| `rand.float()`       | Random float in [0.0, 1.0)            |
| `rand.choice(lst)`   | Random element from list              |
| `rand.seed(n)`       | Seed the RNG                          |
| `rand.shuffle(lst)`  | Shuffle list in place                 |

### `fs.*`

| Function                 | Description                          |
|--------------------------|--------------------------------------|
| `fs.read(path)`          | Read file to string                  |
| `fs.write(path, data)`   | Write string to file                 |
| `fs.append(path, data)`  | Append string to file                |
| `fs.exists(path)`        | Boolean file/dir existence check     |
| `fs.delete(path)`        | Delete file                          |
| `fs.list_dir(path)`      | List directory contents              |
| `fs.mkdir(path)`         | Create directory                     |
| `fs.cwd()`               | Current working directory            |

### `tensor.*`

| Function                   | Description                          |
|----------------------------|--------------------------------------|
| `tensor.zeros(shape)`      | Zero-filled tensor                   |
| `tensor.ones(shape)`       | One-filled tensor                    |
| `tensor.rand(shape)`       | Random tensor                        |
| `tensor.add(a, b)`         | Element-wise addition                |
| `tensor.mul(a, b)`         | Element-wise multiplication          |
| `tensor.matmul(a, b)`      | Matrix multiplication                |
| `tensor.shape(t)`          | Shape as list                        |
| `tensor.reshape(t, shape)` | Reshape tensor                       |

---

## 8. Graphics Subsystem

PASTA v1.4.1 introduces a complete native 2D graphics pipeline — from PASTA script to X11 window to screen with no external graphics frameworks required.

### Pipeline

```
PASTA script
    ↓  PIXEL(canvas, x, y, r, g, b)
executor.rs  gfx_windows HashMap  (headless RGB buffer)
    ↓  BLIT(window, canvas)
Canvas::load_rgb() → Canvas struct (ARGB u32 pixels)
    ↓
X11Window::present()
    ↓
upload_canvas() → BGRA byte conversion
    ↓
XPutImage() → X server → compositor → screen
```

### 8.1 Core Graphics Builtins

These six builtins form the complete graphics primitive set. All are case-insensitive.

| Builtin                           | Returns        | Description                                       |
|-----------------------------------|----------------|---------------------------------------------------|
| `WINDOW(title, w, h)`             | window_handle  | Create a window (+ live X11 window if available)  |
| `CANVAS(w, h)`                    | canvas_handle  | Allocate an off-screen pixel buffer               |
| `PIXEL(canvas, x, y, r, g, b)`   | none           | Set one pixel — r,g,b in [0,255]                  |
| `BLIT(window, canvas)`            | none           | Copy canvas → window buffer + push to X11         |
| `WINDOW_OPEN(window)`             | bool           | True while window is alive; polls X11 events      |
| `WINDOW_SAVE(window, path)`       | none           | Save framebuffer as P6 PPM file                   |
| `CLOSE(window)`                   | none           | Destroy window and free resources                 |

Handles are opaque strings (`win://1`, `canvas://2`). They are always valid across calls within the same session.

### 8.2 pasta_G Standard Graphics Library

`src/stdlib/pasta_G.ph` is auto-loaded and provides a full high-level 2D drawing API implemented in pure Pasta on top of the six core builtins.

**Lifecycle wrappers**

| Function                         | Description                              |
|----------------------------------|------------------------------------------|
| `g_window(title, w, h)`          | Create window                            |
| `g_canvas(w, h)`                 | Create canvas                            |
| `g_show(win, canvas)`            | BLIT canvas to window                    |
| `g_save(win, path)`              | Save to PPM                              |
| `g_open(win)`                    | Returns true while open                  |
| `g_close(win)`                   | Close window                             |

**Drawing primitives**

| Function                                        | Description                  |
|-------------------------------------------------|------------------------------|
| `g_pixel(canvas, x, y, r, g, b)`               | Single pixel                 |
| `g_pixel_color(canvas, x, y, [r,g,b])`         | Pixel with color list        |
| `g_fill_rect(canvas, x, y, w, h, r, g, b)`     | Filled rectangle             |
| `g_clear(canvas, w, h, r, g, b)`               | Fill entire canvas           |
| `g_clear_black(canvas, w, h)`                  | Fill with black              |
| `g_line(canvas, x0, y0, x1, y1, r, g, b)`      | Bresenham line               |
| `g_rect(canvas, x, y, w, h, r, g, b)`          | Rectangle outline            |
| `g_circle(canvas, cx, cy, radius, r, g, b)`    | Midpoint circle              |
| `g_gradient_h(canvas, x,y,w,h, r0,g0,b0, r1,g1,b1)` | Horizontal gradient   |
| `g_gradient_v(canvas, x,y,w,h, r0,g0,b0, r1,g1,b1)` | Vertical gradient     |

**Named color constants**

`G_BLACK`, `G_WHITE`, `G_RED`, `G_GREEN`, `G_BLUE`, `G_YELLOW`, `G_CYAN`, `G_MAGENTA`, `G_GRAY`, `G_ORANGE`

Each is a `[r, g, b]` list usable with `_color` variants.

**Loop helpers**

| Function                             | Description                                             |
|--------------------------------------|---------------------------------------------------------|
| `g_loop(win, canvas, frame_fn)`      | Event loop — calls `frame_fn(canvas)` each tick         |
| `g_render_save(title,w,h,draw_fn,path)` | One-shot: create → draw → blit → save → close       |

### 8.3 X11 Native Window Backend

Location: `src/stdlib/graphics/backend/x11.rs`

The X11 backend is compiled when `--features x11` is passed. It is otherwise omitted — headless operation (pixel buffer + PPM save) always works without any display.

**Build requirements:**
```bash
sudo pacman -S libx11          # Arch Linux
# or
sudo apt install libx11-dev    # Debian/Ubuntu
```

**Pixel format pipeline:**
- Canvas stores pixels as `u32` `0xAARRGGBB`
- X11 `ZPixmap` 32bpp expects `BGRX` byte order on little-endian x86
- `upload_canvas()` performs the inline ARGB→BGRA conversion per pixel
- XImage is reused across blits (no malloc per frame) — buffer is pre-allocated at window creation

**Event handling:**
- `WM_DELETE_WINDOW` registered at creation — closing the window sets `open = false`
- `WINDOW_OPEN()` polls pending X events each call via `XPending` / `XNextEvent`
- `XInitThreads()` called once at startup via `std::sync::Once`

### 8.4 Graphics Examples

**Gradient fill + save to PPM (headless):**
```pasta
w = WINDOW("gradient", 200, 120)
c = CANVAS(200, 120)
y = 0
while y < 120:
    x = 0
    while x < 200:
        r = x % 256
        g = y % 256
        b = (x + y) % 256
        PIXEL(c, x, y, r, g, b)
        x = x + 1
    y = y + 1
BLIT(w, c)
WINDOW_SAVE(w, "gradient.ppm")
```

**Live X11 window with event loop:**
```pasta
w = WINDOW("live", 400, 300)
c = CANVAS(400, 300)
g_clear_black(c, 400, 300)
g_circle(c, 200, 150, 80, 50, 200, 255)
BLIT(w, c)
while WINDOW_OPEN(w):
    BLIT(w, c)
CLOSE(w)
```

**Using pasta_G helpers:**
```pasta
set draw = LAMBDA canvas:
    g_clear_black(canvas, 320, 240)
    g_fill_rect(canvas, 20, 20, 100, 80, 220, 50, 50)
    g_circle(canvas, 200, 120, 60, 50, 220, 50)
    g_line(canvas, 50, 220, 270, 220, 255, 255, 255)
END
g_render_save("shapes", 320, 240, draw, "shapes.ppm")
```

---

## 9. Type System

PASTA uses a unified `Value` enum. The typing module provides configurable numeric promotion, rounding, and coercion.

### Value Variants

| Variant    | Rust backing     | Description                      |
|------------|------------------|----------------------------------|
| `Number`   | `f64`            | All numeric values               |
| `String`   | `String`         | UTF-8 string                     |
| `Bool`     | `bool`           | Boolean true/false               |
| `List`     | `Vec<Value>`     | Heterogeneous list               |
| `Tensor`   | `RuntimeTensor`  | N-dimensional tensor             |
| `Lambda`   | `Vec<Statement>` | Callable block                   |
| `Heap`     | `GcRef`          | GC-managed heap reference        |
| `Pending`  | `(Value, u64)`   | RET.LATE deferred return value   |
| `None`     | —                | Absent / unassigned              |

### `type_of()` Return Values

| PASTA value | `type_of()` returns |
|-------------|---------------------|
| Number      | `"number"`          |
| String      | `"string"`          |
| Bool        | `"bool"`            |
| List        | `"heap"`            |
| Tensor      | `"tensor"`          |
| Lambda      | `"lambda"`          |
| None        | `"none"`            |

---

## 10. Shell / OS Layer

The `shell_os` subsystem integrates shell-like OS primitives into PASTA. It includes a virtual filesystem (VFS) with persistent backing via `DiskImages/fs.img`, filesystem shell commands, and a CLI integration layer.

---

## 11. Async Runtime — pasta_async

`src/pasta_async/` is a self-contained sub-crate providing async I/O and concurrency primitives. Modules: `api.rs`, `io.rs`, `runtime.rs`, `serialize.rs`, `sync.rs`, `testing.rs`.

---

## 12. AI / ML Operations

The `src/ai/` module provides a native AI/ML subsystem accessible via `tensor.*` stdlib:

```pasta
t = tensor.zeros([3, 3])
r = tensor.rand([2, 4])
PRINT tensor.shape(t)     # [3, 3]
result = tensor.matmul(t, r)
```

Modules: `tensor.rs`, `autograd.rs`, `models.rs`, `learn.rs`, `datasets.rs`, `generate.rs`, `tokenizer.rs`.

---

## 13. Meatball Runtime Architecture (MRA)

The MRA is PASTA's scaffold for agent-based and multi-backend execution (`src/meatballs/`). Agent binary communicates via JSON-over-stdio. Phase 0 schema defined in `phase0/mra_schema.json`. Backends: `local`, `pseudo-vm`, `vm` (stubs).

---

## 14. REPL & CLI

Launched with no arguments:

```
PASTA interpreter — :help for commands, exit to quit
pasta> _
```

| Command         | Description                          |
|-----------------|--------------------------------------|
| `:help`         | Show available commands              |
| `:history`      | Display command history              |
| `:clear`        | Clear the screen                     |
| `:env`          | Dump current environment bindings    |
| `exit` / `quit` | Exit the REPL                        |

**Script mode:** `pasta <script.ps>`  
**Eval mode:** `pasta -e "PRINT 1 + 1"` (planned)  
**Debug output:** `PASTA_DEBUG=3 pasta script.ps` (trace level)  
**Pretty errors:** `PASTA_PRETTY=1 pasta script.ps`

---

## 15. readline Module

`src/readline.rs` — production raw-mode line editor with:
- Full cursor navigation: Home, End, Left, Right
- 50-entry history ring (Up/Down)
- Kill/yank: Ctrl-K (kill to end), Ctrl-U (kill to start)
- Delete-at-cursor: ESC[3~ / Delete key
- Ctrl-C (interrupt) and Ctrl-D (EOF)
- Fallback to `stdin.read_line` on non-TTY

---

## 16. Architecture Overview

```
                    ┌─────────────────────────────────┐
                    │         PASTA Script (.ps)       │
                    └──────────────┬──────────────────┘
                                   │
                    ┌──────────────▼──────────────────┐
                    │     Lexer  (lexer.rs)            │
                    │  AliasTable · Unicode · Tokens   │
                    └──────────────┬──────────────────┘
                                   │
                    ┌──────────────▼──────────────────┐
                    │     Parser  (parser.rs)          │
                    │  AST · Grammar · Brace/Indent    │
                    └──────────────┬──────────────────┘
                                   │
                    ┌──────────────▼──────────────────┐
                    │  Semantics  (semantics/)         │
                    │  Constraints · Priority · Scope  │
                    └──────────────┬──────────────────┘
                                   │
          ┌────────────────────────▼────────────────────────┐
          │              Executor  (executor.rs)             │
          │   eval_stmt · eval_expr · call_builtin           │
          │   Environment · Functions · GC · Traceback       │
          │   gfx_windows (headless) · x11_windows (live)    │
          └────┬──────────┬──────────┬──────────┬───────────┘
               │          │          │          │
          ┌────▼───┐  ┌───▼───┐  ┌──▼──┐  ┌───▼──────────┐
          │stdlib/ │  │ ai/   │  │MRA  │  │  Graphics    │
          │.ph hdrs│  │tensor │  │meatb│  │  Pipeline    │
          └────────┘  └───────┘  └─────┘  │  canvas.rs   │
                                           │  x11.rs      │
                                           │  XPutImage   │
                                           │  → screen    │
                                           └──────────────┘
```

---

## 17. Typing System Internals

The typing module (`src/typing/`) provides:
- `compute_numeric_op` in `operands.rs` — centralizes all arithmetic dispatch
- `apply_round_and_downcast` — post-operation rounding per `CoercionConfig`
- `division_always_float` — forces float results from integer division
- Rounding levels 1–5 (no rounding → 2 → 4 → 6 → 10 decimal places)

---

## 18. Error System

PASTA v1.4.1 introduces a structured error system with numeric codes.

### Error Code Ranges

| Range   | Subsystem                        |
|---------|----------------------------------|
| E0xxx   | Lexer / tokenizer                |
| E1xxx   | Parser / syntax                  |
| E2xxx   | Runtime / evaluation (25 codes)  |
| E3xxx   | Type system (9 codes)            |
| E4xxx   | Graphics subsystem (12 codes)    |
| E5xxx   | I/O and filesystem (6 codes)     |
| E7xxx   | Concurrency / threading (4 codes)|
| E8xxx   | AI / tensor (4 codes)            |
| E9xxx   | Internal / assertions (3 codes)  |

### Selected Error Codes

| Code  | Meaning                                      |
|-------|----------------------------------------------|
| E2001 | Undefined variable                           |
| E2002 | Undefined function                           |
| E2003 | Arity mismatch (wrong number of arguments)   |
| E2004 | Division by zero                             |
| E2006 | Loop iteration limit exceeded                |
| E3001 | Type mismatch                                |
| E3004 | Index out of bounds                          |
| E4001 | Window creation failed                       |
| E4003 | Unknown graphics handle                      |
| E4005 | BLIT dimension mismatch                      |
| E4007 | X11 connection failed                        |

### Debug Environment Variables

| Variable       | Effect                                                    |
|----------------|-----------------------------------------------------------|
| `PASTA_DEBUG=0` | Silent — fatal errors only                              |
| `PASTA_DEBUG=1` | Normal — errors and warnings (default)                  |
| `PASTA_DEBUG=2` | Verbose — includes hints and notes                      |
| `PASTA_DEBUG=3` | Trace — statement-level execution trace                 |
| `PASTA_DEBUG=4` | Spam — token and expression detail                      |
| `PASTA_PRETTY=1`| Rust-compiler-style output with carets, hints, colors   |
| `NO_COLOR=1`    | Disable ANSI color output                               |

---

## 19. Test Suite

| Test File                   | Sections | Description                               |
|-----------------------------|----------|-------------------------------------------|
| `10_full_suite.ps`          | 50       | Complete regression suite — all passing   |
| `09_big_test.ps`            | 30       | Core language regression suite            |
| `test_graphics.ps`          | —        | Gradient render → `out.ppm`               |
| `test_shapes.ps`            | —        | Triangle/circle/rect in X11 window        |
| `mand_test.ps`              | —        | Mandelbrot stress test                    |
| `01_arithmetic_bindings.ps` | —        | Arithmetic and variable binding           |
| `06_functions_and_lambdas.ps` | —      | Functions, lambdas, closures              |

Run all:
```bash
pasta tests/10_full_suite.ps    # => === ALL 50 TESTS COMPLETE ===
pasta tests/09_big_test.ps      # => === ALL TESTS COMPLETE ===
pasta tests/test_graphics.ps    # => out.ppm written
DISPLAY=:0 pasta tests/test_shapes.ps  # => live X11 window
```

---

## 20. Configuration & Build

### Build profiles

```bash
# Headless (default — no display required)
cargo build --release

# With live X11 window support
cargo build --release --features x11

# Install system-wide
sudo cp target/release/pasta /usr/bin/pasta
```

### Cargo features

| Feature        | Effect                                           |
|----------------|--------------------------------------------------|
| `x11`          | Enable live X11 window backend (requires libx11) |
| `canvas_png`   | Enable PNG export via `image` crate              |
| `modloader_dev`| Enable file-watching for module hot-reload       |
| `scheduler`    | Enable task scheduler subsystem                  |
| `typing`       | Enable extended typing module                    |

### Global debug flags

| Global Symbol   | Default | Effect when `true`                       |
|-----------------|---------|------------------------------------------|
| `VERBOSE_FLAG`  | `false` | Enable general verbose runtime output    |
| `VERBOSE_DEBUG` | `false` | Enable detailed interpreter trace logs   |

Both are `AtomicBool` and can be set at startup or toggled at runtime via `PASTA_DEBUG` env var.

---

## 21. Changelog

### v1.4.1 — Current Release

#### Graphics Subsystem (★ Major New Feature)

- **Native X11 window pipeline.** `WINDOW()`, `CANVAS()`, `PIXEL()`, `BLIT()`, `WINDOW_OPEN()`, `WINDOW_SAVE()`, `CLOSE()` — complete pixel-to-screen pipeline working end-to-end on Linux X11.
- **X11 backend implemented.** `src/stdlib/graphics/backend/x11.rs` — `XOpenDisplay`, `XCreateSimpleWindow`, `XCreateGC`, `XPutImage` with pre-allocated BGRA pixel buffer, `WM_DELETE_WINDOW` event handling, XImage buffer reuse across blits.
- **Headless mode always available.** Without `--features x11`, all graphics builtins operate on an in-memory RGB buffer. `WINDOW_SAVE` writes P6 PPM files. No display required.
- **`pasta_G.ph` rewritten.** Old placeholder header using `CALL` stubs replaced with fully functional pure-Pasta graphics library: `g_line`, `g_fill_rect`, `g_circle`, `g_gradient_h/v`, `g_loop`, `g_render_save`, named color constants.
- **`stdlib` module wired.** `src/stdlib/mod.rs` created and `pub mod stdlib` added to `lib.rs`. Graphics module now properly in the Rust module tree.
- **`Canvas::load_rgb()` and `Canvas::fill()` added.** Enables loading raw RGB byte slices into the Canvas struct for X11 upload.
- **`x11_windows` field added to `Executor`.** Live X11 window handles stored alongside headless buffers, keyed by the same handle string.
- **`out.ppm` verified.** `tests/test_graphics.ps` produces correct 200×120 gradient (72000 bytes). First pixel `(0,0,0)`, last pixel `(199,119,231)` — correct.
- **`tests/test_shapes.ps` added.** Draws red filled rectangle, green circle outline, blue triangle outline, white border in a live 400×300 X11 window.

#### Language

- **Brace block syntax.** `while cond { ... }` and `if cond { ... }` work alongside traditional colon/indent blocks. Both styles fully supported and mixable.
- **Semicolon as statement terminator.** `;` is now consumed by the parser — C-style `x = 0;` no longer crashes with `Undefined variable ';'`.
- **New token types.** `LBrace`, `RBrace`, `Semicolon`, `ColonColon`, `Question`, `FloorDiv`, `PipeArrow`, `Backslash`, `PlusEq`, `MinusEq`, `StarEq`, `SlashEq`, `PercentEq`, `Arrow`, `FatArrow`, `StarStar`, `Tilde`, `Ampersand`, `Pipe`.
- **New keywords.** `UNLESS`, `UNTIL`, `PASS`, `ASSERT`, `TYPEOF`, `YIELD`, `RETURN`, `MATCH`, `WHEN`, `WITH`, `FROM`, `CONST`, `EXPORT`, `AWAIT`, `DRAW`, `COLOR`, `FRAME` — all tokenized and in the alias table.
- **Compound operators.** `+=`, `-=`, `*=`, `/=`, `%=` now lex correctly.
- **Floor division `//`.** Lexes as `FloorDiv` token, distinct from `/`.
- **Bitwise operators.** `&`, `|`, `~`, `<<`, `>>` now lex as proper tokens.
- **Case-insensitive graphics dispatch.** `WINDOW`, `window`, `Window` all hit the same builtin arm.
- **`CANVAS`, `PIXEL`, `BLIT`, `WINDOW_OPEN`, `CLOSE` builtins added** to executor (previously missing — only `window` and `window_set_pixel` existed).

#### Error System

- **`src/interpreter/errors.rs` expanded.** Full `RuntimeErrorKind` enum with 60+ variants covering E2xxx–E9xxx. Each kind has `.code()`, `.message()`, and `.hint()`.
- **`DebugLevel` enum.** `Silent/Normal/Verbose/Trace/Spam` — controlled via `PASTA_DEBUG` environment variable.
- **ANSI color helpers.** `red()`, `yellow()`, `cyan()`, `bold()`, `dimmed()`, `green()`, `magenta()` — color-aware, respects `NO_COLOR`.
- **Pretty diagnostic format.** `PASTA_PRETTY=1` enables Rust-compiler-style output: error code, message, file/line pointer, source caret, hint.
- **`Diagnostic` struct.** Warning/Note/Hint severity levels with span and code.
- **`utils/errors.rs` expanded.** New `ErrorKind` variants: `Graphics`, `Window`, `Canvas`, `X11`, `Lex`, `Scope`, `Loop`, `Thread`, `Ai`, `Tensor`, `Assertion`, `Unimplemented`.

#### Infrastructure

- **`x11 = { version = "2.3", features = ["xlib"], optional = true }` added to Cargo.toml.** Gated behind `--features x11`.
- **`pasta_G.ph` override fix.** Old `pasta_G.ph` was being auto-loaded and defining `blit` as a Pasta lambda using `CALL` (undefined), silently overriding the executor builtin. Rewritten to use real builtins.

---

### v1.4

- Full 50-section test suite passing
- `readline.rs` production raw-mode line editor
- `src/lib.rs` crate root established
- stdlib `.ph` headers promoted to real executable source
- Typing module finalized
- `pasta_async` sub-crate integrated
- `FOR IN` loop, `DO WHILE` form, `ATTEMPT`/`OTHERWISE`, `PRIORITY`
- `RET.LATE` deferred return
- Negative indexing (`s[-1]`)
- Parser `DEF` body `WHILE` bug fixed

### v1.3

- `FOR IN` loop skeleton
- `ATTEMPT`/`OTHERWISE` error handling
- `PRIORITY` keyword and semantic pass
- `RET.LATE` deferred return
- Lambda first-class values
- Shell_OS integration and VFS
- `pasta_async` sub-crate scaffold
- MRA skeleton
- AI/ML subsystem scaffold
- Typing system refactor

### v1.2

- `RET.NOW` early return
- Recursive function support
- `range()` builtin with optional step
- String negative indexing
- Additional list builtins

### v1.1

- `DEF` / `DO` function definition
- `IF` / `OTHERWISE`
- Basic arithmetic operators
- String and list builtins

### v1.0

- Initial PASTA interpreter in Rust
- Lexer, Parser, Executor scaffolded
- `PRINT`, literals, variables, arithmetic, `IF`/`OTHERWISE`/`END`

---

## 22. Roadmap / To-Do

### Immediate Priority (v1.5 targets)

- [ ] **`BREAK` and `CONTINUE` keywords.** Loop control flow — `BREAK` exits early, `CONTINUE` skips to next iteration.
- [ ] **String interpolation.** `"hello {name}"` or `f"..."` syntax to eliminate `concat()` chains.
- [ ] **Dictionaries / Maps.** `{key: value}` literal type with `dict.get`, `dict.set`, `dict.keys`, `dict.values`, `dict.contains`, `dict.remove`, `dict.len`.
- [ ] **`IMPORT` / module system.** `IMPORT "utils.ps"` or `IMPORT math` for explicit code splitting.
- [ ] **Mandelbrot in pure Pasta.** Write `iterate_point(x0, y0, max_iter)` and `mandelbrot(w, h, max_iter)` using CANVAS/PIXEL/BLIT. Target: `examples/mandelbrot_gui.pasta` running live in X11 at 200×150.
- [ ] **`FOR IN` with index.** `FOR i idx IN list` or `enumerate(lst)` builtin.
- [ ] **REPL history persistence.** Save/restore `~/.pasta_history` across sessions.

### Medium-Term (v1.6)

- [ ] **Mandelbrot at full resolution.** 800×600, `max_iter=1000`, with color palette mapping.
- [ ] **Mouse event handling in X11 backend.** Expose click/move events for zoom and pan.
- [ ] **True lexical closure capture.** Proper capture-at-definition semantics.
- [ ] **Typed exceptions in `ATTEMPT`.** `CATCH TypeError`, `CATCH IOError`, etc.
- [ ] **`MATCH` / pattern matching.** `MATCH value: CASE x: ... END`.
- [ ] **CLI flags.** `--verbose`, `--debug`, `--version`, `--eval <expr>`, `--check`.
- [ ] **Multi-line REPL.** Continuation detection for multi-line `DEF` and `IF` blocks.
- [ ] **`stdio.*` namespace.** `stdio.read_line()`, `stdio.read_all()`, `stdio.write(s)`.
- [ ] **SHM extension for X11.** Zero-copy blit via `XShmPutImage` for higher frame rates.
- [ ] **Wayland backend.** `wl_surface` + `wl_shm` for display-server-agnostic rendering.

### Long-Term / Architecture

- [ ] **MRA backends.** Implement `local` and `pseudo-vm` in `meatballs/backends/`.
- [ ] **Bytecode compiler.** AST → compact bytecode for faster repeated execution.
- [ ] **Garbage collector.** Replace drop-based reclamation with a proper tracing GC.
- [ ] **Native async — `ASYNC DEF` / `AWAIT`.** Surface `pasta_async` as first-class keywords.
- [ ] **AI model training pipeline.** Wire `learn.rs` to `tensor.*` stdlib.
- [ ] **LSP / language server.** Autocomplete, go-to-definition, hover docs, inline diagnostics.
- [ ] **Windows / macOS support.** Win32 backend stub exists — needs `CreateWindowEx` + `StretchDIBits`.
- [ ] **Tail-call optimization (TCO).** Unbounded recursion depth for tail-recursive patterns.
- [ ] **`device.*` auto-configure expansion.** GPU/NPU detection for tensor dispatch routing.
- [ ] **Expanded test suite.** Dedicated tests for graphics, async, VFS, MRA, AI/tensor.

---

*PASTA v1.4.1 — Built with ❤️ in Rust*  
*Project root: `/home/travis/pasta` · Platform: Arch Linux*  
*Native X11 graphics pipeline: `cargo build --release --features x11`*
