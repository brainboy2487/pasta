# PASTA — Program for Assignment, Statements, Threading, and Allocation

> **Version 1.4** · Scripting Language Interpreter written in Rust  
> Platform: Arch Linux · Build: `cargo build --release` · Root: `/home/travis/pasta`

[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#18-configuration--build)
[![Tests](https://img.shields.io/badge/tests-50%2F50-brightgreen)](#17-test-suite)
[![Language](https://img.shields.io/badge/language-Rust-orange)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-1.4-blue)](#19-changelog)

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
5. [Keywords Reference](#5-keywords-reference)
6. [Built-in Functions](#6-built-in-functions)
7. [Standard Library Modules](#7-standard-library-modules)
8. [Type System](#8-type-system)
9. [Shell / OS Layer](#9-shell--os-layer)
10. [Async Runtime — pasta_async](#10-async-runtime--pasta_async)
11. [AI / ML Operations](#11-ai--ml-operations)
12. [Meatball Runtime Architecture (MRA)](#12-meatball-runtime-architecture-mra)
13. [REPL & CLI](#13-repl--cli)
14. [readline Module](#14-readline-module)
15. [Architecture Overview](#15-architecture-overview)
16. [Typing System Internals](#16-typing-system-internals)
17. [Test Suite](#17-test-suite)
18. [Configuration & Build](#18-configuration--build)
19. [Changelog](#19-changelog)
20. [Roadmap / To-Do](#20-roadmap--to-do)

---

## 1. Overview

PASTA is a full-featured, embeddable scripting language interpreter written entirely in Rust. It is designed for expressiveness, safety, and extensibility — combining a clean high-level syntax with direct access to OS primitives, a virtual filesystem, async I/O, AI/ML tensor operations, and a novel Meatball Runtime Architecture (MRA) for agent-based execution.

**Core design goals:**

- Readable, colon-terminated block syntax for scripts and interactive use
- Rust-native performance with a safe ownership model under the hood
- First-class support for stdlib namespaces: `sys`, `fs`, `net`, `time`, `rand`, `gc`, `debug`, `ffi`, `thread`, `device`, `tensor`, `memory`
- Integrated shell/OS layer with a virtual filesystem (VFS) and `shell_os` subsystem
- Pluggable typing system with configurable numeric coercion, promotion, and rounding
- Production-ready REPL with raw-mode readline, 50-entry history ring, and full cursor navigation
- Scaffold for a Meatball Runtime Architecture (MRA) enabling agent-based and multi-backend workloads
- Full 50-section regression suite passing cleanly as of v1.4

---

## 2. Quick Start

### Build

```bash
cd /home/travis/pasta
cargo build --release
```

### Run a script

```bash
./target/release/pasta tests/09_big_test.ps    # 30-section regression suite
./target/release/pasta tests/10_full_suite.ps  # 50-section full suite
```

### Interactive REPL

```bash
./target/release/pasta
# PASTA interpreter — :help for commands, exit to quit
pasta> PRINT "hello world"
hello world
pasta> x = 10
pasta> PRINT x * 2
20
pasta> exit
Goodbye.
```

### Run all tests

```bash
./target/release/pasta tests/10_full_suite.ps   # => === ALL 50 TESTS COMPLETE ===
./target/release/pasta tests/09_big_test.ps     # => === ALL TESTS COMPLETE ===
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
│   │   ├── mod.rs
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
│   │   ├── errors.rs              # Runtime error types
│   │   ├── executor.rs            # Core interpreter dispatch loop
│   │   ├── mod.rs
│   │   ├── repl.rs                # Interactive REPL
│   │   └── shell.rs               # Shell integration shim
│   ├── lexer/
│   │   ├── alias.rs               # Dotted-namespace token absorption
│   │   ├── lexer.rs               # Tokenizer
│   │   ├── mod.rs
│   │   ├── tokens.rs              # Token definitions
│   │   └── unicode.rs             # Unicode helpers
│   ├── meatballs/                 # MRA scaffold
│   │   ├── agent/                 # agent.rs, Cargo.toml
│   │   ├── api/                   # meatball_api.rs, mod.rs
│   │   ├── backends/              # Backend stubs (local, pseudo-vm, vm)
│   │   ├── cli/                   # cli.rs
│   │   ├── kernel/
│   │   ├── phase0/                # mra_schema.json, objective.md.txt
│   │   ├── runtime/               # runtime.rs
│   │   └── tests/
│   ├── parser/
│   │   ├── ast.rs                 # AST node definitions
│   │   ├── grammar.rs             # Grammar rules
│   │   ├── mod.rs
│   │   └── parser.rs              # Recursive descent parser
│   ├── pasta_async/               # Async runtime (sub-crate)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── api.rs
│   │       ├── io.rs
│   │       ├── lib.rs
│   │       ├── runtime.rs
│   │       ├── serialize.rs
│   │       ├── sync.rs
│   │       └── testing.rs
│   ├── runtime/
│   │   ├── asm.rs                 # AsmRuntime / AsmBlock
│   │   ├── bitwise.rs             # Bitwise operations
│   │   ├── devices.rs             # Device detection & profiles
│   │   ├── device_ls.json         # Known device list
│   │   ├── meatball.rs            # Meatball runtime bridge
│   │   ├── mod.rs                 # auto_configure, detect_host_arch
│   │   ├── rng.rs                 # RNG utilities
│   │   ├── scheduler.rs           # Task scheduler
│   │   ├── strainer.rs            # Data filtering/straining
│   │   └── threading.rs           # Thread primitives
│   ├── semantics/
│   │   ├── constraints.rs         # Type & value constraints
│   │   ├── mod.rs
│   │   ├── priority.rs            # PRIORITY semantic pass
│   │   └── resolver.rs            # Name & scope resolution
│   ├── stdlib/                    # Standard library (.ph headers + .pa source)
│   │   ├── debug.ph
│   │   ├── device.ph
│   │   ├── ffi.ph
│   │   ├── fs.ph
│   │   ├── gc.ph
│   │   ├── lib.rs                 # Stdlib loader
│   │   ├── math.ph
│   │   ├── memory.ph
│   │   ├── net.ph
│   │   ├── pasta_G.ph             # Global pasta_G helpers
│   │   ├── rand.ph
│   │   ├── stdio.ph
│   │   ├── stdlib.pa              # Core stdlib in PASTA source
│   │   ├── sys.ph
│   │   ├── tensor.ph
│   │   ├── thread.ph
│   │   └── time.ph
│   ├── typing/                    # Type system
│   │   ├── bool.rs / bool_coerce.rs
│   │   ├── float.rs               # Float helpers, rounding, formatting
│   │   ├── int.rs
│   │   ├── lib.rs
│   │   ├── mod.rs
│   │   ├── operands.rs            # compute_numeric_op, apply_round_and_downcast
│   │   ├── string.rs / string_coerce.rs
│   │   ├── tensor_type.rs         # Tensor type definition
│   │   ├── types.rs               # Core Value enum
│   │   └── util.rs                # Numeric promotion, engine-config helpers
│   ├── utils/
│   │   ├── errors.rs
│   │   ├── helpers.rs
│   │   ├── logging.rs
│   │   └── mod.rs
│   ├── lib.rs                     # Crate root — public API re-exports
│   └── readline.rs                # Raw-mode line editor
├── tests/
│   ├── 01_arithmetic_bindings.ps
│   ├── 01_basic_print.ps
│   ├── 01_string_probe.ps
│   ├── 04_basic_while.ps
│   ├── 05_nested_while.ps
│   ├── 06_functions_and_lambdas.ps
│   ├── 07_do_multi_alias_repeat.ps
│   ├── 08_test_RET.ps
│   ├── 09_big_test.ps             # 30-section regression suite
│   ├── 10_full_suite.ps           # 50-section full suite
│   ├── 10_small_test_30.ps
│   ├── mand_test.ps               # Mandelbrot stress test
│   ├── rewrite_test.ps
│   ├── test_advanced.ps
│   └── test_suite.ps
├── docs/
│   ├── meatball_readme.txt
│   ├── README1.txt
│   ├── README2.txt
│   ├── shell_readme.txt
│   └── typing_readme.txt
├── DiskImages/
│   └── fs.img                     # Virtual disk image
├── tools/
│   └── output_dir_tree.py
├── examples/
├── Cargo.toml
└── Cargo.lock
```

---

## 4. Language Reference

PASTA scripts use the `.ps` extension. Comments begin with `#`. All blocks open with a colon (`:`) on the header line and are terminated with `END`.

### 4.1 Literals

| Type    | Example           | Notes                       |
|---------|-------------------|-----------------------------|
| Integer | `42`, `0`, `-1`   | 64-bit signed               |
| Float   | `3.14`, `-0.5`    | f64 internally              |
| Bool    | `true`, `false`   | Lowercase required          |
| String  | `"hello pasta"`   | Double-quoted UTF-8         |
| List    | `[1, 2, 3]`       | Heterogeneous values ok     |
| Null    | implicit / unset  | Unassigned variable         |

```pasta
PRINT 42
PRINT 3.14
PRINT true
PRINT false
PRINT "hello world"
PRINT 0
PRINT -1
```

### 4.2 Variables

Variables are assigned with `=`. No declaration keyword is required. Rebinding is allowed. Variable names are case-sensitive.

```pasta
x = 10
y = x + 5
PRINT x        # 10
PRINT y        # 15
x = x * 2
PRINT x        # 20
```

### 4.3 Operators

**Arithmetic**

| Operator | Description      | Example     | Result |
|----------|------------------|-------------|--------|
| `+`      | Addition         | `2 + 3`     | `5`    |
| `-`      | Subtraction      | `10 - 4`    | `6`    |
| `*`      | Multiplication   | `3 * 7`     | `21`   |
| `/`      | Division (float) | `15 / 4`    | `3.75` |
| `%`      | Modulo           | `17 % 5`    | `2`    |
| `^`      | Exponentiation   | `2 ^ 8`     | `256`  |

Operator precedence follows standard mathematical convention. Use parentheses `()` for explicit grouping.

```pasta
PRINT 1 + 2 * 3      # 7
PRINT (1 + 2) * 3    # 9
PRINT 100 / 10 / 2   # 5
PRINT 2 ^ 3 ^ 2      # 512
```

**Comparison**

| Operator | Description      |
|----------|------------------|
| `==`     | Equal            |
| `!=`     | Not equal        |
| `>`      | Greater than     |
| `<`      | Less than        |
| `>=`     | Greater or equal |
| `<=`     | Less or equal    |

String equality is supported via `==` and `!=`.

**Boolean**

| Operator | Description  |
|----------|--------------|
| `&&`     | Logical AND  |
| `\|\|`   | Logical OR   |
| `NOT`    | Logical NOT  |

```pasta
PRINT true && false        # false
PRINT false || true        # true
PRINT NOT (1 == 2)         # true
PRINT (1 < 2) && (3 < 4)  # true
```

### 4.4 Strings

String variables support positive and negative indexing and a full set of built-in operations.

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

Lists are heterogeneous, zero-indexed, and support a comprehensive set of built-in operations.

```pasta
nums = [10, 20, 30, 40, 50]
PRINT nums[0]                     # 10
PRINT list_len(nums)              # 5
PRINT list_sum(nums)              # 150
PRINT list_min(nums)              # 10
PRINT list_max(nums)              # 50
PRINT list_avg(nums)              # 30.0
PRINT list_sort(nums)             # [10, 20, 30, 40, 50]
PRINT list_rev(nums)              # [50, 40, 30, 20, 10]
PRINT list_slice(nums, 1, 3)      # [20, 30]
PRINT list_push(nums, 60)         # appends 60, returns new list
PRINT list_pop(nums)              # removes and returns last element
PRINT list_concat([1,2], [3,4])   # [1, 2, 3, 4]
PRINT list_first(nums)            # 10
PRINT list_last(nums)             # 50
PRINT list_contains(nums, 30)     # true

# range() — start, stop, optional step
PRINT range(0, 5)                 # [0, 1, 2, 3, 4]
PRINT range(2, 6)                 # [2, 3, 4, 5]
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

# Standalone IF (no OTHERWISE required)
IF x == 10:
    PRINT "ten"
END

# Nested IF / OTHERWISE
IF x < 0:
    PRINT "negative"
OTHERWISE:
    IF x == 0:
        PRINT "zero"
    OTHERWISE:
        PRINT "positive"
    END
END
```

### 4.7 WHILE Loops

Both `WHILE` and `DO WHILE` forms are supported and have identical semantics.

```pasta
# Standard WHILE
i = 0
WHILE i < 5:
    PRINT i
    i = i + 1
END

# DO WHILE form
i = 0
DO WHILE i < 3:
    PRINT i
    i = i + 1
END

# Accumulator pattern
total = 0
n = 1
WHILE n <= 10:
    total = total + n
    n = n + 1
END
PRINT total   # 55
```

**Nested WHILE:**

```pasta
outer = 1
DO WHILE outer <= 3:
    inner = 1
    DO WHILE inner <= 3:
        IF outer == inner:
            PRINT outer
        END
        inner = inner + 1
    END
    outer = outer + 1
END
# prints: 1  2  3
```

### 4.8 FOR IN Loops

Iterate over lists, ranges, or strings.

```pasta
# Over a list
FOR x IN [10, 20, 30, 40, 50]:
    PRINT x
END

# Over a range
FOR i IN range(0, 5):
    PRINT i
END

# Over a range with step
FOR i IN range(3, 8):
    PRINT i
END

# Over a string (character-by-character)
FOR ch IN "hello":
    PRINT ch
END

# Accumulator with FOR
total = 0
FOR n IN range(1, 11):
    total = total + n
END
PRINT total   # 55

# Nested FOR
total = 0
FOR i IN range(1, 7):
    FOR j IN range(1, 7):
        total = total + 1
    END
END
PRINT total   # 36
```

### 4.9 Functions — DEF / DO

Functions are defined with `DEF` and called with `DO`. Arguments are positional and space-separated at the call site.

```pasta
# No-argument function
DEF greet:
    PRINT "hello"
END
DO greet    # hello

# Function with arguments
DEF add a b:
    RET.NOW a + b
END
result = DO add 3 4
PRINT result    # 7

# Multi-argument, expression-rich
DEF multiply a b:
    RET.NOW a * b
END
PRINT DO multiply 6 7    # 42

# Multi-target DO
DEF worker name:
    PRINT name
END
DO worker "Alice"
DO worker "Bob"
DO worker "Carol"
```

**Scope isolation:** Functions execute in their own scope. Outer variables are readable; mutation inside a function does not affect outer bindings unless re-assigned at the outer scope.

```pasta
x = 999
DEF peek:
    PRINT x    # reads outer x
END
DO peek     # 999
PRINT x     # 999 — unchanged
```

**Global mutation pattern:**

```pasta
counter = 0
DEF increment:
    counter = counter + 1
END
DO increment
DO increment
DO increment
PRINT counter   # 3
```

### 4.10 Lambda Expressions

Anonymous functions are first-class values. They can be assigned, passed as arguments, and returned from functions.

```pasta
# Basic lambda
square = LAMBDA x: x * x END
PRINT square(6)      # 36

multiply = LAMBDA a b: a * b END
PRINT multiply(7, 6)    # 42

# Higher-order: pass lambda as argument
apply = LAMBDA f x: f(x) END
PRINT apply(square, 5)   # 25

# Immediately applied
PRINT (LAMBDA x: x + 1 END)(99)   # 100
```

### 4.11 Return Semantics — RET.NOW / RET.LATE

PASTA provides two distinct return keywords with different execution semantics.

**RET.NOW** — immediate return. Exits the function body at that exact point and returns the given value.

```pasta
DEF sign x:
    IF x < 0:
        RET.NOW "negative"
    END
    IF x == 0:
        RET.NOW "zero"
    END
    RET.NOW "positive"
END

PRINT DO sign -5    # negative
PRINT DO sign 0     # zero
PRINT DO sign 7     # positive
```

**RET.LATE** — deferred / pending return. Sets the return value but continues executing the rest of the function body. The deferred value is returned when the body fully exits.

```pasta
DEF deferred_example:
    RET.LATE 42
    PRINT "body continued after RET.LATE"
    # function exits here and returns 42
END

result = DO deferred_example
# prints: body continued after RET.LATE
PRINT result    # 42
```

**Recursion** works naturally with both return forms:

```pasta
DEF factorial n:
    IF n <= 1:
        RET.NOW 1
    END
    RET.NOW n * (DO factorial n - 1)
END

PRINT DO factorial 1     # 1
PRINT DO factorial 5     # 120
PRINT DO factorial 10    # 3628800

DEF fibonacci n:
    IF n <= 0: RET.NOW 0 END
    IF n == 1: RET.NOW 1 END
    RET.NOW (DO fibonacci n - 1) + (DO fibonacci n - 2)
END

PRINT DO fibonacci 7    # 13
PRINT DO fibonacci 10   # 55
```

### 4.12 Error Handling — ATTEMPT

`ATTEMPT` / `OTHERWISE` provides structured error handling. The `OTHERWISE` branch executes if the `ATTEMPT` body raises a runtime error; otherwise the `ATTEMPT` result is used.

```pasta
# Basic error catch
ATTEMPT:
    x = 1 / 0
OTHERWISE:
    PRINT "caught an error"
END

# Return a default value on error
result = ATTEMPT:
    risky_operation()
OTHERWISE:
    42
END
PRINT result    # 42 if risky_operation fails

# From test suite (section 48)
PRINT ATTEMPT: 2 OTHERWISE: 0 END        # 2
PRINT ATTEMPT: 1/0 OTHERWISE: 42 END     # 42
```

### 4.13 Priority Declarations

`PRIORITY` attaches execution-priority metadata to a block. This is consumed by the semantics engine (`semantics/priority.rs`) and passed to the MRA scheduler subsystem.

```pasta
PRIORITY high:
    DO critical_task
END

PRIORITY low:
    DO background_task
END

PRINT "priorities set"
```

Standard priority levels: `high`, `normal`, `low`. Custom strings are accepted and forwarded to the scheduler.

---

## 5. Keywords Reference

### Control & Block Keywords

| Keyword      | Category   | Description                                              |
|--------------|------------|----------------------------------------------------------|
| `PRINT`      | I/O        | Print a value or expression to stdout                    |
| `IF`         | Control    | Conditional branch — opened by `:`, closed by `END`      |
| `OTHERWISE`  | Control    | Else clause of `IF` or `ATTEMPT`                         |
| `WHILE`      | Loop       | Condition-checked loop                                   |
| `DO WHILE`   | Loop       | Alternate WHILE form (identical semantics)               |
| `FOR`        | Loop       | Iteration loop                                           |
| `IN`         | Loop       | Separates loop variable from iterable in `FOR`           |
| `END`        | Block      | Closes any open block: IF, WHILE, FOR, DEF, LAMBDA       |
| `DEF`        | Function   | Define a named function                                  |
| `DO`         | Function   | Call a named function (also prefix for `DO WHILE`)       |
| `LAMBDA`     | Function   | Anonymous function expression                            |
| `RET.NOW`    | Return     | Immediate return — exits function body immediately       |
| `RET.LATE`   | Return     | Deferred return — body continues, value returned at exit |
| `ATTEMPT`    | Error      | Opens a try/catch block                                  |
| `PRIORITY`   | Scheduler  | Attach priority metadata to a block                      |
| `NOT`        | Boolean    | Logical negation                                         |

### Operator Tokens

| Symbol  | Category   | Description                        |
|---------|------------|------------------------------------|
| `+`     | Arithmetic | Addition                           |
| `-`     | Arithmetic | Subtraction / unary negation       |
| `*`     | Arithmetic | Multiplication                     |
| `/`     | Arithmetic | Division (float result)            |
| `%`     | Arithmetic | Modulo                             |
| `^`     | Arithmetic | Exponentiation                     |
| `==`    | Comparison | Equal                              |
| `!=`    | Comparison | Not equal                          |
| `>`     | Comparison | Greater than                       |
| `<`     | Comparison | Less than                          |
| `>=`    | Comparison | Greater or equal                   |
| `<=`    | Comparison | Less or equal                      |
| `&&`    | Boolean    | Logical AND                        |
| `\|\|`  | Boolean    | Logical OR                         |
| `=`     | Assignment | Variable assignment / rebinding    |
| `:`     | Block      | Block opener (end of header line)  |
| `#`     | Comment    | Line comment                       |

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
| `pow(x, y)`                     | Power (also via `^` operator)      |
| `min(a, b)`                     | Minimum of two values              |
| `max(a, b)`                     | Maximum of two values              |
| `log(x)`                        | Natural logarithm                  |
| `log2(x)`                       | Base-2 logarithm                   |
| `sin(x)` / `cos(x)` / `tan(x)` | Trigonometric functions            |
| `sign(x)`                       | Returns -1, 0, or 1                |

### String

| Function                    | Description                                 |
|-----------------------------|---------------------------------------------|
| `len(s)`                    | String or list length                       |
| `upper(s)`                  | Uppercase                                   |
| `lower(s)`                  | Lowercase                                   |
| `concat(a, b)`              | Concatenate two strings                     |
| `trim(s)`                   | Strip leading/trailing whitespace           |
| `starts_with(s, prefix)`    | Boolean prefix check                        |
| `ends_with(s, suffix)`      | Boolean suffix check                        |
| `replace(s, old, new)`      | Replace all occurrences                     |
| `split(s, delim)`           | Split into list by delimiter                |
| `substr(s, start, end)`     | Extract substring by index range            |
| `contains(s, sub)`          | Boolean containment check                   |
| `char_at(s, i)`             | Character at index                          |
| `to_string(x)`              | Convert any value to string representation  |

### Type Conversion

| Function       | Description                                |
|----------------|--------------------------------------------|
| `int(x)`       | Convert to integer (truncates float)       |
| `float(x)`     | Convert to float                           |
| `bool(x)`      | Convert to boolean                         |
| `to_string(x)` | Convert to string                          |
| `type_of(x)`   | Returns type name: `"number"`, `"string"`, `"bool"`, `"heap"` |

```pasta
PRINT int(3.9)       # 3
PRINT int("-2")      # -2
PRINT float("3.14")  # 3.14
PRINT bool(0)        # false
PRINT bool(1)        # true
PRINT to_string(100) # "100"
PRINT type_of(42)    # number
PRINT type_of("hi")  # string
PRINT type_of([])    # heap
PRINT type_of(true)  # bool
```

### List

| Function                  | Description                              |
|---------------------------|------------------------------------------|
| `list_len(lst)`           | Length                                   |
| `list_sum(lst)`           | Sum of all numeric elements              |
| `list_min(lst)`           | Minimum element                          |
| `list_max(lst)`           | Maximum element                          |
| `list_avg(lst)`           | Average of all elements                  |
| `list_sort(lst)`          | Sort ascending — returns new list        |
| `list_rev(lst)`           | Reverse — returns new list               |
| `list_slice(lst, s, e)`   | Slice from s to e (exclusive end)        |
| `list_push(lst, x)`       | Append element, return new list          |
| `list_pop(lst)`           | Remove and return last element           |
| `list_first(lst)`         | First element                            |
| `list_last(lst)`          | Last element                             |
| `list_concat(a, b)`       | Concatenate two lists                    |
| `list_contains(lst, x)`   | Boolean element presence check           |
| `list_flatten(lst)`       | Flatten one level of nesting             |
| `range(s, e)`             | List `[s, s+1, ..., e-1]`               |
| `range(s, e, step)`       | List with step (positive or negative)    |

---

## 7. Standard Library Modules

Stdlib modules use dotted-namespace dispatch through `call_builtin`. The lexer's `alias.rs` absorbs dots into identifier tokens (`sys.env` becomes a single `Identifier` token), enabling seamless dispatch without a separate member-access operator in the grammar.

All modules ship as `.ph` header files in `src/stdlib/` with executor dispatch blocks wired in `executor.rs`.

### `sys.*`

| Function          | Description                           |
|-------------------|---------------------------------------|
| `sys.env(key)`    | Read environment variable             |
| `sys.argv()`      | Command-line arguments as list        |
| `sys.exit(code)`  | Exit interpreter with given code      |
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

### `net.*`

| Function               | Description                         |
|------------------------|-------------------------------------|
| `net.get(url)`         | HTTP GET, returns body string        |
| `net.post(url, data)`  | HTTP POST                            |
| `net.resolve(host)`    | DNS resolution                       |

### `gc.*`

| Function       | Description                           |
|----------------|---------------------------------------|
| `gc.collect()` | Trigger a GC cycle                    |
| `gc.stats()`   | Return GC statistics as string        |

### `debug.*`

| Function              | Description                          |
|-----------------------|--------------------------------------|
| `debug.trace(x)`      | Print trace info for value x         |
| `debug.dump_env()`    | Dump current environment bindings    |
| `debug.assert(cond)`  | Assert condition, raise on fail      |

### `ffi.*`

| Function                 | Description                          |
|--------------------------|--------------------------------------|
| `ffi.load(lib)`          | Load a shared library                |
| `ffi.call(sym, ...args)` | Call a foreign symbol                |

### `thread.*`

| Function              | Description                          |
|-----------------------|--------------------------------------|
| `thread.spawn(fn)`    | Spawn a new thread                   |
| `thread.join(handle)` | Wait for thread completion           |
| `thread.sleep(ms)`    | Sleep current thread (milliseconds)  |
| `thread.id()`         | Return current thread ID             |

### `device.*`

| Function            | Description                           |
|---------------------|---------------------------------------|
| `device.arch()`     | Host architecture string              |
| `device.cores()`    | Logical CPU core count                |
| `device.memory()`   | Total system memory in bytes          |
| `device.profile()`  | Auto-detected device profile name     |

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

### `memory.*`

| Function               | Description                           |
|------------------------|---------------------------------------|
| `memory.alloc(n)`      | Allocate N bytes                      |
| `memory.free(ptr)`     | Free allocation                       |
| `memory.read(ptr, n)`  | Read N bytes from address             |
| `memory.write(ptr, d)` | Write data to address                 |

---

## 8. Type System

PASTA uses a unified `Value` enum defined in `src/typing/types.rs`. The typing module provides configurable numeric promotion, rounding, and coercion.

### Value Variants

| Variant  | Rust backing  | Description                    |
|----------|---------------|--------------------------------|
| `Int`    | `i64`         | Integer number                 |
| `Float`  | `f64`         | Floating-point number          |
| `Bool`   | `bool`        | Boolean true/false             |
| `Str`    | `String`      | UTF-8 string                   |
| `List`   | `Vec<Value>`  | Heap-allocated list            |
| `Tensor` | internal      | N-dimensional tensor           |
| `Fn`     | `FnDef`       | Named function reference       |
| `Lambda` | `LambdaDef`   | Anonymous function             |
| `Null`   | —             | Absent / unassigned value      |

### Coercion Engine

- `DefaultCoercion` carries a `CoercionConfig` controlling numeric promotion and rounding behavior.
- `StandardExecutor` attempts to downcast the engine to `DefaultCoercion` to read its config; other engines fall back to global float helpers in `float.rs`.
- `compute_numeric_op` in `operands.rs` centralizes all arithmetic dispatch.
- `apply_round_and_downcast` handles post-operation rounding according to the configured level.
- `division_always_float` config option forces float results even from integer operands.

### Rounding Levels

| Level | Behavior                              |
|-------|---------------------------------------|
| 1     | No rounding — full f64 precision      |
| 2     | Round to 2 decimal places (default)   |
| 3     | Round to 4 decimal places             |
| 4     | Round to 6 decimal places             |
| 5     | Round to 10 decimal places            |

### `type_of()` Return Values

| PASTA type | `type_of()` returns |
|------------|---------------------|
| Integer    | `"number"`          |
| Float      | `"number"`          |
| Bool       | `"bool"`            |
| String     | `"string"`          |
| List       | `"heap"`            |

---

## 9. Shell / OS Layer

The `shell_os` subsystem (`src/interpreter/shell_os/`) integrates shell-like OS primitives directly into the PASTA interpreter. It was merged from the standalone `shell_OS` project and adapted to accept PASTA `Environment`/`Executor` types.

### Virtual Filesystem (VFS)

Located in `shell_os/vfs/`. Provides an in-memory virtual filesystem with persistent backing via `DiskImages/fs.img`.

| Component | File       | Description                                           |
|-----------|------------|-------------------------------------------------------|
| `fs.rs`   | VFS driver | Core filesystem operations (read, write, mkdir, etc.) |
| `node.rs` | VFS node   | Inode-like node type for files and directories        |
| `path.rs` | Path utils | PASTA-flavored path handling and normalization        |

### Shell Commands

`shell_os/commands/fs_commands.rs` provides filesystem shell commands invocable from the interpreter. The `shell.rs` shim connects the `shell_os` subsystem to the main executor.

### CLI Integration

`shell_os/cli/cli.rs` provides the CLI for shell-mode operation. The shell entrypoint wraps PASTA `Environment`/`Executor` types so that shell commands and script execution share a unified environment.

---

## 10. Async Runtime — pasta_async

`src/pasta_async/` is a self-contained sub-crate (`pasta_async/Cargo.toml`) providing async I/O and concurrency primitives for PASTA.

| Module         | Description                                       |
|----------------|---------------------------------------------------|
| `api.rs`       | Public async API surface                          |
| `io.rs`        | Async I/O operations                              |
| `runtime.rs`   | Async runtime event loop                          |
| `serialize.rs` | Async-safe value serialization                    |
| `sync.rs`      | Synchronization primitives (mutex, channel stubs) |
| `testing.rs`   | Async test helpers                                |

The async runtime is integrated into `src/lib.rs` and available to the executor for non-blocking operations. The sub-crate was reorganized from a root-level module to `src/pasta_async/` as part of the v1.4 directory restructure.

---

## 11. AI / ML Operations

The `src/ai/` module provides a native AI/ML subsystem accessible from PASTA scripts via the `tensor.*` stdlib namespace and the `ai_network.rs` interpreter hook.

| Module        | Description                                         |
|---------------|-----------------------------------------------------|
| `tensor.rs`   | Core N-dimensional tensor type                      |
| `autograd.rs` | Reverse-mode automatic differentiation              |
| `models.rs`   | Model definition and parameter management           |
| `learn.rs`    | Training loops, loss functions, optimizer stubs     |
| `datasets.rs` | Dataset loading and batching                        |
| `generate.rs` | Text and tensor generation utilities                |
| `tokenizer.rs`| Tokenization pipeline                               |

PASTA scripts interact with the AI subsystem through `tensor.*` calls:

```pasta
t = tensor.zeros([3, 3])
r = tensor.rand([2, 4])
PRINT tensor.shape(t)          # [3, 3]
PRINT tensor.shape(r)          # [2, 4]
result = tensor.matmul(t, r)
```

---

## 12. Meatball Runtime Architecture (MRA)

The Meatball Runtime Architecture (MRA) is PASTA's scaffold for agent-based and multi-backend execution. It lives in `src/meatballs/`.

### Components

| Component   | Path               | Description                                              |
|-------------|--------------------|----------------------------------------------------------|
| `api/`      | `meatball_api.rs`  | Rust API surface — public interface for all backends     |
| `agent/`    | `agent.rs`         | Agent binary running inside Meatballs (JSON-over-stdio)  |
| `runtime/`  | `runtime.rs`       | Runtime hooks and scheduler stubs                        |
| `backends/` | (stubs)            | Backend implementations: `local`, `pseudo-vm`, `vm`     |
| `phase0/`   | `mra_schema.json`  | Phase 0 design schema for MRA protocol                   |
| `phase0/`   | `objective.md.txt` | Phase 0 objectives document                              |
| `kernel/`   | (artifacts)        | Optional kernel images and build artifacts               |

### MRA Protocol

The agent binary communicates via **JSON-over-stdio**. The `meatball_api.rs` defines the Rust API surface that frontends use to dispatch work to backends. The `phase0/mra_schema.json` defines the protocol message schema.

### Disk Images

`DiskImages/fs.img` provides a virtual disk image used by the VFS and MRA backends for persistent storage.

---

## 13. REPL & CLI

### REPL

Launched by running `./target/release/pasta` with no arguments.

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

The REPL uses the raw-mode `readline` module for cursor navigation and history. All PASTA language features are available interactively including multi-line blocks.

### Script Mode

```bash
./target/release/pasta <script.ps>
```

### lib.rs API

The public API exposed via `src/lib.rs`:

```rust
pub use interpreter::{Executor, Environment, Value, ThreadMeta};
pub use parser::Parser;
pub use runtime::{auto_configure, detect_host_arch};
pub use runtime::asm::{AsmRuntime, AsmBlock};

// Convenience initializer
pub fn init_executor_with_auto_config() -> Executor
```

---

## 14. readline Module

`src/readline.rs` implements a production-quality raw-mode line editor for the PASTA REPL on Unix/Linux.

### Key Bindings

| Binding          | Action                                       |
|------------------|----------------------------------------------|
| `↑` / `↓`        | Scroll 50-entry history ring (up/down)       |
| `←` / `→`        | Move cursor left / right within line         |
| `Home` / `Ctrl-A`| Jump to start of line                        |
| `End` / `Ctrl-E` | Jump to end of line                          |
| `Backspace`      | Delete character before cursor               |
| `Delete` (ESC[3~)| Delete character at cursor                   |
| `Ctrl-K`         | Kill (delete) from cursor to end of line     |
| `Ctrl-U`         | Kill from start of line to cursor            |
| `Ctrl-C`         | Clear current line                           |
| `Ctrl-D`         | EOF / exit (on empty line only)              |
| `Enter`          | Submit current line                          |

### History API

```rust
pub fn read_line_with_history(prompt: &str) -> io::Result<Option<String>>
pub fn history_push(line: &str)
pub fn history_get() -> Vec<String>
```

- Returns `None` on EOF (Ctrl-D on empty line)
- History ring: max 50 entries; oldest discarded on overflow
- Consecutive identical entries are deduplicated
- Falls back cleanly to `stdin.read_line` on non-TTY (pipes, redirection)
- History persists within a session; cross-session persistence is planned for v1.5

---

## 15. Architecture Overview

```
┌─────────────────────────────────────────────┐
│            pasta.rs  (binary entry)          │
│         REPL loop  /  script loader          │
└─────────────────┬───────────────────────────┘
                  │
      ┌───────────▼────────────────────┐
      │         lib.rs (crate root)     │
      │  init_executor_with_auto_config │
      │  global VERBOSE_FLAG / DEBUG    │
      └──────┬───────────┬─────────────┘
             │           │
    ┌─────────▼──┐  ┌────▼──────────────────┐
    │   Lexer     │  │       Parser           │
    │  alias.rs   │  │  ast.rs / parser.rs    │
    │  tokens.rs  │  │  grammar.rs            │
    └─────────────┘  └────┬──────────────────┘
                          │  AST
          ┌───────────────▼────────────────────┐
          │           Semantics                 │
          │  resolver.rs / constraints.rs       │
          │  priority.rs                        │
          └───────────────┬────────────────────┘
                          │
          ┌───────────────▼────────────────────┐
          │           Executor                  │
          │  executor.rs (dispatch loop)        │
          │  environment.rs (scope stack)       │
          │  errors.rs                          │
          └───┬───────────┬────────────┬────────┘
              │           │            │
  ┌───────────▼─┐ ┌───────▼──┐ ┌──────▼──────────────┐
  │   stdlib     │ │  typing  │ │      runtime          │
  │  .ph / .pa   │ │  types   │ │  asm / devices / rng  │
  │  namespaces  │ │  coerce  │ │  threading / scheduler│
  └─────────────┘ └──────────┘ └──────────────────────┘
              │
  ┌───────────▼─────────────────────────────────────┐
  │   shell_os  │  pasta_async  │  ai  │  meatballs  │
  └─────────────────────────────────────────────────┘
```

### Global Flags (from `lib.rs`)

| Symbol          | Type        | Default | Description                          |
|-----------------|-------------|---------|--------------------------------------|
| `VERBOSE_FLAG`  | `AtomicBool`| `false` | Enable general verbose output        |
| `VERBOSE_DEBUG` | `AtomicBool`| `false` | Enable detailed interpreter traces   |

Both are `#[no_mangle]` for FFI visibility from external tools.

---

## 16. Typing System Internals

The typing module (`src/typing/`) centralizes all numeric promotion, rounding, and coercion logic.

### Key Files

| File               | Role                                                            |
|--------------------|-----------------------------------------------------------------|
| `types.rs`         | Core `Value` enum — single source of truth for all types       |
| `operands.rs`      | `compute_numeric_op` and `apply_round_and_downcast`            |
| `float.rs`         | Rounding and display formatting helpers                         |
| `util.rs`          | Promotion helper and engine-config extraction                   |
| `bool_coerce.rs`   | Bool coercion rules (truthy/falsy evaluation)                   |
| `string_coerce.rs` | String-to-type and type-to-string coercion                      |
| `tensor_type.rs`   | Tensor type descriptor and shape handling                       |

### Coercion Notes

- `DefaultCoercion` carries `CoercionConfig`; `StandardExecutor` downcasts to read it.
- Other coercion engines fall back to global float helpers in `float.rs`.
- `string::to_string` uses a global display level (2) for float formatting — per-engine wiring is planned for v1.6.
- `once_cell` is an unconditional dependency for lazy global initialization.

### Lexer Dot-Absorption

`alias.rs` absorbs dots into identifier tokens so `sys.env` lexes as a single `Identifier` token. This enables the complete dotted-namespace dispatch through `call_builtin` without a separate member-access operator in the grammar — keeping the language syntax clean.

---

## 17. Test Suite

PASTA ships a comprehensive regression suite covering all core language features, algorithms, and edge cases.

### Suite Overview

| File                          | Sections | Status           | Coverage                                   |
|-------------------------------|----------|------------------|--------------------------------------------|
| `10_full_suite.ps`            | 50       | ✅ All pass      | Full language, stdlib, algorithms          |
| `09_big_test.ps`              | 30       | ✅ All pass      | Core features, RET.NOW/LATE, priority      |
| `06_functions_and_lambdas.ps` | —        | ✅ Passing       | DEF, DO, LAMBDA, closures                  |
| `08_test_RET.ps`              | —        | ✅ Passing       | Return semantics                           |
| `07_do_multi_alias_repeat.ps` | —        | ✅ Passing       | Multi-target DO, aliasing                  |
| `mand_test.ps`                | —        | ✅ Passing       | Mandelbrot set stress test                 |

### `10_full_suite.ps` — 50 Sections at a Glance

| Sections | Topics Covered                                                                      |
|----------|-------------------------------------------------------------------------------------|
| 1–6      | Literals, Arithmetic, Variables, Strings, Boolean Logic, Comparisons               |
| 7–9      | IF/OTHERWISE, WHILE, Nested WHILE                                                   |
| 10–13    | FOR IN list, FOR IN range, FOR IN string, Nested FOR                               |
| 14–16    | Lists, List builtins, Math builtins                                                 |
| 17       | Type conversion (`int`, `float`, `bool`, `to_string`, `type_of`)                  |
| 18–20    | DEF basic, Scope isolation, Global mutation                                         |
| 21–26    | RET.NOW basic/early-exit, Recursion, RET.NOW+WHILE, RET.NOW+FOR, Cross-function    |
| 27–29    | Lambdas, DO multi-target, Nested functions                                          |
| 30–32    | FOR accumulator, String+FOR, range()                                                |
| 33–35    | Modulo, Recursive+range, WHILE+RET.NOW search                                      |
| 36–41    | Collatz, Fibonacci iterative, GCD/LCM, Exponentiation, String reversal, Palindrome |
| 42–45    | Index-based iteration, Multiplication table, FizzBuzz (1–15), Primes count         |
| 46–50    | Functions as values, RET.LATE, ATTEMPT, Priority, Full pipeline                    |

### Running Tests

```bash
./target/release/pasta tests/10_full_suite.ps
# === ALL 50 TESTS COMPLETE ===

./target/release/pasta tests/09_big_test.ps
# === ALL TESTS COMPLETE ===
```

---

## 18. Configuration & Build

### Requirements

- Rust stable toolchain (latest stable recommended)
- Arch Linux (primary target; other Linux distros supported)
- `libc` crate — required for raw-mode terminal on Unix
- `once_cell = "1.18"` — unconditional dependency for lazy globals

### Cargo Features

| Feature      | Description                                             |
|--------------|---------------------------------------------------------|
| `scheduler`  | Enable `Scheduler` type from `runtime/scheduler.rs`    |
| `canvas_png` | PNG canvas support via `dep:image` crate               |

> **Important:** The image feature must be named `canvas_png` — not `image` — to avoid a name clash with the `image` crate dependency in `Cargo.toml`.

### Build Commands

```bash
# Release build (recommended)
cargo build --release

# Debug build
cargo build

# With scheduler feature
cargo build --release --features scheduler
```

### Binary Location

```
./target/release/pasta
```

### Diagnostics Flags

| Global Symbol   | Default | Effect when `true`                       |
|-----------------|---------|------------------------------------------|
| `VERBOSE_FLAG`  | `false` | Enable general verbose runtime output    |
| `VERBOSE_DEBUG` | `false` | Enable detailed interpreter trace logs   |

Both are `AtomicBool` and can be set at startup or toggled at runtime.

---

## 19. Changelog

### v1.4 — Current Release

#### New Language Features

- **`FOR IN` loop** — iterate over lists, ranges, and strings (all three iterable types verified across test sections 10–13, 25, 31, 34).
- **`DO WHILE` form** — alternate `WHILE` syntax now fully supported alongside standard `WHILE`.
- **`ATTEMPT` / `OTHERWISE` error handling** — structured try/catch verified in test section 48.
- **`PRIORITY` declarations** — semantic pass wired to scheduler stubs; verified in section 49.
- **`RET.LATE` deferred return** — body-after-return execution confirmed correct in sections 27 and 47.
- **Negative indexing** — `s[-1]` and `lst[-1]` now supported for both strings and lists.
- **`range()` with negative step** — `range(10, 1, -3)` → `[10, 7, 4, 1]` verified.
- **Functions as first-class values** — lambdas assignable and passable, verified in section 46.

#### Infrastructure & Runtime

- **Full 50-section test suite passing.** `10_full_suite.ps` — all 50 sections complete cleanly including FizzBuzz, Primes, Collatz, Palindrome, GCD/LCM, Fibonacci iterative, and full pipeline.
- **`readline.rs` — production raw-mode line editor added.** Full cursor navigation (Home/End/Left/Right), 50-entry history ring, kill/yank (Ctrl-K/U), Delete-at-cursor (ESC[3~), Ctrl-C/D. Falls back to `stdin.read_line` on non-TTY.
- **`src/lib.rs` crate root established.** All `use pasta::` references in `src/bin/pasta.rs` now resolve. Public API: `Executor`, `Environment`, `Value`, `ThreadMeta`, `Parser`, `AsmRuntime`, `AsmBlock`, `auto_configure`, `detect_host_arch`.
- **`init_executor_with_auto_config()` function added.** Constructs an `Executor`, runs device auto-config, records diagnostics.
- **stdlib `.ph` headers promoted to real executable PASTA source.** All major namespaces (`sys.*`, `time.*`, `rand.*`, `gc.*`, `debug.*`, `fs.*`, `net.*`, `ffi.*`, `thread.*`, `device.*`, `tensor.*`, `memory.*`) have executor dispatch blocks wired in `executor.rs`.
- **Typing module finalized.** `util.rs`, `operands.rs`, `float.rs`, `bool_coerce.rs`, `string_coerce.rs` all complete. Numeric promotion, rounding levels 1–5, and `division_always_float` config in place.
- **`pasta_async` sub-crate integrated.** Moved to `src/pasta_async/` with its own `Cargo.toml`. Build errors from reorganization resolved.
- **`missing_docs` warnings systematically silenced** across `lexer`, `parser`, `semantics`, `interpreter`, and `stdlib` layers.
- **Directory structure reorganized and backups cleaned up.**

#### Bug Fixes

- **Parser `DEF` body `WHILE` bug** — `DEF` bodies containing `WHILE...END` blocks were silently dropping subsequent statements. Fixed with an `End`-skip guard in the parser's `DEF` body loop to prevent premature `END` token consumption when `DEDENT` is emitted before `END` after nested blocks. This resolved regressions in test sections 25–30.
- **`ParseError::new`** — span-first argument ordering enforced throughout the parser.
- **`env.set_local()`** — corrected from incorrect `env.set()` call sites.
- **`TraceFrame.context`** — corrected from erroneous `.label` field name.
- **`once_cell`** — promoted to unconditional dependency in `Cargo.toml`.
- **`canvas_png` feature rename** — eliminated `Cargo.toml` name clash with the `image` crate.

---

### v1.3

- Initial `FOR IN` loop skeleton (list, range, string iteration)
- `ATTEMPT` / `OTHERWISE` error handling skeleton
- `PRIORITY` keyword and semantic pass
- `RET.LATE` deferred return keyword
- Lambda first-class values (`LAMBDA...END`)
- Shell_OS integration (`shell_os/` subsystem merged into interpreter)
- VFS introduced (`shell_os/vfs/` — `fs.rs`, `node.rs`, `path.rs`)
- `pasta_async` sub-crate scaffold
- MRA skeleton (`meatballs/`) — agent, api, backends, runtime stubs
- AI/ML subsystem scaffold (`src/ai/`)
- Typing system refactor — `DefaultCoercion`, `CoercionConfig`, rounding levels
- `alias.rs` dot-absorption for dotted-namespace dispatch
- `mand_test.ps` Mandelbrot stress test added

### v1.2

- `RET.NOW` early-return keyword
- Recursive function support
- `range()` built-in with optional step
- String negative indexing
- Additional list built-ins: `list_sort`, `list_rev`, `list_slice`, `list_concat`, `list_contains`
- `type_of()` built-in
- `WHILE` and nested `WHILE` loops verified
- Scope isolation for `DEF` functions

### v1.1

- `DEF` / `DO` function definition and invocation
- Multi-argument functions
- Global variable mutation from inside functions
- `IF` / `OTHERWISE` conditional blocks
- Basic arithmetic operators: `+`, `-`, `*`, `/`, `%`, `^`
- String built-ins: `len`, `upper`, `lower`, `concat`, `trim`, `replace`, `split`
- List built-ins: `list_len`, `list_sum`, `list_min`, `list_max`, `list_avg`

### v1.0

- Initial PASTA interpreter in Rust
- Lexer, Parser, Executor scaffolded
- `PRINT` statement
- Integer, Float, Bool, String literals
- Variable assignment (`=`)
- Arithmetic expressions
- `IF` / `OTHERWISE` / `END` blocks

---

## 20. Roadmap / To-Do

### Immediate Priority (v1.5 targets)

- [ ] **`BREAK` and `CONTINUE` keywords.** Loop control flow is missing. `BREAK` exits a loop early; `CONTINUE` skips to the next iteration. Required by many common algorithm patterns and expected by users coming from any other language.
- [ ] **String interpolation.** A `"hello {name}"` or `f"..."` style syntax to eliminate repeated `concat()` chains. This is the single most friction-causing gap in daily scripting.
- [ ] **Dictionaries / Maps.** A `{key: value}` literal type with associated builtins: `dict.get`, `dict.set`, `dict.keys`, `dict.values`, `dict.contains`, `dict.remove`, `dict.len`. General-purpose scripting requires a map type.
- [ ] **`IMPORT` / module system.** Allow `.ps` scripts to import other `.ps` files or stdlib modules explicitly: `IMPORT "utils.ps"` or `IMPORT math`. Currently all stdlib is auto-loaded; explicit imports enable code splitting and reuse.
- [ ] **REPL history persistence.** Save and restore readline history across sessions by writing to `~/.pasta_history` on exit and loading on startup.
- [ ] **`PRINT` formatting.** Add `PRINT fmt "pattern"` or f-string syntax for formatted output — field widths, alignment, float precision — without requiring manual `concat` + `to_string`.
- [ ] **`FOR IN` with index (`enumerate`).** `FOR i idx IN list` or an `enumerate(lst)` builtin to expose the loop index alongside the value without a manual counter variable.

### Medium-Term (v1.6)

- [ ] **True lexical closure capture.** Lambdas currently close over the outer environment at call time. Implement proper capture-at-definition semantics so closures hold live references to closed-over variables.
- [ ] **Typed exceptions in `ATTEMPT`.** Allow `CATCH TypeError`, `CATCH IOError`, etc. for discriminated error handling: `ATTEMPT ... CATCH TypeError: ... CATCH IOError: ... END`.
- [ ] **`MATCH` / pattern matching.** A `MATCH value: CASE x: ... END` construct for exhaustive branching on values and types.
- [ ] **Tail-call optimization (TCO).** Deep recursion currently builds full Rust call stacks. TCO would allow unbounded recursion depth for tail-recursive patterns.
- [ ] **CLI flags.** Implement `--verbose`, `--debug`, `--version`, `--eval <expr>`, and `--check` (syntax-check only) via `clap` or `argh`. Current CLI accepts only a script path.
- [ ] **Per-engine float display level.** `string::to_string` uses a global display level; wire per-engine display level through `CoercionConfig` so different executor instances can have different float formatting.
- [ ] **`stdio.*` namespace.** `stdio.read_line()`, `stdio.read_all()`, `stdio.write(s)` for interactive and piped I/O within scripts.
- [ ] **Multi-line REPL.** The REPL currently executes one logical line at a time. Add continuation detection (open block after `:`) so the user can type multi-line `DEF` and `IF` blocks interactively.

### Long-Term / Architecture

- [ ] **MRA backends — implement `local` and `pseudo-vm`.** `meatballs/backends/` contains stubs. The `local` backend (run agent in-process) and `pseudo-vm` (sandboxed subprocess) need full implementations, plus the dispatch logic in `meatball_api.rs`.
- [ ] **MRA agent protocol finalization.** Complete the JSON-over-stdio protocol schema (`phase0/mra_schema.json`) and implement the full agent dispatch loop in `agent.rs` including handshake, capability negotiation, and error propagation.
- [ ] **Bytecode compiler.** Compile the AST to a compact bytecode format for faster repeated execution, AOT distribution, and as a foundation for a future JIT.
- [ ] **Garbage collector.** Replace drop-based reclamation with a proper tracing GC. Expose `gc.collect()` and `gc.stats()` to scripts (both are currently stubs in `gc.ph`).
- [ ] **Native async — `ASYNC DEF` / `AWAIT`.** Surface `pasta_async` primitives as first-class language keywords so async I/O can be written naturally inside PASTA scripts.
- [ ] **`device.*` auto-configure expansion.** Add more device profiles to `device_ls.json`; implement GPU/NPU detection for automatic tensor dispatch routing.
- [ ] **AI model training pipeline.** Wire `learn.rs` training loops to the `tensor.*` stdlib so a complete ML training run can be scripted entirely in PASTA.
- [ ] **LSP / language server.** A Language Server Protocol implementation for editor integration: autocomplete, go-to-definition, hover docs, inline diagnostics.
- [ ] **Windows / macOS support.** The readline module and `shell_os` layer are Unix-specific. Platform abstractions are needed for cross-platform builds.
- [ ] **`saucey` subsystem.** `src/saucey/saucey.rs` is a placeholder. Define and implement its role (suspected: data pipeline or domain-specific layer) and integrate it with the executor.
- [ ] **Comprehensive `#[doc]` coverage.** Continue the systematic elimination of `missing_docs` warnings across all remaining public API surface.
- [ ] **Expanded test suite.** Add dedicated `.ps` test files for: stdlib namespace coverage, async operations, VFS operations, MRA dispatch, AI/tensor operations, error handling edge cases, and the new dict/map type once landed.

---

*PASTA v1.4 — Built with ❤️ in Rust*  
*Project root: `/home/travis/pasta` · Platform: Arch Linux*
