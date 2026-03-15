================================================================================
PASTALANG - UNIFIED COMPREHENSIVE REFERENCE
================================================================================
Version: 1.2 (March 7, 2026)
PASTA: Program for Assignment, Statements, Threading, and Allocation

PASTA is a domain-specific language built in Rust for describing concurrent
threads, priority relationships, constraint expressions, tensor/AI operations,
general scripting, and deferred/async-style return values via RET.NOW/RET.LATE.

================================================================================
TABLE OF CONTENTS
================================================================================

 1.  Quick Start
 2.  Building & Installation
 3.  Project Structure
 4.  Core Language Features
 5.  Data Types
 6.  Lexical Grammar & Tokens
 7.  Keywords & Aliases (Complete)
 8.  Operators & Precedence
 9.  Statements & Syntax
10.  Expressions
11.  Scoping & Environment
12.  Built-in Functions (Complete Reference)
13.  File I/O Operations
14.  Shell & Filesystem Operations
15.  AI/ML & Tensor Operations
16.  Priority & Constraint System
17.  Standard Library (stdlib) Reference
18.  REPL & Interactive Mode
19.  CLI Usage
20.  Examples
21.  Testing & Debugging
22.  Performance & Limitations
23.  Troubleshooting
24.  Architecture Overview
25.  Version & Test Status
26.  TODO

================================================================================
1. QUICK START
================================================================================

Your first PASTA program (save as hello.pa):

    x = 42
    PRINT x
    PRINT "Hello, World!"

Build and run:

    cargo build --release
    ./target/release/pasta hello.pa

Run inline:

    ./target/release/pasta --eval 'x = 42\nPRINT x\n'

Interactive REPL:

    ./target/release/pasta

File extensions:
    .pa    PASTA source files (primary)
    .ps    PASTA source files (alternate)
    .ph    PASTA header files
    .pasta PASTA source files (alternate)

================================================================================
2. BUILDING & INSTALLATION
================================================================================

REQUIREMENTS:
    - Rust 1.56+ (Edition 2021)
    - Cargo
    - Standard development tools (gcc or clang)

BUILD STEPS:

    # Clone repository
    git clone <repo>
    cd pasta

    # Debug build
    cargo build

    # Release build (optimized, ~10x faster)
    cargo build --release

    # Run all tests
    cargo test

    # Run tests (library only, no integration)
    cargo test --lib

    # Run single test with output
    RUST_BACKTRACE=1 cargo test <testname> --lib -- --nocapture

OPTIONAL CARGO FEATURES:

    # Image support (for canvas/graphics saving)
    cargo build --features image

    # Advanced tensor support (ndarray backend)
    cargo build --features ndarray

    # Task scheduler support
    cargo build --features scheduler

MAINTENANCE:

    cargo fmt          # Format code
    cargo clippy       # Lint
    cargo build -v     # Verbose build output

IMPORTANT: Never use sudo cargo — causes permission issues in target/.
If target/ has permission errors:
    sudo chown -R $USER:$USER target
    rm -rf target && cargo build

================================================================================
3. PROJECT STRUCTURE
================================================================================

    src/
    ├── lib.rs                  Main library exports
    ├── interpreter/            Core interpreter
    │   ├── executor.rs         Statement execution, builtins, ControlFlow (~3000 lines)
    │   ├── environment.rs      Variable storage, scopes, Value enum
    │   ├── repl.rs             Interactive REPL mode
    │   ├── shell.rs            Integrated shell (ls, cd, mkdir, etc.)
    │   ├── shell_os/           OS-level shell adapter
    │   ├── ai_network.rs       Neural network backend
    │   ├── errors.rs           RuntimeError, Traceback types
    │   └── mod.rs              Module exports
    ├── lexer/                  Tokenization
    │   ├── lexer.rs            Main lexer with indent/dedent handling
    │   ├── tokens.rs           TokenType enum definitions
    │   ├── alias.rs            Keyword alias table (JSON-overridable)
    │   └── unicode.rs          Unicode math normalization (×→*, ÷→/, etc.)
    ├── parser/                 Syntax analysis
    │   ├── parser.rs           Main parser (precedence climbing)
    │   ├── ast.rs              AST node types (Statement, Expr, RetLateCondition)
    │   └── grammar.rs          EBNF grammar reference
    ├── runtime/                Runtime subsystems
    │   ├── devices.rs          Device detection & auto-configure
    │   ├── threading.rs        Thread metadata
    │   ├── scheduler.rs        Task scheduler (feature-gated)
    │   ├── rng.rs              Random number generation (hardware RNG preferred)
    │   ├── asm.rs              Sandboxed ASM block runtime
    │   ├── bitwise.rs          Bitwise operations
    │   └── strainer.rs         Garbage collector (mark-and-sweep)
    ├── semantics/              Semantic analysis
    │   ├── constraints.rs      Constraint engine
    │   ├── priority.rs         Priority graph (directed, cycle-detected)
    │   └── resolver.rs         Symbol resolution (scope-aware)
    └── utils/                  Utilities
        ├── errors.rs           Error types & diagnostics
        ├── logging.rs          Lightweight logging (pasta_info!, pasta_debug!, etc.)
        └── helpers.rs          Helper functions (now_millis, bytes_to_hex, etc.)

    stdlib.pa               Standard library (180+ functions, NOT auto-loaded)
    src/stdio.ph            Standard I/O header (auto-loaded)
    src/pasta_G.ph          Math/general header (auto-loaded)
    tests/                  Integration tests (.ps files)
    examples/               Example .pasta programs

