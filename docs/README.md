# PASTA — Programming And Scripting Tool for Automation

> **Version 1.6.1** · Scripting Language Interpreter written in Rust  
> Platform: Arch Linux · Build: `cargo build --release` · Root: `/home/travis/pasta`

[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#20-configuration--build)
[![Tests](https://img.shields.io/badge/tests-343%20passing-brightgreen)](#19-test-suite)
[![Language](https://img.shields.io/badge/language-Rust-orange)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-1.6.1-blue)](#21-changelog)
[![Graphics](https://img.shields.io/badge/graphics-X11%20native-purple)](#8-graphics-subsystem)

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
   - 4.12 [Error Handling — TRY / ATTEMPT](#412-error-handling--try--attempt)
   - 4.13 [Priority Declarations](#413-priority-declarations)
   - 4.14 [Brace Blocks](#414-brace-blocks)
   - 4.15 [Module Imports — FROM / USE / AS](#415-module-imports--from--use--as)
   - 4.16 [Pointer & Reference System](#416-pointer--reference-system)
5. [Keywords Reference](#5-keywords-reference)
6. [Built-in Functions](#6-built-in-functions)
7. [Standard Library Modules](#7-standard-library-modules)
8. [Graphics Subsystem](#8-graphics-subsystem)
   - 8.1 [Importing Graphics](#81-importing-graphics)
   - 8.2 [2D Drawing API](#82-2d-drawing-api)
   - 8.3 [Color Constants](#83-color-constants)
   - 8.4 [X11 Native Window Backend](#84-x11-native-window-backend)
   - 8.5 [Graphics Examples](#85-graphics-examples)
9. [Shell / OS Layer](#9-shell--os-layer)
    - 9.1 [Script Shell](#91-script-shell)
    - 9.2 [Interactive Shell](#92-interactive-shell)
10. [Pipeline System](#10-pipeline-system)
    - 10.1 [Script Pipelines](#101-script-pipelines)
    - 10.2 [Expression Pipe Operator |>](#102-expression-pipe-operator-)
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

**PASTA** (**P**rogramming **A**nd **S**cripting **T**ool for **A**utomation) is a domain-specific language and interpreter written in Rust. It combines Python-like readability with a Rust-backed execution model, offering:

- **Clean syntax** — Colon/indent or brace-delimited blocks, `DEF`/`DO` functions, `IF`/`OTHERWISE`, `WHILE`, `FOR IN`, `TRY` error handling
- **Script pipelines** — `a.ps | b.ps | c.ps` up to 8 stages; first stage sends values via `RETURN`; downstream stages receive per-value via `PIPE_IN`
- **`|>` pipe operator** — expression-level `value |> function` piping
- **Opt-in 2D graphics** — block `FROM graphics: use ... END`; full drawing API: lines, rects, circles, ellipses, triangles, polygons, arcs; 32 named color constants; PPM export and live X11 windows
- **Integrated shell** — VFS shell with quote parsing, IO redirect, env vars, glob expansion, command pipes, and text-processing builtins
- **First-class functions & lambdas** — `LAMBDA x: x * x END`, closures, higher-order patterns
- **Threading** — `DO: ... END` async blocks, global thread registry, `:threads` / `:thread-details` in REPL
- **Modular stdlib** — `.ph` headers for math, time, fs, rand, tensor, graphics; stdlib module registry
- **AI/ML subsystem** — `tensor.*` operations, autograd scaffold, model training pipeline
- **Async primitives** — `pasta_async` sub-crate with I/O, sync, and runtime modules
- **Structured errors** — Numeric error codes (E0xxx–E9xxx), `PASTA_PRETTY=1` Rust-style diagnostics
- **Module imports** — `FROM` / `USE` / `AS` lazy import system with `PASTA_MODULE_PATH` support
- **Unified pointer system** — `ALLOC`/`FREE`/`GOTO`/`PULL`/`PUSH` for memory, files, devices, and networks

---

## 2. Quick Start

```bash
# Clone and build
git clone https://github.com/yourname/pasta.git
cd pasta
cargo build --release

# Run test suite
./target/release/pasta tests/10_full_suite.ps   # 50 sections

# REPL mode
./target/release/pasta
pasta> PRINT 1 + 2
3
pasta> exit

# Script mode
./target/release/pasta examples/hello.ps
```

**Hello World:**

```pasta
# hello.ps
PRINT "Hello, PASTA!"
x = 10 + 32
PRINT x    # 42
```

**With graphics (X11 build):**

```bash
cargo build --release --features x11
./target/release/pasta tests/test_shapes.ps
```

---

## 3. Project Structure

```
pasta/
├── src/
│   ├── ai/                        # AI/ML subsystem
│   │   ├── autograd.rs            # Automatic differentiation
│   │   ├── datasets.rs            # Dataset loading
│   │   ├── generate.rs            # Generation utilities
│   │   ├── learn.rs               # Training loops
│   │   ├── models.rs              # Model definitions
│   │   ├── tensor.rs              # Tensor operations
│   │   ├── tokenizer.rs           # Tokenization
│   │   └── mod.rs
│   ├── bin/
│   │   └── pasta.rs               # Main binary entrypoint
│   ├── error_logging/             # Error system
│   │   ├── error_handler.rs
│   │   ├── error_messages.rs
│   │   ├── error_messages.json
│   │   └── mod.rs
│   ├── interpreter/               # Core interpreter
│   │   ├── ai_network.rs          # AI network integration
│   │   ├── environment.rs         # Variable environment
│   │   ├── errors.rs              # RuntimeErrorKind enum (60+ variants)
│   │   ├── ex_eval.rs             # Expression evaluator
│   │   ├── ex_frame.rs            # Stack frames
│   │   ├── executor.rs            # Statement executor
│   │   ├── int_api.rs             # Internal API
│   │   ├── int_api_tests.rs       # API tests
│   │   └── mod.rs
│   ├── kernel/                    # Low-level kernel operations
│   ├── lexer/                     # Tokenizer
│   │   ├── lexer.rs               # Main lexer implementation
│   │   ├── alias_table.rs         # Keyword aliases
│   │   ├── tokens.rs              # Token types
│   │   └── mod.rs
│   ├── meatballs/                 # MRA agent system
│   │   ├── backends/              # Execution backends
│   │   ├── phase0/                # Schema definitions
│   │   └── mod.rs
│   ├── mod_loader/                # Module loading system
│   │   ├── loader.rs              # Module loader
│   │   ├── resolver.rs            # Path resolution
│   │   └── mod.rs
│   ├── parser/                    # AST generation
│   │   ├── parser.rs              # Main parser
│   │   ├── ast.rs                 # AST node definitions
│   │   └── mod.rs
│   ├── pasta_async/               # Async runtime sub-crate
│   │   ├── api.rs                 # Async API
│   │   ├── io.rs                  # Async I/O
│   │   ├── runtime.rs             # Runtime loop
│   │   ├── serialize.rs           # Serialization
│   │   ├── sync.rs                # Synchronization primitives
│   │   ├── testing.rs             # Async tests
│   │   └── mod.rs
│   ├── pipelines/                 # Pipeline processing
│   ├── runtime/                   # Runtime support
│   ├── saucey/                    # Saucey GC and pointer system
│   │   ├── saucey.rs              # Main Saucey implementation
│   │   ├── strainer.rs            # GC (Strainer)
│   │   └── mod.rs
│   ├── semantics/                 # Semantic analysis
│   │   ├── constraints.rs         # Type constraints
│   │   ├── priority.rs            # Priority handling
│   │   ├── scope.rs               # Scope analysis
│   │   └── mod.rs
│   ├── stdlib/                    # Standard library
│   │   ├── graphics/              # ★ Native graphics subsystem
│   │   │   ├── backend/
│   │   │   │   ├── mod.rs         # BackendWindow trait
│   │   │   │   ├── x11.rs         # X11 native backend (XPutImage pipeline)
│   │   │   │   └── win32.rs       # Win32 stub (future)
│   │   │   ├── api.rs             # Importable graphics helpers
│   │   │   ├── canvas.rs          # Canvas (ARGB pixel buffer)
│   │   │   ├── draw.rs            # Bresenham line/circle/rect
│   │   │   ├── events.rs          # Event helpers
│   │   │   ├── mod.rs
│   │   │   ├── pixel_format.rs    # Pixel format helpers
│   │   │   └── window.rs          # Window struct
│   │   ├── debug.ph, device.ph, ffi.ph, fs.ph, gc.ph
│   │   ├── math.ph, memory.ph, net.ph
│   │   ├── pasta_G.ph             # ★ Standard graphics library (pure Pasta)
│   │   ├── rand.ph, stdio.ph, stdlib.pa
│   │   ├── sys.ph, tensor.ph, thread.ph, time.ph
│   │   └── mod.rs
│   ├── threading/                 # Threading support
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
│   ├── test_graphics.ps           # ★ Gradient render test
│   ├── test_shapes.ps             # ★ Triangle/circle/rect in X11 window
│   ├── mand_test.ps               # Mandelbrot stress test
│   └── ...
├── examples/
│   ├── graphics_test.pasta        # Graphics example
│   └── mandelbrot_gui.pasta       # Mandelbrot GUI target
├── tools/                         # Dev tooling scripts
├── docs/                          # Documentation
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
| Bool    | `true`, `false`   | Case-insensitive; `true`/`false` preferred |
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
| `AND`    | Logical AND  |
| `OR`     | Logical OR   |
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

**String Interpolation (v1.4.3)**

```pasta
name = "PASTA"
version = 1.4
PRINT "Welcome to {name} v{version}!"   # Welcome to PASTA v1.4!
PRINT "2 + 2 = {2 + 2}"                  # 2 + 2 = 4
PRINT "Escaped: {{ and }}"              # Escaped: { and }
```

### 4.5 Lists

```pasta
nums = [10, 20, 30, 40, 50]
PRINT nums[0]                     # 10  (bracket indexing: 0-based)
PRINT nums(1)                     # 10  (paren indexing: 1-based)
PRINT nums[1]                     # 20  (0-based: second element)
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

# BREAK and CONTINUE (v1.4.2)
i = 0
WHILE i < 10:
    i = i + 1
    IF i == 5: CONTINUE END
    IF i == 8: BREAK END
    PRINT i
END
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

# With BREAK/CONTINUE (v1.4.2)
FOR item IN [1, 2, 3, 4, 5]:
    IF item == 3: CONTINUE END
    IF item == 5: BREAK END
    PRINT item
END
```

### 4.9 Functions — DEF / DO

```pasta
DEF add(a, b):
    RET.NOW a + b
END
result = add(3, 4)
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
DEF sign(x):
    IF x < 0: RET.NOW "negative" END
    IF x == 0: RET.NOW "zero" END
    RET.NOW "positive"
END
```

`RET.LATE` is **experimental** — it sets a deferred return value and allows the function body to continue executing, but the caller receives a raw pending object rather than the resolved value. Use `RET.NOW` or `RETURN` for reliable returns.

### 4.12 Error Handling — TRY / ATTEMPT

`TRY` and `ATTEMPT` are interchangeable keywords for the same block-level construct:

```pasta
TRY:
    x = 1 / 0
OTHERWISE:
    PRINT "caught an error"
END

# ATTEMPT is an alias for TRY
ATTEMPT:
    x = risky_call()
OTHERWISE:
    PRINT "caught error"
END
```

To bind the error message, use `ATTEMPT error_var:`:

```pasta
ATTEMPT err:
    raise("something went wrong")
OTHERWISE:
    PRINT err   # "something went wrong"
END
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

### 4.15 Module Imports — FROM / USE / AS

New in v1.4.2 — a complete lazy module import system. Symbols are bound on first use, so module code runs only when a symbol from the module is first accessed.

#### Basic import

```pasta
FROM:
    mymodule
        use
            add
        END
END

set result = add(2, 3)   # mymodule is loaded here, on first call
```

#### Import with alias

Use `AS` to bind the imported symbol under a different name:

```pasta
FROM:
    mathutil
        use
            add as plus
        END
END

set y = plus(10, 7)      # y == 17
```

#### Multiple symbols from one module

```pasta
FROM:
    arith
        use
            add
            mul
        END
END

set s = add(3, 4)        # s == 7
set p = mul(3, 4)        # p == 12
```

#### Module search path

The runtime searches for `<name>.pm` files in this order:

1. `./<name>.pm` — current working directory
2. `./modules/<name>.pm` — local `modules/` subdirectory
3. `$PASTA_MODULE_PATH/<name>.pm` — each colon-separated directory in the `PASTA_MODULE_PATH` environment variable
4. `./src/stdlib/<name>.pm` — development tree
5. `./stdlib/<name>.pm` — legacy layout

Set `PASTA_MODULE_PATH` to point to installed stdlib or project-specific module directories:

```bash
export PASTA_MODULE_PATH=/usr/share/pasta/modules:/home/user/myproject/modules
```

#### Module file format (`.pm`)

A module file must declare itself with `MOD` and list its exported symbols with `export`:

```pasta
MOD mathutil:
    export add

    def add(a, b):
        return a + b
END
```

---

### 4.16 Pointer & Reference System

**New in v1.4.4** — A unified pointer abstraction for memory, files, devices, and network resources.

#### Pointer Types

PASTA supports four kinds of pointers:

| Kind | Description | Creation |
|------|-------------|----------|
| `MEM` | Raw memory buffer | `ALLOC.MEM(size)` |
| `FILE` | File handle | `ALLOC.FILE(path, mode)` or `REF.FILE(path)` |
| `DEV` | Device handle (GPIO, serial, etc.) | `ALLOC.DEV(id, type)` |
| `NET` | Network socket | `ALLOC.NET(host, port)` or `REF.NET(endpoint)` |

#### Allocating Pointers

Use `ALLOC.<KIND>` to create a new pointer:

```pasta
// Allocate 1KB memory buffer
ALLOC.MEM(1024) -> buffer

// Open a file for reading
ALLOC.FILE("/tmp/data.txt", "r") -> file_ptr

// Create network connection
ALLOC.NET("localhost", 8080) -> sock
```

#### Reading/Writing with GOTO, PULL, PUSH

The `GOTO` statement sets the active pointer context. Inside a `GOTO` block, `PULL` reads and `PUSH` writes:

```pasta
ALLOC.MEM(64) -> buf

GOTO buf:
    // Write bytes to buffer
    PUSH.BYTE 0x48    // 'H'
    PUSH.BYTE 0x69    // 'i'
    
    // Read bytes back (from current offset)
    PULL.BYTE -> b1
    PULL.BYTE -> b2
END
```

#### Data Types for PULL/PUSH

| Type | Description |
|------|-------------|
| `PULL.BYTE` / `PUSH.BYTE` | Single byte (0-255) |
| `PULL.INT` / `PUSH.INT` | 64-bit integer |
| `PULL.FLOAT` / `PUSH.FLOAT` | 64-bit float |
| `PULL.STR(len)` / `PUSH.STR` | String with length |
| `PULL.BYTES(len)` / `PUSH.BYTES` | Raw byte array |

#### Getting Pointer Info

Use `INFO` to inspect a pointer's metadata:

```pasta
ALLOC.MEM(128) -> buf
INFO buf -> metadata

// metadata is a list of [key, value] pairs:
// [[id, 1], [kind, MEM], [alive, true], [temporary, false], [size, 128], [offset, 0]]
```

#### REF Expressions

`REF.<KIND>(target)` creates a pointer with optional metadata:

```pasta
// Create file pointer with metadata
set fp = REF.FILE("/etc/passwd") WITH { mode: "r", encoding: "utf-8" }

// Create memory pointer from existing data
set mp = REF.MEM([1, 2, 3, 4])  // Initialize with byte list
```

#### Freeing Pointers

Use `FREE` to release a pointer. Accessing a freed pointer raises an error:

```pasta
ALLOC.MEM(64) -> buf
// ... use buf ...
FREE buf

// This would raise error[E070]: Use after free
// GOTO buf: ... END
```

#### GC Integration

Pointers allocated inside `GOTO` blocks are automatically marked as temporary and freed when the block exits:

```pasta
ALLOC.MEM(1024) -> main_buf

GOTO main_buf:
    // This pointer is freed when GOTO exits
    ALLOC.MEM(64) -> temp_buf
    // ...
END

// temp_buf is automatically freed here
```

#### Error Codes

| Code | Name | Description |
|------|------|-------------|
| E070 | `PTR_USE_AFTER_FREE` | Pointer accessed after being freed |
| E071 | `PTR_KIND_MISMATCH` | Operation not supported for pointer kind |
| E072 | `PTR_NO_CONTEXT` | PULL/PUSH without active GOTO context |
| E073 | `PTR_NOT_FOUND` | Pointer ID not in registry |
| E074 | `PTR_INVALID_TYPE` | Expected pointer value, got something else |
| E075 | `PTR_METADATA_ERROR` | Invalid metadata in REF expression |

---

## 5. Keywords Reference

### Control & Block Keywords

| Keyword      | Category   | Description                                              |
|--------------|------------|----------------------------------------------------------|
| `PRINT`      | I/O        | Print a value or expression to stdout                    |
| `IF`         | Control    | Conditional branch                                       |
| `OTHERWISE`  | Control    | Else clause of `IF` or `ATTEMPT`                         |
| `UNLESS`     | Control    | Inverted IF — simple conditions only (v1.4.6)            |
| `WHILE`      | Loop       | Condition-checked loop                                   |
| `UNTIL`      | Loop       | **Reserved — unreliable, do not use**                    |
| `FOR`        | Loop       | Iteration loop                                           |
| `IN`         | Loop       | Separates loop variable from iterable                    |
| `MATCH`      | Control    | **Reserved — not yet implemented**                       |
| `WHEN`       | Control    | **Reserved — not yet implemented**                       |
| `END`        | Block      | Closes any open block                                    |
| `DEF`        | Function   | Define a named function                                  |
| `DO`         | Function   | Call a named function                                    |
| `LAMBDA`     | Function   | Anonymous function expression                            |
| `RET.NOW`    | Return     | Immediate return                                         |
| `RET.LATE`   | Return     | Deferred return                                          |
| `ATTEMPT`    | Error      | Opens a try/catch block (alias for `TRY`)                |
| `TRY`        | Error      | Opens a try/catch block (preferred form)                 |
| `PRIORITY`   | Scheduler  | Attach priority metadata to a block                      |
| `WITH`       | Context    | **Reserved — not yet implemented**                       |
| `FROM`       | Import     | Opens a lazy module import block                         |
| `USE`        | Import     | Names the symbols to import inside a `FROM` block        |
| `AS`         | Import     | Binds an imported symbol under an alias name             |
| `BREAK`      | Loop       | Exits the enclosing loop immediately (new v1.4.2)        |
| `CONTINUE`   | Loop       | Skips to the next loop iteration (new v1.4.2)            |
| `YIELD`      | Generator  | **Reserved — not yet implemented**                       |
| `AWAIT`      | Async      | **Reserved — not yet implemented**                       |
| `ASSERT`     | Debug      | Runtime assertion (new v1.4.1)                           |
| `PASS`       | Control    | Explicit no-op (new v1.4.1)                              |
| `CONST`      | Variable   | Constant binding (new v1.4.1)                            |
| `NOT`        | Boolean    | Logical negation                                         |
| `AND`        | Boolean    | Logical AND                                              |
| `OR`         | Boolean    | Logical OR                                               |
| `GOTO`       | Control    | Jump to a named LOOP label (v1.4.6)                      |
| `LOOP`       | Control    | Define a named jump target for GOTO (v1.4.6)             |
| `TYPEOF`     | Type       | Returns type string of a value (v1.4.6)                  |

### New Operator Tokens (v1.4.1)

| Symbol   | Description             |
|----------|-------------------------|
| `{` `}`  | Brace block delimiters  |
| `;`      | Statement terminator    |
| `::`     | Namespace separator     |
| `\|>`    | Forward pipe operator   |
| `//`     | Floor division          |
| `->`     | Return type arrow       |
| `=>`     | Fat arrow / match arm   |
| `&` `\|` `~` `<<` `>>` | Bitwise operators |
| `?`      | Optional / ternary      |

> **Note:** `&&` and `||` are **not supported** — use `AND` and `OR` instead. `**` is **not supported** — use `^`. Compound assignment (`+=`, `-=`, `*=`, `/=`) is **not implemented**.

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
| `pow(x, y)`                     | Power (also `^`)                   |
| `min(a, b)` / `max(a, b)`       | Min / max of two values            |
| `log(x)` / `log2(x)`            | Natural / base-2 logarithm         |
| `sin(x)` / `cos(x)` / `tan(x)`  | Trigonometric functions            |
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
| `TYPEOF x`     | Keyword form of `type_of()` — same return values             |
| `error(msg)`   | Raise a runtime error with the given message                 |
| `raise(msg)`   | Alias for `error(msg)`                                       |

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
| `range(s, e)`             | List `[s, s+1, ..., e-1]`                |
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

Graphics in PASTA v1.5 is **opt-in** — import exactly what you need from the `graphics` module. No startup overhead or warnings on non-graphics scripts.

### 8.1 Importing Graphics

```pasta
FROM graphics:
    USE canvas_create, canvas_fill_rect, canvas_draw_circle,
        canvas_save_ppm, color_rgb, RED, GREEN, BLUE
END
```

Any graphics symbol used without an import will raise an undefined-variable error rather than silently failing.

### 8.2 2D Drawing API

#### Canvas lifecycle

| Function | Returns | Description |
|----------|---------|-------------|
| `canvas_create(w, h)` | handle | Create blank canvas |
| `canvas_clear(c, color)` | — | Fill canvas with color |
| `canvas_present(c)` | — | Push canvas to its window |
| `canvas_blit(c, dst)` | — | Copy canvas to another |
| `canvas_width(c)` | number | Canvas width |
| `canvas_height(c)` | number | Canvas height |
| `canvas_save_ppm(c, path)` | — | Save as PPM file |

#### Pixel operations

| Function | Description |
|----------|-------------|
| `canvas_set_pixel(c, x, y, color)` | Set one pixel |
| `canvas_get_pixel(c, x, y)` | Get packed color at pixel |

#### Drawing primitives

| Function | Description |
|----------|-------------|
| `canvas_draw_line(c, x0,y0, x1,y1, color)` | Bresenham line |
| `canvas_draw_rect(c, x,y, w,h, color)` | Rectangle outline |
| `canvas_fill_rect(c, x,y, w,h, color)` | Filled rectangle |
| `canvas_draw_circle(c, cx,cy, r, color)` | Circle outline |
| `canvas_fill_circle(c, cx,cy, r, color)` | Filled circle |
| `canvas_draw_ellipse(c, cx,cy, rx,ry, color)` | Ellipse outline |
| `canvas_fill_ellipse(c, cx,cy, rx,ry, color)` | Filled ellipse |
| `canvas_draw_triangle(c, x0,y0, x1,y1, x2,y2, color)` | Triangle outline |
| `canvas_fill_triangle(c, x0,y0, x1,y1, x2,y2, color)` | Filled triangle |
| `canvas_draw_polygon(c, points, color)` | Polygon outline |
| `canvas_fill_polygon(c, points, color)` | Filled polygon |
| `canvas_draw_arc(c, cx,cy, r, a0, a1, color)` | Arc segment |

#### Color helpers

| Function | Description |
|----------|-------------|
| `color_rgb(r, g, b)` | Pack RGB into u32 `0xFFRRGGBB` |
| `color_rgba(r, g, b, a)` | Pack RGBA |
| `color_hsv(h, s, v)` | HSV → packed RGB |
| `color_lerp(c1, c2, t)` | Linear interpolate between two colors |

### 8.3 Color Constants

32 named color constants available after import:

`RED` `GREEN` `BLUE` `WHITE` `BLACK` `YELLOW` `CYAN` `MAGENTA` `ORANGE` `PINK`  
`PURPLE` `BROWN` `GRAY` `LIGHT_GRAY` `DARK_GRAY` `LIME` `TEAL` `NAVY` `MAROON`  
`OLIVE` `SILVER` `GOLD` `INDIGO` `VIOLET` `CORAL` `SALMON` `KHAKI` `TURQUOISE`  
`LAVENDER` `BEIGE` `TRANSPARENT`

### 8.4 X11 Native Window Backend

Build with `--features x11` for live windows:

```bash
sudo pacman -S libx11          # Arch Linux
cargo build --release --features x11
```

Window functions (from the `graphics` module):

| Function | Description |
|----------|-------------|
| `window_create(title, w, h)` | Open a native window |
| `window_is_open(win)` | True while window is alive |
| `window_close(win)` | Close and free resources |
| `window_poll(win)` | Poll OS events |
| `window_key(win)` | Last key pressed (string) |

### 8.5 Graphics Examples

**Headless — draw shapes and save to PPM:**

```pasta
FROM:
    graphics
        use
            canvas_create, canvas_fill_rect, canvas_draw_circle,
            canvas_save_ppm, RED, BLUE
        END
    END
END

c = canvas_create(320, 240)
canvas_fill_rect(c, 20, 20, 100, 80, RED)
canvas_draw_circle(c, 200, 120, 60, BLUE)
canvas_save_ppm(c, "out.ppm")
PRINT "saved out.ppm"
```

**Live X11 window:**

```pasta
FROM:
    graphics
        use
            window_create, window_is_open, window_close, window_poll,
            canvas_create, canvas_fill_circle, canvas_present, GREEN
        END
    END
END

win = window_create("demo", 400, 300)
c   = canvas_create(400, 300)
canvas_fill_circle(c, 200, 150, 80, GREEN)
canvas_present(c)
WHILE window_is_open(win):
    window_poll(win)
END
window_close(win)
```

---

## 9. Shell / OS Layer

### 9.1 Script Shell

The script shell (`src/interpreter/shell.rs`) lets Pasta scripts run OS commands and shell pipelines as strings. It supports:

- **Quote parsing** — `"arg with spaces"` and `'single quotes'` work correctly
- **Env var expansion** — `$HOME`, `${VAR}` expanded before execution
- **Glob expansion** — `*.ps`, `data_?.csv`, `[abc]*` expanded to matching files
- **IO redirection** — `cmd > file`, `cmd >> file`, `cmd < file`
- **Pipes** — `cmd1 | cmd2 | cmd3` with stdin/stdout chaining
- **Text builtins** — `echo`, `grep`, `wc`, `sort`, `uniq`, `head`, `tail`, `find`
- **Binary whitelist** — external execution limited to `/bin`, `/usr/bin`, `/usr/local/bin`, and relative paths

```pasta
# From a Pasta script
shell("ls *.ps | sort | head -5")
shell("grep 'RETURN' producer.ps > results.txt")
```

### 9.2 Interactive Shell

Accessed from the REPL via `:shell` or by running `pasta --shell`. Features:

- Full VFS (virtual filesystem) backed by `DiskImages/fs.img`
- Standard commands: `ls`, `cd`, `pwd`, `cat`, `mkdir`, `rm`, `cp`, `mv`, `touch`
- All script-shell features (pipes, redirect, glob, env vars) available interactively
- Pipeline syntax: `a.ps | b.ps | c.ps` launches a multi-stage script pipeline

---

## 10. Pipeline System

### 10.1 Script Pipelines

PASTA v1.5 supports **multi-stage script pipelines** — up to 8 `.ps` files chained with `|`:

```bash
# At the REPL or interactive shell:
pasta> producer.ps | transform.ps | consumer.ps
```

**How it works:**

- Each stage runs in its own thread, registered in the global thread registry
- **First stage** — runs once; every `RETURN value` sends `value` downstream
- **Middle / last stages** — run once per incoming item; `PIPE_IN` holds the received value; `RETURN` forwards downstream (last stage discards)
- Threads are named `pipeline-{id}-stage-{n}` and visible in `:threads`
- Errors in one stage are logged to stderr; other stages continue normally
- Fire-and-forget: the REPL returns immediately while stages run in background

```pasta
# producer.ps
FOR i IN range(1, 6):
    RETURN i
END

# double.ps — runs once per value received
PRINT "doubling " + str(PIPE_IN)
RETURN PIPE_IN * 2

# printer.ps — terminal stage
PRINT "result: " + str(PIPE_IN)
```

```bash
pasta> producer.ps | double.ps | printer.ps
spawned pipeline (3): producer.ps | double.ps | printer.ps
doubling 1
doubling 2
...
result: 2
result: 4
...
```

**From the CLI binary:**

```bash
pasta "producer.ps|double.ps|printer.ps"
pasta --spawn-pipeline left.ps right.ps
```

**Thread inspection:**

```
pasta> :threads
  THID      NAME                              STATUS        ELAPSED
  ----------------------------------------------------------------------
  1         pipeline-1-stage-0               finished      12ms
  2         pipeline-1-stage-1               running       8ms
  3         pipeline-1-stage-2               running       8ms

pasta> :thread-details 2
Thread THID:2
  name:    pipeline-1-stage-1
  status:  running
  elapsed: 8ms (still running)
  pipeline id:    1
  pipeline stage: 2/3
```

### 10.2 Expression Pipe Operator `|>`

For expression-level piping within a single script:

```pasta
double = LAMBDA x: x * 2 END
square = LAMBDA x: x * x END

result = 3 |> double |> square    # (3*2)^2 = 36
PRINT result
```

`value |> fn` passes `value` as the argument to `fn`. Works with any callable.

---

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
sudo cp target/release/pasta /usr/local/bin/pasta
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

### v1.6.1 — Current Release

#### New Language Features

- **`color(r, g, b)`** — pack three 0-255 bytes into a `0xFFRRGGBB` color value; always available (no import needed)
- **`COLOR_GREY`** — additional named color constant (alias for mid-grey)
- **`range(n)`** — returns `[1..n]` inclusive integer list; `range(s, e)` returns `[s..e]` inclusive
- **`LIST_CONTAINS(list, val)`** — returns `true` if `val` is an element of `list`; O(n) linear scan
- **Dict literal syntax** — `{"key": val, ...}` as an expression; works in assignments, DEF returns, and nested expressions
- **`WHILE(UNBIND_SCOPE)` / `FOR(UNBIND_SCOPE)` / `IF(UNBIND_SCOPE)`** — push a new Block scope; variables created inside die when the block exits
- **`WHILE(BIND_SCOPE)` / `FOR(BIND_SCOPE)` / `IF(BIND_SCOPE)`** — push a Block scope but hoist all vars to the nearest enclosing Function or Global scope on exit

#### Bug Fixes

- **Lexer: blank lines in indented blocks** — blank or comment-only lines no longer emit spurious `Dedent`/`Indent` tokens that mangled adjacent expressions (previously caused "undefined variable" on lines after blank lines inside DEF bodies)
- **Parser: `parse_do_body` single-newline skip** — block bodies now correctly skip any number of blank/comment-only lines between a `WHILE`/`FOR`/`IF` header and its first indented statement; previously a single blank line caused the entire block body to be parsed as top-level code

#### Game Demo

- **`examples/game_demos/snake.ps`** — classic Snake game using v1.6.1 features: `color()`, dict literals, `range()`, `LIST_CONTAINS`, `WHILE`/`FOR IN`, `DEF` functions, live X11 window with FPS throttling

---

### v1.5.0

#### Graphics Revamp (opt-in, fully redesigned)

- **`FROM graphics: use ... END` (block indent form)** — Graphics is now an opt-in library; no startup warnings on non-graphics scripts
- **Full 2D drawing API**: `canvas_draw_line`, `canvas_draw_rect`, `canvas_fill_rect`, `canvas_draw_circle`, `canvas_fill_circle`, `canvas_draw_ellipse`, `canvas_fill_ellipse`, `canvas_draw_triangle`, `canvas_fill_triangle`, `canvas_draw_polygon`, `canvas_fill_polygon`, `canvas_draw_arc`
- **32 named color constants** imported on demand: `RED`, `GREEN`, `BLUE`, `WHITE`, `BLACK`, `YELLOW`, `CYAN`, `MAGENTA`, `ORANGE`, `PINK`, `PURPLE`, `BROWN`, `GRAY`, `LIGHT_GRAY`, `DARK_GRAY`, `LIME`, `TEAL`, `NAVY`, `MAROON`, `OLIVE`, `SILVER`, `GOLD`, `INDIGO`, `VIOLET`, `CORAL`, `SALMON`, `KHAKI`, `TURQUOISE`, `LAVENDER`, `BEIGE`, `TRANSPARENT`
- **Color helpers**: `color_rgb()`, `color_rgba()`, `color_hsv()`, `color_lerp()`
- Removed `pasta_G.ph` auto-load; removed startup warning "Could not find stdlib graphics helpers"

#### Pipeline System (new)

- **Multi-stage script pipelines** — `a.ps | b.ps | c.ps` up to 8 stages; each stage is its own thread
- **`PIPE_IN` variable** — downstream stages receive per-item value via `PIPE_IN` global
- **`RETURN` sends downstream** — every `RETURN value` in a pipeline stage forwards the value to the next stage
- **Thread registry integration** — stage threads named `pipeline-{id}-stage-{n}`, visible in `:threads`
- **`:thread-details N`** REPL command shows pipeline metadata per thread
- **Fire-and-forget** — REPL returns immediately; stages run in background
- **`--spawn-pipeline`** CLI flag and inline `|` syntax in `pasta "a.ps|b.ps"` both supported

#### Shell Overhaul (P1 security + P2 features)

- **P1 security**: Quote tokenizer (`"arg with spaces"`, `'quotes'`), safe VFS path traversal (replaced unsafe raw pointer `walk_mut`), binary execution whitelist, `..` at root boundary fix
- **P2 features**: IO redirect (`>`, `>>`, `<`), env var expansion (`$VAR`, `${VAR}`), glob expansion (`*`, `?`, `[abc]`), multi-stage shell pipes, text builtins (`echo`, `grep`, `wc`, `sort`, `uniq`, `head`, `tail`, `find`)

#### Keywords & REPL

- **`:keywords`** REPL command completely rewritten — now lists all ~100+ builtins and keywords grouped by category
- **`:thread-details N`** — new REPL command for per-thread pipeline metadata
- **REPL banner** updated to `PASTA v1.5`

---

### v1.4.6

#### Family Object System (★ Major New Feature)

- **`OBJ.FAM` syntax.** A dual-parent lineage object model built into the language. Every family node carries exactly two parent slots (`pa`, `pb`), an adoption state machine, and a reconciliation engine.
- **`x = OBJ.LST(pa, pb)`** — allocate an immutable family node. `OBJ.LST.MUT(pa, pb)` allocates a mutable node.
- **`DOES_PARENT_EXIST x`** — boolean check whether a node has live parent references.
- **`::USE UNSAFE-READ::` / `::USE UNSAFE-WRITE::`** — opt-in unsafe permission pragmas for low-level family access.
- **`TYPEOF x`** — keyword now correctly parses and dispatches to the `type()` builtin (was silently broken).
- **Full runtime:** `FamilyRegistry`, `FamilyEventBus`, ASM (`AdoptionStateMachine`), reconciliation (Option C), GC hooks, snapshot/recovery API, and structured `LineageError` diagnostics.
- **22 family tests** covering all subsystems.

#### Keyword Completions

- **`PASS`** — explicit no-op statement (consumes a line, emits nothing).
- **`ASSERT expr`** — runtime assertion; raises `"Assertion failed"` if `expr` is falsy. Compiles to `IF NOT expr: error("Assertion failed") END`.
- **`UNLESS cond: body END`** — inverted conditional; compiles to `IF NOT cond: body END`.
- **`error(msg)` / `raise(msg)`** — builtin functions that immediately raise a runtime error.

#### Correctness Fixes

- **Alias conflict resolution.** `"with"` was shadowed by both FOR and WITH (WITH wins); `"when"` by IF and WHEN (WHEN wins); `"await"` by WAIT and AWAIT (AWAIT wins). Dead first-registrations removed.
- **Type coercion unification.** `Eq`/`Neq` comparisons now use the same `coerce_to_number` closure as arithmetic — `"" == 0` is now `true` (consistent with `"" + 5 = 5`).
- **Heap truthiness.** `Value::Heap` is dereferenced before truthiness check; non-empty lists correctly evaluate as truthy.
- **Exponent right-associativity confirmed.** `2^3^2 = 512` ✅
- **Pipeline operator precedence confirmed.** `double |> 5 = 10` ✅

#### TRY / OTHERWISE

- Verified correct behaviour in all contexts: basic, ATTEMPT with error binding, nested TRY, TRY inside FOR IN / WHILE loops, TRY inside function bodies, bare TRY (no OTHERWISE).
- **13 integration tests** added in `tests/try_otherwise_integration.rs`.

#### Tests

- **206 tests passing**, up from 146 in v1.4.5.
- New test files: `tests/try_otherwise_integration.rs` (13 tests).

---

### v1.4.5

#### Scope System (★ Major Rework)

- **Dedicated `scope.rs` module.** All scope semantics are now centralized in `src/interpreter/scope.rs`. The module provides `ScopeKind`-aware `scope_assign()`, `enter()`/`leave()` wrappers, and convenience accessors — replacing scattered inline scope logic.
- **`ScopeKind` enum.** Every scope frame is now tagged as `Global`, `Function`, or `Block`:
  - `Function` — hard boundary (function/lambda calls, module bodies, thread bodies). New variables created inside stay inside and are discarded on return.
  - `Block` — soft boundary (IF bodies, loop bodies, DO bodies). New variables escape to the nearest `Function`/`Global` scope so they persist after the block ends.
- **`scope_assign()` replaces `scopes.len() > 1` hack.** The old assignment handler used `if scopes.len() > 1 { set_local } else { assign }`, which caused ALL assignments inside any scoped block to create a local copy. The new logic: (1) if the variable already exists anywhere, update it in place; (2) if the variable is new, create it in the nearest `Function` or `Global` scope.
- **Variable persistence fixed.** Variables assigned inside `IF`, `WHILE`, and `DO` bodies now correctly persist in the enclosing function/global scope.
- **All 18+ `push_scope` call sites tagged** with the correct `ScopeKind`.

#### GOTO Loops

- **`LOOP` named-loop keyword** defines a named jump target for `GOTO`.
- **Syntax:** `name = LOOP … GOTO name … END` — `GOTO name` restarts the loop from the top.

#### Timed DO Loops

- **`DO FOR <N>ms` timed loops** execute a body for a wall-clock duration.
- **Syntax:** `DO FOR 500ms\n    body\nEND` — runs `body` repeatedly for 500 milliseconds.
- **Lexer fix:** `500ms` (no space) now correctly tokenizes as `Number(500) + Identifier("ms")` instead of a single unknown identifier.
- **Alias conflict fix:** `"do"` was wrongly aliased to `THEN` in `alias.rs`, causing all `DO` blocks to be misidentified. Fixed by removing `"do"` from the `THEN` alias list.

#### Parser

- **`IF` without colon now accepted.** `IF cond\n    body\nEND` (Python-style newline-indented body) no longer causes a parse error. `THEN`, `DO`, `:`, and a bare newline are all valid body openers after an `IF` condition.
- **Both DO-WHILE syntaxes supported:**
  - `DO targets WHILE condition: body` — original (condition first, body after colon)
  - `DO\n    body\nWHILE condition\nEND` — C-style (body first, condition after)

#### Tests

- **146 tests passing**, up from 145 in v1.4.4.
- New tests in `scope.rs`: new-var-in-block-escapes-to-global, new-var-in-function-stays-local, update-existing-var-from-inside-block, nested-blocks-escape-to-function-scope.

---

### v1.4.4

#### Pointer & Reference System (★ Major New Feature)

- **Unified pointer abstraction.** Four pointer kinds — `MEM`, `FILE`, `DEV`, `NET` — providing a consistent API for memory buffers, file handles, device I/O, and network sockets.
- **`ALLOC.<KIND>(args)` statement.** Allocates a new pointer: `ALLOC.MEM(1024) -> buf`, `ALLOC.FILE("/tmp/data", "r") -> fh`, `ALLOC.NET("localhost", 8080) -> sock`.
- **`GOTO ptr:` context block.** Sets the active pointer for `PULL`/`PUSH` operations. Supports nested contexts with proper scoping.
- **`PULL.<TYPE>` / `PUSH.<TYPE>` data transfer.** Read/write bytes, integers, floats, strings, and raw byte arrays to the active pointer: `PUSH.BYTE 0x48`, `PULL.INT -> value`, `PULL.STR(32) -> s`.
- **`INFO ptr` inspection.** Returns metadata list: `[[id, N], [kind, MEM], [alive, true], [temporary, false], [size, 1024], [offset, 0]]`.
- **`REF.<KIND>(target)` expression.** Create pointer references with optional `WITH { key: value }` metadata: `REF.FILE("/etc/passwd") WITH { mode: "r" }`.
- **`FREE ptr` deallocation.** Explicitly release pointer resources. Accessing a freed pointer raises `E070`.
- **PointerRegistry runtime.** Thread-safe `Arc<RwLock<PointerRegistry>>` in executor for cross-scope pointer access.
- **PointerContext stack.** Active pointer context managed via `GOTO`/`END` blocks with nested scope support.
- **GC integration.** `PointerGcTracker` automatically frees temporary pointers allocated within `GOTO` blocks when the block exits.

#### Error Handling

- **Pointer error codes.** Six new error codes for pointer operations:
  - `E070`: `PTR_USE_AFTER_FREE` — pointer accessed after `FREE`
  - `E071`: `PTR_KIND_MISMATCH` — operation unsupported for pointer kind
  - `E072`: `PTR_NO_CONTEXT` — `PULL`/`PUSH` without active `GOTO` context
  - `E073`: `PTR_NOT_FOUND` — pointer ID not in registry
  - `E074`: `PTR_INVALID_TYPE` — expected pointer value, got other type
  - `E075`: `PTR_METADATA_ERROR` — invalid metadata in `REF` expression

#### Tests

- **157 tests passing** (119 unit + 38 integration), up from 145 in v1.4.3.
- **`tests/pointer_test.pasta`** — Basic pointer allocation and operations.
- **`tests/pointer_gc_test.pasta`** — GC scope behavior and temporary pointer cleanup.
- **`tests/pointer_error_test.pasta`** — Error code validation.
- **`tests/pointer_integration_test.pasta`** — 12-test comprehensive suite: ALLOC operations, GOTO context, nested GOTO, PUSH/PULL data types, REF expressions, FREE cleanup.

---

### v1.4.3

#### Language

- **String interpolation fully implemented.** `"hello {name}"` — any Pasta expression can appear inside `{…}` in a string literal; the expression is lexed, parsed, and evaluated at runtime, and its result is stringified. `{{` produces a literal `{`, `}}` produces a literal `}`. Works in `SET`, `PRINT`, function arguments, and all other string contexts.
- **`RET.NOW expr` without parentheses fixed.** The parser now recognises the three-token form `RET . NOW` (emitted when there is whitespace before the dot) in addition to the single-token absorbed form `RET.NOW`, making `RET.NOW a + b` equivalent to `RET.NOW(): a + b`.

#### Tests

- **145 tests passing** (119 unit + 26 integration), up from 135 in v1.4.2.
- **`tests/string_interp_integration.rs`** — 10 tests: basic variable substitution, multiple interpolations per string, arithmetic inside `{…}`, double-brace escaping, plain string fast-path, builtin call inside interpolation, `SET`/`PRINT` path, multi-argument function call via `RET.NOW`.

---

### v1.4.2

#### Module Import System

- **Block indent `FROM` / `use` / `AS` import syntax.** Symbols are lazily bound on first access; the module file is loaded, parsed, and executed only once per session.
- **Alias binding fixed.** `add as myplus` correctly registers `myplus` in `exec.functions` so named parameters are available.
- **Error propagation fixed.** A missing module now surfaces a `ModuleNotFoundError` immediately at call time rather than falling through to an unhelpful "unknown function" message.
- **`PASTA_MODULE_PATH` environment variable.** Colon-separated list of directories appended to the module search path, enabling installed stdlib and project-specific modules.
- **Stdlib search path corrected.** `resolve_module_path` now correctly resolves the development `src/stdlib/` directory and the legacy `stdlib/` layout.

#### Bug fixes

- **`runtime_error_includes_traceback` test fixed.** `pop_frame` was being called before `span_err` in `WhileBlock` (×3) and `ForIn` (×1) error paths, causing an empty traceback. Fixed by removing the premature pops.
- **All debug instrumentation removed.** ~25 `eprintln!`/`println!` calls removed from `ex_eval.rs`, `executor.rs`, and `environment.rs`; `[ENV_GET_FAIL]` per-lookup dump removed from `environment::get()`.

#### Language

- **`BREAK` and `CONTINUE` keywords fully implemented.** End-to-end loop-control flow: lexer → alias table → AST → parser → `ControlFlowSignal` → evaluator. `BREAK` exits the innermost loop immediately; `CONTINUE` skips the remainder of the current iteration and re-evaluates the loop condition. Both uppercase (`BREAK`/`CONTINUE`) and lowercase (`break`/`continue`) aliases work. Correct signal propagation: `Return`/`Killed` signals pass through loop boundaries; `Break`/`Continue` are consumed by their own loop boundary and do not leak to outer scopes.
- **`PRINT` multi-argument formatting fixed.** `PRINT a, ",", b` was rendering as `[a, ,, b]` (list with brackets) because `do_print` printed `Value::List` with the same `[…]` format as REPL display. `do_print` now renders a `Value::List` as space-separated items without brackets.
- **`PASTA_MODULE_PATH` test race fixed.** `resolve_honours_pasta_module_path_env` and `resolve_pasta_module_path_multiple_entries` were non-deterministically failing under parallel test execution due to concurrent `env::set_var` calls. Serialised with a per-file `OnceLock<Mutex<()>>` guard.
- **Install path corrected.** `tools/install_pasta.sh` now installs to `/usr/local/bin/pasta` instead of `/usr/bin/pasta`.

#### Tests

- **135 tests passing** (119 unit + 16 integration), up from 50 in v1.4.1.
- **`tests/from_use_integration.rs`** — 4 tests: basic lazy load, alias (`add as plus`), multi-symbol, module-not-found error.
- **`tests/module_path_resolution.rs`** — 5 tests covering CWD, `modules/` subdir, `PASTA_MODULE_PATH` (single and multi-entry), and missing-module error quality.
- **`tests/break_continue_integration.rs`** — 6 tests: `WHILE` break, nested break (inner-only), `WHILE` continue, `FOR IN` break, `FOR IN` continue, lowercase alias round-trip.

---

### v1.4.1

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

### Completed in v1.6.1

- [x] **`color(r,g,b)` builtin** — pack RGB into color value
- [x] **`COLOR_GREY` constant** — additional named color
- [x] **`range(n)` / `range(s,e)`** — inclusive integer range builtins
- [x] **`LIST_CONTAINS(list, val)`** — membership test builtin
- [x] **Dict literal `{...}` syntax** — inline dict construction expressions
- [x] **Scope modifiers** — `UNBIND_SCOPE` / `BIND_SCOPE` on WHILE/FOR/IF blocks
- [x] **Lexer blank-line bug fixed** — spurious Dedent/Indent on blank lines in blocks
- [x] **Parser parse_do_body bug fixed** — blank lines between block header and body
- [x] **Snake game demo** — `examples/game_demos/snake.ps`

### Completed in v1.5.0

- [x] **Graphics revamp** — opt-in block `FROM graphics: use ... END`, full 2D API, 32 color constants
- [x] **Pipeline system** — multi-stage MPSC, `PIPE_IN`, `RETURN` forwarding, thread registry, `:thread-details`
- [x] **Shell overhaul** — P1 security (quote tokenizer, safe VFS traversal, whitelist), P2 features (redirect, glob, env vars, text builtins)
- [x] **`:keywords` comprehensive listing** — all ~100+ builtins grouped by category
- [x] **Version bumped to 1.5.0**

### Previously Completed

- [x] **`BREAK` and `CONTINUE` keywords** (v1.4.2)
- [x] **String interpolation** `"hello {name}"` (v1.4.3)
- [x] **`FROM` / `USE` / `AS` module import system** (v1.4.2)
- [x] **Family Object System** `OBJ.FAM` (v1.4.6)
- [x] **`PASS`, `ASSERT`, `UNLESS` keywords** (v1.4.6)
- [x] **`TRY / OTHERWISE` integration tests** (v1.4.6)

### Near-Term (v1.6.1.x)

- [ ] **Windows support** — pack pasta into a `.exe` installer via `cross` cross-compilation; Win32 display backend; readline fallback for Windows terminal
- [ ] **Dictionaries / Maps** — `{key: value}` literal with `dict.get`, `dict.set`, `dict.keys`, `dict.values`, `dict.contains`, `dict.remove`, `dict.len`
- [ ] **`FOR IN` with index** — `enumerate(lst)` builtin or `FOR i idx IN list` syntax
- [ ] **REPL history persistence** — save/restore `~/.pasta_history` across sessions
- [ ] **GLOB.DEF** — global function modifier prefix (currently deferred by design)

### Medium-Term (v1.6.1)

- [ ] **True lexical closure capture** — proper capture-at-definition semantics
- [ ] **Typed exceptions** — `CATCH TypeError`, `CATCH IOError` in `TRY/OTHERWISE`
- [ ] **`MATCH` / pattern matching** — `MATCH value: CASE x: ... END`
- [ ] **Multi-line REPL** — continuation detection for multi-line `DEF` and `IF` blocks
- [ ] **`stdio.*` namespace** — `stdio.read_line()`, `stdio.write(s)`
- [ ] **SHM X11 extension** — `XShmPutImage` for zero-copy blit

### Long-Term / Architecture

- [ ] **MRA backends** — implement `local` and `pseudo-vm` in `meatballs/backends/`
- [ ] **Bytecode compiler** — AST → compact bytecode for faster repeated execution
- [ ] **Garbage collector** — replace drop-based reclamation with tracing GC
- [ ] **Native async** — `ASYNC DEF` / `AWAIT` surfacing `pasta_async` as first-class keywords
- [ ] **AI model training pipeline** — wire `learn.rs` to `tensor.*` stdlib
- [ ] **LSP / language server** — autocomplete, go-to-definition, hover docs
- [ ] **Wayland backend** — `wl_surface` + `wl_shm` for compositor-agnostic rendering
- [ ] **Tail-call optimization** — unbounded recursion depth for tail-recursive patterns

---

*PASTA v1.6.1 — Built with ❤️ in Rust*  
*Project root: `/home/travis/pasta` · Platform: Arch Linux*  
*Native X11 graphics pipeline: `cargo build --release --features x11`*