================================================================================
4. CORE LANGUAGE FEATURES
================================================================================

IMPLEMENTED & WORKING (as of v1.2):
    ✓ Variable assignments with type inference
    ✓ Arithmetic operators (+, -, *, /, %)
    ✓ Logical operators (AND, OR, NOT)
    ✓ Comparison operators (==, !=, <, >, <=, >=)
    ✓ Unicode operators (≈, ≠, ≡ — token defined and wired)
    ✓ DO blocks (named threads with optional alias & repeat count)
    ✓ WHILE loops (all variants: named, lambda, multiple targets)
    ✓ Nested WHILE and WHILE+IF interaction
    ✓ IF/OTHERWISE conditional statements (fully working)
    ✓ Function definitions: DEF name(params): ... END
    ✓ Named parameters in DEF (positional arg binding at call site)
    ✓ Lambda expressions: lambda x: expr
    ✓ Lambda calls via environment lookup before builtins
    ✓ RET.NOW(): expr  — immediate return with control flow unwind
    ✓ RET.LATE(ms): expr  — snapshot-now, deliver-later (Value::Pending)
    ✓ resolve(handle)  — block until RET.LATE value is ready
    ✓ List creation, indexing (positive and negative), slicing
    ✓ String operations and character indexing
    ✓ Math builtins (abs, sqrt, pow, floor, ceil, round, min, max, clamp, sign)
    ✓ Conversion builtins (int, num, float, bool, str)
    ✓ List builtins (list_first, list_last, list_sum, list_average, list_reverse,
                     list_take, list_drop, list_slice, list_concat, list_flatten)
    ✓ String builtins (upper, lower, trim, starts_with, ends_with, replace, split)
    ✓ Random builtins (rand, rand_int, rand_range)
    ✓ System builtins (exit, sleep, time, env)
    ✓ Priority relationships (A OVER B)
    ✓ Constraint expressions (LIMIT OVER)
    ✓ Proper lexical scoping (scope stack, set_local/set_global)
    ✓ File I/O (read_from_file, write_to_file)
    ✓ Shell & filesystem operations (shell.* namespace)
    ✓ AI/ML tensor operations (tensor.* namespace)
    ✓ Neural network operations (ai.* namespace)
    ✓ Autograd engine (library level — not yet exposed as builtins)
    ✓ REPL (interactive mode with :env, :threads, :keywords, :shell, etc.)
    ✓ Integrated :shell (ls, cd, mkdir, rm, cp, mv)
    ✓ Header file loading (.ph files, auto-loaded at startup)
    ✓ Garbage collection (Strainer GC, runs after each top-level statement)
    ✓ Traceback / error diagnostics with span info
    ✓ type() returns "pending" for Value::Pending

PARTIALLY IMPLEMENTED:
    ~ TRY/OTHERWISE exception handling (parser wired, executor stub)
    ~ OBJ/SPAWN/MUT object family system (AST + data structures exist,
      execution not wired — Value::Object pending)
    ~ Autograd builtins (library exists, PASTA-level exposure pending)

RESERVED / NOT YET IMPLEMENTED:
    ✗ PAUSE/UNPAUSE/RESTART execution control
    ✗ WAIT/AWAIT condition waiting
    ✗ GROUP thread grouping
    ✗ CLASS type definitions
    ✗ LEARN ML macro keyword
    ✗ RET.LATE(trigger_fn()) polling (WhenTrue condition — placeholder)
    ✗ Value::Object(u64) for object instances as first-class values

================================================================================
5. DATA TYPES
================================================================================

PASTA has 9 value types (Value enum in environment.rs):

NUMBER:         42              Integer (stored as f64)
                3.14            Floating point
                -10.5           Negative

STRING:         "hello"         Double-quoted strings
                "PASTA rocks"

BOOL:           true            (aliases: True, TRUE)
                false           (aliases: False, FALSE)

LIST:           [1, 2, 3]       Homogeneous or mixed
                ["a", "b"]
                [1, "mixed", 3]

TENSOR:         tensor.zeros([2, 3])    2×3 zero matrix
                tensor.ones([5, 5])
                tensor.eye(4)           Identity matrix
                tensor.rand([3, 4])

LAMBDA:         fn = lambda x: x * 2   Anonymous function
                double = lambda x: x * 2
                result = double(6)      # 12

PENDING:        handle = my_fn()        Returned by RET.LATE
                result = resolve(handle) Blocks until ready
                type(handle)            Returns "pending"

NONE:           x = None                Null/undefined value

HEAP:           Internal GC-managed handle type (not user-facing)

TYPE INSPECTION:

    type(42)            # "number"
    type("hello")       # "string"
    type(true)          # "bool"
    type([1,2,3])       # "list"
    type(None)          # "none"
    type(handle)        # "pending"  ← new in v1.2

TYPE CONVERSION:

    str(42)             # "42"
    num("100")          # 100
    float("3.14")       # 3.14
    int(3.9)            # 3  (truncates)
    bool(1)             # true

================================================================================
6. LEXICAL GRAMMAR & TOKENS
================================================================================

WHITESPACE & INDENTATION:
    - Indentation is SIGNIFICANT (spaces or tabs).
    - Leading space changes produce Indent/Dedent tokens.
    - Newline separates statements.
    - Lines starting with # are comments.

IDENTIFIERS:    [A-Za-z_][A-Za-z0-9_]*

NUMBERS:        \d+(\.\d+)?   (parsed as f64)

STRINGS:        "(?:[^"\\]|\\.)*"  (double-quoted)

UNICODE MATH (auto-normalized by lexer):
    × → *    ⋅ → *    · → *
    ÷ → /    ⁄ → /
    ⁰¹²³⁴⁵⁶⁷⁸⁹ → 0-9 (superscripts)
    − → -

COMPLETE TOKEN TABLE:

    Token       Lexeme          Description
    ---------   -----------     ----------------------------------
    Indent      (implicit)      Increase block depth
    Dedent      (implicit)      Decrease block depth
    Newline     \n              Statement separator
    Identifier  [A-Za-z_]...   Variable/function names
    Number      \d+(\.\d+)?    Numeric literal (f64)
    String      "..."           Double-quoted string
    Bool        true/false      Boolean literal
    Plus        +               Addition / list concat
    Minus       -               Subtraction
    Star        *               Multiplication
    Slash       /               Division
    At          @               Matrix multiply
    Eq          =               Assignment
    EqEq        ==              Equality
    Neq         !=              Inequality
    Lt          <               Less than
    Gt          >               Greater than
    Lte         <=              Less or equal
    Gte         >=              Greater or equal
    Approx      ≈               Approximate equality
    NotEq       ≠               Unicode not equal
    StrictEq    ≡               Strict identity
    And         AND             Logical AND
    Or          OR              Logical OR
    Not         NOT             Logical NOT
    Dot         .               Member access / RET.NOW / OBJ.*.MUT
    Comma       ,               Separator
    Colon       :               Block header terminator
    LParen      (               Left parenthesis
    RParen      )               Right parenthesis
    LBracket    [               Left bracket (list/index)
    RBracket    ]               Right bracket
    Obj         OBJ             Object family declaration
    Spawn       SPAWN           Spawn block
    Do          DO              DO block keyword
    While       WHILE           While loop keyword
    For         FOR             Repeat count keyword
    As          AS              Thread alias keyword
    End         END             Block terminator
    Def         DEF             Function definition keyword
    Set         SET             Assignment keyword (optional)
    Over        OVER            Priority operator
    Limit       LIMIT           Constraint keyword
    Print       PRINT/ECHO      Output keyword
    If          IF              Conditional
    Otherwise   OTHERWISE       Else clause
    Try         TRY             Exception (partial)
    Eof         (end)           End of file

NOTE: RET is NOT a dedicated token. It arrives as Identifier("RET") and
is intercepted by the parser when followed by a Dot token.

================================================================================
7. KEYWORDS & ALIASES (COMPLETE)
================================================================================

CORE STATEMENTS:

    Keyword     Aliases                         Purpose
    -------     ------------------------------- --------------------------
    DEF         define, function, func          Define a function
    DO          run, start, spawn, begin        Execute block / thread
    FOR         times, repeat, using, with      Repeat count
    WHILE       -                               Conditional loop
    AS          named, called                   Thread alias
    END         stop, finish, terminate         Close block
    PRINT       echo, println, ECHO             Print output
    SET         assign, let, make               Assignment (optional)
    OVER        above, before                   Priority relationship
    LIMIT       bounded_by, under               Constraint limit
    IF          when, provided                  Conditional
    OTHERWISE   else, catch                     Else clause
    TRY         attempt                         Exception (partial)
    PAUSE       sleep, hold, suspend            (reserved)
    UNPAUSE     resume, continue                (reserved)
    RESTART     reset, rerun                    (reserved)
    WAIT        await, hold_for                 (reserved)
    GROUP       bundle                          (reserved)
    CLASS       type, kind                      (reserved)
    LEARN       build_model, make_net           (reserved)
    OBJ         obj                             Object family decl (partial)
    SPAWN       spawn                           Spawn block (partial)

BOOLEAN KEYWORDS:

    TRUE        true, True
    FALSE       false, False
    AND         and  (keyword for logical and)
    OR          or   (keyword for logical or)
    NOT         not, negate

RETURN KEYWORDS (parser-level, not dedicated tokens):

    RET.NOW():  expr    Immediate return from function
    RET.LATE(ms): expr  Deferred return (snapshot now, deliver after ms)

================================================================================
8. OPERATORS & PRECEDENCE
================================================================================

ARITHMETIC:
    a + b       Addition
    a - b       Subtraction
    a * b       Multiplication
    a / b       Division (float result)
    a @ b       Matrix multiply (tensors)

COMPARISON:
    a == b      Equal
    a != b      Not equal
    a < b       Less than
    a > b       Greater than
    a <= b      Less than or equal
    a >= b      Greater than or equal
    a ≈ b       Approximate equality
    a ≠ b       Unicode not equal
    a ≡ b       Strict identity (type + value)

LOGICAL:
    a AND b     Both true
    a OR b      Either true
    NOT a       Negate boolean

INDEXING:
    list[0]     Zero-based index access
    list[-1]    Negative index (from end)
    string[0]   Character access

OPERATOR PRECEDENCE (highest to lowest):

    1.  ( )                         Parentheses
    2.  * / @                       Multiplicative / matmul
    3.  + -                         Additive
    4.  < > <= >= == !=  ≈ ≠ ≡      Comparison
    5.  AND                         Logical AND
    6.  OR                          Logical OR
    7.  =                           Assignment (lowest)

================================================================================
9. STATEMENTS & SYNTAX
================================================================================

── ASSIGNMENT ──────────────────────────────────────────────────────────────────

    x = 10
    SET x = 10           # SET keyword optional
    y = x + 5

── FUNCTION DEFINITION (DEF) ───────────────────────────────────────────────────

    # With named parameters (NEW in v1.2)
    DEF add(a, b):
        RET.NOW(): a + b
    END

    DEF greet(name):
        PRINT "Hello " name
    END

    greet("World")
    result = add(3, 4)      # 7

    # Zero-parameter form (DO-compatible)
    DEF calculate:
        result = 3.14 * 5 * 5
        PRINT result
    END

── RET.NOW(): ──────────────────────────────────────────────────────────────────

    Syntax:  RET.NOW(): expr

    Evaluates expr immediately, returns the value to the caller, and
    stops executing the function body. Equivalent to Python's `return`.

    DEF abs_val(x):
        IF x < 0:
            RET.NOW(): x * -1
        END
        RET.NOW(): x
    END

    PRINT abs_val(-5)     # 5
    PRINT abs_val(3)      # 3

    DEF factorial(n):
        IF n <= 1:
            RET.NOW(): 1
        END
        RET.NOW(): n * factorial(n - 1)
    END

    NOTES:
    - RET.NOW() cancels any pending RET.LATE in the same function.
    - Works in lambdas and named functions.
    - RET is NOT a keyword token — parsed as identifier "RET" + Dot + "NOW".

── RET.LATE(ms): ───────────────────────────────────────────────────────────────

    Syntax:  RET.LATE(duration_ms): expr

    Snapshots the value of expr at declaration time, registers a timer,
    and CONTINUES executing the function body. Returns a Value::Pending
    handle to the caller. The snapshot is the value at declaration time —
    later changes to variables do NOT affect it.

    DEF slow_double(x):
        snapshot = x * 2
        RET.LATE(2000): snapshot    # snapshot x*2 now, deliver in 2 seconds
        PRINT "still running..."    # this executes immediately
    END

    handle = slow_double(21)        # returns immediately with Pending handle
    PRINT "doing other work"
    result = resolve(handle)        # blocks here until 2000ms have elapsed
    PRINT result                    # 42

    DURATION:
    - Argument is wall-clock milliseconds (integer).
    - Value snapshotted at RET.LATE declaration, not at resolve() time.
    - If RET.NOW() fires before resolve() is called, the pending handle
      is silently dropped (both won't normally appear in the same function).

    TRIGGER FORM (placeholder — not yet polling):
    - RET.LATE(check_fn()): value  — WhenTrue condition, fires when
      check_fn() returns truthy. Polling not yet implemented; stores
      deliver_at_ms=0 which causes resolve() to return immediately.

── DO BLOCK ────────────────────────────────────────────────────────────────────

    # Simple repeat
    DO 5:
        PRINT "hello"
    END

    # Named thread with alias and repeat count
    DO worker AS w FOR 3:
        PRINT w
    END

    # DO with body (inline)
    DO compute:
        x = x + 1
    END

── WHILE LOOP ──────────────────────────────────────────────────────────────────

    counter = 0
    DO WHILE counter < 5:
        PRINT counter
        counter = counter + 1
    END

    # Named while target
    i = 0
    DO loop WHILE i < 3:
        PRINT i
        i = i + 1
    END

    ITERATION LIMIT: Default 1,000,000 per target.

── IF / OTHERWISE ──────────────────────────────────────────────────────────────

    IF x > 100:
        PRINT "big"
    OTHERWISE:
        PRINT "small"
    END

    # Nested
    IF x > 100:
        PRINT "big"
    OTHERWISE:
        IF x > 50:
            PRINT "medium"
        OTHERWISE:
            PRINT "small"
        END
    END

── LAMBDA EXPRESSION ───────────────────────────────────────────────────────────

    double = lambda x: x * 2
    result = double(21)             # 42

    add = lambda a, b: a + b
    total = add(10, 20)             # 30

    # Multi-param lambda uses __arg_0__, __arg_1__... internally
    # Prefer DEF for multi-param functions (cleaner param names)

── PRIORITY OVERRIDE ───────────────────────────────────────────────────────────

    A OVER B                # A has higher priority than B
    task_a OVER task_b
    critical OVER background

── CONSTRAINT EXPRESSION ───────────────────────────────────────────────────────

    expr1 [relation] expr2 LIMIT OVER constraint_expr

    velocity distance LIMIT OVER time
    speed < max_speed LIMIT OVER engine_power

── PRINT / ECHO ────────────────────────────────────────────────────────────────

    PRINT x                 # Print single value
    PRINT "Hello"           # Print string
    PRINT "Value: " x       # Print label + value
    ECHO x                  # Alias for PRINT
    print(x)                # Function form
    println(x)              # Function form with newline

================================================================================
10. EXPRESSIONS
================================================================================

BINARY EXPRESSIONS (precedence climbing):

    2 + 3 * 4           # 14
    (2 + 3) * 4         # 20
    x > 5 AND y < 10

FUNCTION CALLS:

    add(3, 4)           # Named function call
    double(6)           # Lambda call
    abs(-7)             # Builtin call
    resolve(handle)     # Pending value resolution

LIST INDEXING:

    lst[0]              # First element
    lst[-1]             # Last element
    "pasta"[0]          # "p"  (string character)
    "pasta"[-1]         # "a"

LIST LITERALS:

    [1, 2, 3]
    ["a", "b", "c"]
    []                  # Empty list

================================================================================
11. SCOPING & ENVIRONMENT
================================================================================

SCOPE STACK MODEL:
    - Variables stored in a stack of HashMaps.
    - Each function call / DO block pushes a new scope.
    - Scope is popped on exit or after RET.NOW().
    - Variable lookup searches innermost scope outward.
    - set_global() always writes to scope[0].

FUNCTION SCOPE RULES:
    - DEF with named params: each param bound to its argument via set_local().
    - RET.NOW() correctly pops scope before returning.
    - RET.LATE() sets __ret_late__ local; caller captures it after body finishes.
    - Lambda params use __arg_0__, __arg_1__, ... as local names.

THREAD METADATA:
    - DO blocks register logical thread entries (id, name, priority_weight).
    - Logical threads — not OS threads unless scheduler feature is enabled.

================================================================================
12. BUILT-IN FUNCTIONS (COMPLETE REFERENCE)
================================================================================

── TYPE & CONVERSION ───────────────────────────────────────────────────────────

    type(value)         "number"|"string"|"bool"|"list"|"tensor"|
                        "lambda"|"pending"|"none"|"heap"

    str(value)          Convert to string
    num(value)          Parse to number
    float(value)        Convert to float (same as num)
    int(value)          Convert to integer (truncates)
    bool(value)         Convert to boolean

── MATH ─────────────────────────────────────────────────────────────────────────

    abs(x)              Absolute value
    sqrt(x)             Square root
    pow(base, exp)      base ^ exp
    floor(x)            Round down
    ceil(x)             Round up
    round(x)            Round to nearest
    min(a, b)           Minimum
    max(a, b)           Maximum
    clamp(v, lo, hi)    Clamp between lo and hi
    sign(x)             1, -1, or 0

── RANDOM ──────────────────────────────────────────────────────────────────────

    rand()              Random float [0, 1)
    rand_int(lo, hi)    Random integer [lo, hi]
    rand_range(lo, hi)  Random float [lo, hi)

── LIST OPERATIONS ──────────────────────────────────────────────────────────────

    len(collection)         Length of list, string, or tensor
    length(collection)      Alias for len
    list_first(list)        First element
    list_last(list)         Last element
    list_sum(list)          Sum all numbers
    list_average(list)      Average of numbers
    list_reverse(list)      Reversed copy
    list_take(list, n)      First n elements
    list_drop(list, n)      Skip first n elements
    list_slice(list, s, e)  Elements from index s to e
    list_concat(l1, l2)     Concatenate two lists
    list_flatten(list)      Flatten one level of nesting
    append(list, value)     Append value, return new list
    push(list, value)       Alias for append
    pop(list)               Remove and return last element
    contains(list, v)       Boolean membership test
    index_of(list, v)       Index of v (-1 if not found)
    sort(list)              Sorted copy
    range(n)                [0, 1, ..., n-1]
    range(start, end)       [start, ..., end-1]

── STRING OPERATIONS ────────────────────────────────────────────────────────────

    len(s)                  String length
    concat(s1, s2)          Concatenate (also: string_concat)
    upper(s)                Uppercase
    lower(s)                Lowercase
    trim(s)                 Trim whitespace
    split(s, delim)         Split by delimiter → list
    starts_with(s, prefix)  Boolean prefix check
    ends_with(s, suffix)    Boolean suffix check
    replace(s, old, new)    Replace all occurrences
    string_reverse(s)       Reverse string

── PENDING / DEFERRED RETURN ────────────────────────────────────────────────────

    resolve(handle)         Block until RET.LATE value is ready, return it.
                            If handle is already resolved, returns it immediately.
                            If handle is not Pending, passes through unchanged.

── PRINT / OUTPUT ───────────────────────────────────────────────────────────────

    PRINT value             Print value + newline (statement form)
    print(value)            Function form
    println(value)          Explicit newline form
    echo(value)             Alias

── SYSTEM ────────────────────────────────────────────────────────────────────────

    exit(code)              Exit interpreter with code
    sleep(ms)               Sleep milliseconds (blocks)
    time()                  Current Unix timestamp (float seconds)
    env(name)               Get environment variable

── GRAPHICS (feature: image) ────────────────────────────────────────────────────

    canvas_new(w, h, name)          Create canvas
    canvas_set(name, x, y, r, g, b) Set pixel
    canvas_save(name, path)         Save as PNG
    canvas_fill(name, r, g, b)      Fill with color

================================================================================
13. FILE I/O OPERATIONS
================================================================================

READ FILE:

    read_from_file(path)        Read file as list of byte values (0-255)
    rff(path)                   Short alias

WRITE FILE:

    write_to_file(filename, data)
    write_to_file(filename, data, output_directory)
    wtf(filename, data)         Short alias

    data can be a String or a list of byte numbers (0-255).

================================================================================
14. SHELL & FILESYSTEM OPERATIONS
================================================================================

All shell functions are accessible via shell.function() or plain function().
The REPL :shell command enters an interactive shell session.

DIRECTORY:

    shell.pwd()                     Current working directory
    shell.cd(path)                  Change directory
    shell.ls()                      List current directory
    shell.ls(path)                  List specific directory
    shell.ls_long(path)             Detailed listing
    shell.mkdir(path)               Create directory
    shell.mkdir(path, true)         Create with parents

FILE OPERATIONS:

    shell.touch(path)               Create empty file
    shell.cp(from, to)              Copy file
    shell.mv(from, to)              Move/rename file
    shell.rm(path)                  Delete file
    shell.rmdir(path)               Delete empty directory
    shell.rmdir_r(path)             Recursive delete

FILE INFO:

    shell.exists(path)              Boolean
    shell.is_file(path)             Boolean
    shell.is_dir(path)              Boolean
    shell.file_size(path)           Size in bytes
    shell.realpath(path)            Absolute path

================================================================================
15. AI/ML & TENSOR OPERATIONS
================================================================================

── TENSOR CREATION ──────────────────────────────────────────────────────────────

    tensor.zeros([rows, cols])
    tensor.ones([rows, cols])
    tensor.eye(n)
    tensor.rand([rows, cols])
    tensor.from_list([1, 2, 3])

── TENSOR INSPECTION ────────────────────────────────────────────────────────────

    tensor.shape(t)         Returns shape as list
    tensor.dtype(t)         Returns dtype string ("float32")
    tensor.sum(t)           Sum of all elements
    tensor.mean(t)          Mean of all elements

── TENSOR MANIPULATION ──────────────────────────────────────────────────────────

    tensor.reshape(t, new_shape)
    tensor.transpose(t)
    tensor.flatten(t)

── NEURAL NETWORK ───────────────────────────────────────────────────────────────

    ai.linear(in_dim, out_dim)      Fully-connected linear layer
    ai.mlp([in, hidden..., out])    Multi-layer perceptron
    ai.relu(tensor)                 ReLU activation
    ai.softmax(tensor)              Softmax probabilities
    ai.loss.mse(pred, target)       Mean Squared Error
    ai.loss.crossentropy(logits, class_idx)

    # Both dot and underscore notation work:
    ai.relu(t)   ≡   ai_relu(t)

── AUTOGRAD (library level) ─────────────────────────────────────────────────────

    Located in src/ai/autograd.rs. Supports: add, sub, mul, div, neg,
    sum, mean, relu, powf, matmul with reverse-mode autodiff.
    Not yet exposed as PASTA builtins — see TODO.

================================================================================
16. PRIORITY & CONSTRAINT SYSTEM
================================================================================

PRIORITY:

    A OVER B                    # Directed edge A → B
    urgent OVER routine
    a OVER b
    b OVER c

    Implements topological sort with cycle detection.
    Weights decay at 0.75 per step from highest priority.
    Used by scheduler (feature-gated).

CONSTRAINT:

    expr1 [relation] expr2 LIMIT OVER constraint_expr

    velocity distance LIMIT OVER time
    speed < max_speed LIMIT OVER engine_power

    ConstraintEngine.validate_all() run at end of execute_program().

================================================================================
17. STANDARD LIBRARY (stdlib) REFERENCE
================================================================================

Load with:  include stdlib.pa  (NOT auto-loaded — must be explicit)

The stdlib provides 180+ functions across 19 modules including:
    assert, range, repeat, is_empty, is_null, identity, const
    map, filter, reduce, compose, pipe, partial, flip, curry
    list operations, string formatting, math helpers,
    statistics (mean, median, variance, std_dev),
    date/time helpers, random utilities, and more.

See stdlib.pa for full function listing.

================================================================================
18. REPL & INTERACTIVE MODE
================================================================================

    ./target/release/pasta          Start REPL

REPL COMMANDS:

    exit / quit         Exit
    :help               Show command list
    :env                Show all variables in scope
    :threads            Show active DO threads
    :keywords           Show all keywords
    :reset              Reset interpreter state
    :diag               Show and clear diagnostics
    :clear              Clear screen (ANSI)
    :shell              Enter integrated shell (ls, cd, mkdir, rm, cp, mv)

MULTI-LINE INPUT:
    The REPL accumulates indented blocks automatically.
    Press Enter on a blank line at indent level 0 to execute.

================================================================================
19. CLI USAGE
================================================================================

    ./target/release/pasta file.pa          Run a file
    ./target/release/pasta                  Start REPL
    ./target/release/pasta --verbose file   Verbose diagnostics
    ./target/release/pasta --eval 'code'    Eval inline string

================================================================================
20. EXAMPLES
================================================================================

── BASIC FUNCTION WITH RET.NOW ──────────────────────────────────────────────────

    DEF clamp_val(x, lo, hi):
        IF x < lo:
            RET.NOW(): lo
        END
        IF x > hi:
            RET.NOW(): hi
        END
        RET.NOW(): x
    END

    PRINT clamp_val(5, 1, 10)    # 5
    PRINT clamp_val(-3, 1, 10)   # 1
    PRINT clamp_val(15, 1, 10)   # 10

── DEFERRED RETURN WITH RET.LATE ────────────────────────────────────────────────

    DEF delayed_square(x):
        snapshot = x * x
        RET.LATE(1000): snapshot     # deliver in 1 second
        PRINT "computed, waiting..."
    END

    handle = delayed_square(7)
    PRINT "handle type: " type(handle)  # pending
    result = resolve(handle)            # blocks ~1s
    PRINT result                        # 49

── FIBONACCI (RECURSIVE) ────────────────────────────────────────────────────────

    DEF fib(n):
        IF n <= 1:
            RET.NOW(): n
        END
        RET.NOW(): fib(n - 1) + fib(n - 2)
    END

    PRINT fib(10)    # 55

── LAMBDA PIPELINE ──────────────────────────────────────────────────────────────

    double = lambda x: x * 2
    square = lambda x: x * x

    PRINT double(5)      # 10
    PRINT square(4)      # 16
    PRINT double(square(3))  # 18

── LIST PROCESSING ──────────────────────────────────────────────────────────────

    nums = [3, 1, 4, 1, 5, 9, 2, 6]
    PRINT list_sum(nums)         # 31
    PRINT list_average(nums)     # 3.875
    PRINT list_reverse(nums)     # [6, 2, 9, 5, 1, 4, 1, 3]
    PRINT list_slice(nums, 2, 5) # [4, 1, 5]

── PRIORITY GRAPH ───────────────────────────────────────────────────────────────

    critical OVER important
    important OVER normal
    normal OVER background

    # Executor maintains directed graph, detects cycles.

================================================================================
21. TESTING & DEBUGGING
================================================================================

RUN TEST SUITE:

    ./target/release/pasta tests/test_suite.ps      # Core tests (all pass)
    ./target/release/pasta tests/test_advanced.ps   # Advanced tests (all pass)

REPL DEBUGGING:

    :env                    Dump all variables
    :diag                   Show executor diagnostics
    type(x)                 Inspect value type
    PRINT x                 Print any value

VERBOSE MODE:

    ./target/release/pasta --verbose file.pa
    RUST_BACKTRACE=1 cargo test testname --lib -- --nocapture

CARGO TESTS:

    cargo test              All tests
    cargo test --lib        Library tests only

================================================================================
22. PERFORMANCE & LIMITATIONS
================================================================================

STRENGTHS:
    - Rapid scripting and prototyping
    - AI/ML experimentation without external deps
    - System automation via shell operations
    - Priority and constraint specification
    - Deferred returns (RET.LATE) for async-style patterns

LIMITATIONS:
    - Pure tree-walking interpreter (no JIT, no bytecode)
    - Execution is single-threaded (threading is logical/simulated)
    - WHILE loops capped at 1,000,000 iterations by default
    - RET.LATE(trigger_fn()) polling not yet implemented
    - No lazy evaluation
    - OBJ/CLASS system data structures exist but not wired

OPTIMIZATION TIPS:
    1. Use vectorized tensor operations over manual loops
    2. Use DEF with params instead of globals for cleaner recursion
    3. Use release build (cargo build --release) for ~10x speedup
    4. Batch filesystem operations

================================================================================
23. TROUBLESHOOTING
================================================================================

ISSUE: "Undefined variable"
    Check: spelling, scope. Use :env in REPL to inspect.

ISSUE: "constructor calls not implemented"
    Fixed in v1.2 — all identifier(args) now parsed as Call, not ConstructorCall.
    If you see this, check you have the latest build.

ISSUE: Unexpected token near RET
    Ensure syntax is: RET.NOW(): expr   (dot between RET and NOW, colon after)

ISSUE: resolve() returns immediately without waiting
    RET.LATE(trigger_fn()) WhenTrue polling is a placeholder — returns immediately.
    Use RET.LATE(ms) with a millisecond duration for timed delivery.

ISSUE: Permission denied on build
    sudo chown -R $USER:$USER target && rm -rf target

ISSUE: Parser errors about missing colons
    All block headers require a colon: IF x > 0:  /  DEF f(x):  /  DO WHILE:

ISSUE: Program hangs
    WHILE loops default limit: 1,000,000. Use timeout 5 ./pasta program.pa

ISSUE: Type mismatch
    PRINT type(x) to inspect value type before operations

================================================================================
24. ARCHITECTURE OVERVIEW
================================================================================

EXECUTION PIPELINE:

    Source Code (.pa file)
         │
         ▼
    ┌─────────────┐
    │    LEXER    │  src/lexer/lexer.rs
    │  Tokenizes  │  Unicode normalization, indent/dedent, alias table
    └──────┬──────┘
           │ Vec<Token>
           ▼
    ┌─────────────┐
    │   PARSER    │  src/parser/parser.rs
    │  Builds AST │  Precedence climbing, RET.NOW/LATE parsing
    └──────┬──────┘
           │ Program (AST)
           ▼
    ┌─────────────┐
    │  EXECUTOR   │  src/interpreter/executor.rs
    │  Walks AST  │  ControlFlowSignal, function dispatch, builtins
    └──────┬──────┘
           │
           ├─── Environment (scope stack, Value enum)
           ├─── PriorityGraph (directed edges A→B, topo sort)
           ├─── ConstraintEngine (constraint validation)
           ├─── Shell (filesystem ops)
           ├─── Strainer GC (heap reference counting, mark-and-sweep)
           ├─── Rng (hardware or software RNG)
           └─── ai_network (neural network runtime)

CONTROL FLOW (new in v1.2):

    ControlFlowSignal::Return(Value)
        Set by RetNow handler in execute_statement().
        Checked after each statement in all function call loops.
        Cleared (taken) at function boundary — does not propagate past caller.

VALUE ENUM (environment.rs):
    Number(f64)
    String(String)
    Bool(bool)
    List(Vec<Value>)
    Tensor(RuntimeTensor)
    Lambda(Vec<Statement>)
    Pending(Box<Value>, u64)   ← snapshotted value + deliver_at_ms epoch time
    Heap(GcRef)
    None

AST STATEMENT NODES (ast.rs):
    Assignment { target, value }
    FunctionDef { name, params, body }     ← params: Vec<Identifier>
    RetNow { value }                       ← NEW in v1.2
    RetLate { value, condition }           ← NEW in v1.2
    DoBlock { targets, alias, repeats, body }
    WhileBlock { targets, alias, condition, body }
    If { conditions, then_body, else_body }
    PriorityOverride { higher, lower }
    Constraint { left, relation, right, constraint }
    Print { expr }
    ExprStmt { expr }
    ObjDecl { ... }                        ← data structures wired, execution pending
    SpawnBlock { entries }                 ← pending
    DefDoUntil { ... }                     ← pending

AST EXPR NODES:
    Number, String, Bool, Identifier
    Binary { op, left, right }
    Call { callee, args }                  ← dispatches: named fn → lambda → builtin
    Lambda(Vec<Statement>)
    List { items }
    Index { base, indices }
    ConstructorCall { family_name, args }  ← OBJ system, pending wiring
    Combine, Reassign                      ← OBJ system, pending wiring
    TensorBuilder { expr }

FUNCTION CALL DISPATCH (Expr::Call):
    1. Check self.functions map (DEF with params) → bind params, execute, catch Return
    2. Check environment for Value::Lambda → bind __arg_N__, execute, catch Return
    3. Fall through to call_builtin() → builtins, tensor ops, AI ops, shell ops

GARBAGE COLLECTOR (Strainer):
    Simple mark-and-sweep on heap-allocated values.
    Runs automatically after each top-level statement.
    Manual trigger: executor.collect_garbage()

================================================================================
25. VERSION & TEST STATUS
================================================================================

Language:       PASTA (Program for Assignment, Statements, Threading, Allocation)
Implementation: Rust (Edition 2021)
Version:        1.2
Date:           March 7, 2026

CHANGELOG:

v1.2 (March 7, 2026):
    ✓ DEF with named parameters — positional arg binding at call site
    ✓ Lambda expressions: lambda x: expr  (multi-param supported)
    ✓ Lambda calls dispatch through environment before builtins
    ✓ All identifier(args) forms correctly parsed as Call (not ConstructorCall)
    ✓ RET.NOW(): expr — immediate return with ControlFlowSignal unwind
    ✓ RET.LATE(ms): expr — snapshot-now / deliver-later (Value::Pending)
    ✓ resolve(handle) builtin — blocks until RET.LATE timer expires
    ✓ list_flatten() correctly derefs heap-wrapped inner lists
    ✓ do_print() and print() builtin deref heap values before display
    ✓ Lists print as [a, b, c] (single line) not item-per-line
    ✓ ExprStmt returns evaluated value (enables lambda body returns)
    ✓ Math builtins: abs, sqrt, pow, floor, ceil, round, min, max, clamp, sign
    ✓ Conversion builtins: int, num, float, bool, str
    ✓ Random builtins: rand, rand_int, rand_range
    ✓ System builtins: exit, sleep, time, env
    ✓ type() returns "pending" for Value::Pending
    ✓ All 18 advanced test sections passing

v1.1 (March 1, 2026):
    ✓ Fixed AND/OR operator tokenization
    ✓ Added NOT token
    ✓ Fixed WHILE loop thread ID allocation for lambda targets
    ✓ AI neural network module (ai_network.rs)
    ✓ 10 new AI builtins
    ✓ IF/ELSE AST nodes + executor
    ✓ Traceback system
    ✓ Strainer GC integrated

TEST RESULTS (v1.2):
    test_suite.ps:    all passing
    test_advanced.ps: all 18 sections passing
    cargo test --lib: ~145 tests passing

================================================================================
26. TODO
================================================================================

PRIORITY 1 — Language completeness:

    [ ] RET.LATE(trigger_fn()) — implement WhenTrue polling loop in resolve()
        Currently stores deliver_at_ms=0 and returns immediately.
        Needs: poll trigger_fn() in a sleep loop until truthy.

    [ ] RETURN statement — alias for RET.NOW() for users expecting Python style.
        Simple: parse "return expr" → Statement::RetNow { value: expr }.

    [ ] Object system wiring — OBJ/SPAWN/MUT execution:
        a. Add Value::Object(u64) to Value enum
        b. Wire Statement::ObjDecl → register_object_family()
        c. Wire Expr::ConstructorCall → instantiate_object()
        d. Wire mutation invocation syntax
        e. Wire field access (obj.field reads in eval_expr)

    [ ] TRY/OTHERWISE exception handling — executor handler needed.
        Parser already produces the AST nodes.

PRIORITY 2 — Ergonomics:

    [ ] DEF with default parameter values: DEF f(x, y=0):
    [ ] Variadic functions: DEF f(args...):
    [ ] RETURN as alias for RET.NOW() (common user expectation)
    [ ] Multi-line lambda body (currently single expression only)
    [ ] String interpolation: "Hello {name}"

PRIORITY 3 — Runtime:

    [ ] Autograd builtins exposed as PASTA functions (autograd.rs exists)
    [ ] PAUSE/UNPAUSE/RESTART execution control
    [ ] WAIT/AWAIT condition waiting (pairs with RET.LATE WhenTrue)
    [ ] GROUP thread grouping
    [ ] Dictionary/map type (currently simulated as list-of-pairs)
    [ ] Import / module system (stdlib.pa currently requires manual include)
    [ ] REPL history (up-arrow recall)

PRIORITY 4 — Tooling:

    [ ] cargo clippy clean — zero warnings target
    [ ] Formatter for .pa files
    [ ] LSP / syntax highlighting definitions
    [ ] CLASS keyword full implementation
    [ ] LEARN ML macro

================================================================================
SUPPORT & CONTRIBUTIONS
================================================================================

DEVELOPMENT WORKFLOW:
    cargo build          # Build
    cargo test           # Test
    RUST_BACKTRACE=1 cargo test  # Test with backtraces
    cargo fmt            # Format
    cargo clippy         # Lint

CONTRIBUTING:
    - Keep changes small and focused
    - Include tests for new behavior
    - Run cargo test before opening PR
    - Never commit with sudo artifacts in target/

================================================================================
END OF PASTALANG UNIFIED REFERENCE
================================================================================
Last Updated: March 7, 2026
PASTA v1.2
CONTACT: Travis Garrison, Brainboy2487@gmail.com
