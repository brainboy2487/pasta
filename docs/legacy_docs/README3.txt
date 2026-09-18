================================================================================
PASTA — PROGRAM FOR ASSIGNMENT, STATEMENTS, THREADING, AND ALLOCATION
================================================================================
Version: 1.3 (March 12, 2026)
Language: PASTA
Implementation: Rust (Edition 2021)
Author: Travis Garrison <Brainboy2487@gmail.com>

PASTA is a custom scripting language interpreter written in Rust. It supports
concurrent thread semantics, priority/constraint graphs, tensor and AI/ML ops,
deferred return values, a full standard library, integrated shell operations,
and a Meatball Runtime Agent (MRA) subsystem — all from a clean, indentation-
based syntax inspired by structured pseudocode.

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
18.  Typing System
19.  Meatballs Runtime Agent (MRA)
20.  REPL & Interactive Mode
21.  CLI Usage
22.  Examples
23.  Testing & Debugging
24.  Performance & Limitations
25.  Troubleshooting
26.  Architecture Overview
27.  Version History & Changelog
28.  TODO / Roadmap

================================================================================
1. QUICK START
================================================================================

Save as hello.pa:

    x = 42
    PRINT x
    PRINT "Hello, World!"

Build and run:

    cargo build --release
    ./target/release/pasta hello.pa

Run inline:

    ./target/release/pasta --eval 'x = 42\nPRINT x\n'

Start the interactive REPL:

    ./target/release/pasta

File extensions:
    .pa     PASTA source files (primary)
    .ps     PASTA source files (alternate / test scripts)
    .ph     PASTA header files (stdlib modules)
    .pasta  PASTA source files (alternate)

================================================================================
2. BUILDING & INSTALLATION
================================================================================

REQUIREMENTS:
    - Rust 1.56+ (Edition 2021)
    - Cargo
    - Standard development tools (gcc or clang)

BUILD STEPS:

    git clone <repo>
    cd pasta

    cargo build                 # Debug build
    cargo build --release       # Release build (~10x faster)

    cargo test                  # Run all tests
    cargo test --lib            # Unit tests only
    RUST_BACKTRACE=1 cargo test <name> --lib -- --nocapture

OPTIONAL CARGO FEATURES:

    cargo build --features canvas_png   # PNG canvas support (dep: image crate)
    cargo build --features ndarray      # Advanced tensor backend (ndarray)
    cargo build --features scheduler    # Task scheduler support

NOTE: The image feature is named "canvas_png" (not "image") to avoid a crate
name collision. Use --features canvas_png for PNG save support.

MAINTENANCE:

    cargo fmt           # Format code
    cargo clippy        # Lint
    cargo build -v      # Verbose build output

IMPORTANT: Never use sudo cargo — this causes permission issues in target/.
If target/ has permission errors:
    sudo chown -R $USER:$USER target
    rm -rf target && cargo build

================================================================================
3. PROJECT STRUCTURE
================================================================================

pasta/
├── src/
│   ├── lib.rs                      Main library exports
│   ├── ai/                         AI/ML subsystems
│   │   ├── autograd.rs             Reverse-mode autodiff engine
│   │   ├── datasets.rs             Data loading utilities
│   │   ├── generate.rs             Generation utilities
│   │   ├── learn.rs                LEARN macro scaffolding
│   │   ├── models.rs               Pre-built model templates
│   │   ├── tensor.rs               Tensor utilities
│   │   └── tokenizer.rs            Tokenization helpers
│   ├── bin/
│   │   └── pasta.rs                Binary entry point (CLI)
│   ├── interpreter/                Core interpreter
│   │   ├── executor.rs             Statement execution, builtins, ControlFlow
│   │   ├── environment.rs          Variable storage, scope stack, Value enum
│   │   ├── repl.rs                 Interactive REPL
│   │   ├── shell.rs                High-level shell operations
│   │   ├── shell_os/               OS-level shell adapter
│   │   │   ├── cli/                REPL :shell CLI
│   │   │   ├── commands/           Filesystem command handlers
│   │   │   └── vfs/                Virtual filesystem (fs.rs, path.rs, node.rs)
│   │   ├── ai_network.rs           Neural network runtime
│   │   ├── errors.rs               RuntimeError, Traceback types
│   │   └── mod.rs                  Module exports
│   ├── lexer/                      Tokenization
│   │   ├── lexer.rs                Main lexer (indent/dedent, unicode normalization)
│   │   ├── tokens.rs               TokenType enum
│   │   ├── alias.rs                Keyword alias table (JSON-overridable)
│   │   └── unicode.rs              Unicode math operator normalization
│   ├── meatballs/                  Meatball Runtime Agent (MRA)
│   │   ├── api/                    Rust API surface for MRA
│   │   ├── agent/                  Agent binary (JSON-over-stdio)
│   │   ├── backends/               Backend implementations (local, pseudo-vm, vm)
│   │   ├── cli/                    MRA CLI interface
│   │   ├── phase0/                 Design artifacts (schema, objectives)
│   │   ├── runtime/                Runtime hooks and scheduler stubs
│   │   └── tests/
│   ├── parser/                     Syntax analysis
│   │   ├── parser.rs               Main parser (precedence climbing)
│   │   ├── ast.rs                  AST node types (Statement, Expr, RetLateCondition)
│   │   └── grammar.rs              EBNF grammar reference
│   ├── pasta_async/                Async runtime sub-crate
│   │   └── src/                    api, io, runtime, serialize, sync, testing
│   ├── runtime/                    Runtime subsystems
│   │   ├── asm.rs                  Sandboxed ASM block runtime
│   │   ├── bitwise.rs              Bitwise operations
│   │   ├── devices.rs              Device detection & auto-configure
│   │   ├── meatball.rs             Meatball runtime hooks
│   │   ├── rng.rs                  RNG (hardware preferred)
│   │   ├── scheduler.rs            Task scheduler (feature-gated)
│   │   ├── strainer.rs             Garbage collector (mark-and-sweep)
│   │   └── threading.rs            Thread metadata
│   ├── saucey/
│   │   └── saucey.rs               Saucey utility module (experimental)
│   ├── semantics/                  Semantic analysis
│   │   ├── constraints.rs          Constraint engine
│   │   ├── priority.rs             Priority graph (directed, cycle-detected)
│   │   └── resolver.rs             Symbol resolution (scope-aware)
│   ├── stdlib/                     Standard library
│   │   ├── stdlib.pa               Main stdlib (180+ functions, 19 modules)
│   │   ├── stdio.ph                Standard I/O header (auto-loaded)
│   │   ├── pasta_G.ph              Math/general header (auto-loaded)
│   │   ├── sys.ph                  System namespace
│   │   ├── time.ph                 Time namespace
│   │   ├── rand.ph                 Random namespace
│   │   ├── gc.ph                   GC namespace
│   │   ├── debug.ph                Debug namespace
│   │   ├── fs.ph                   Filesystem namespace
│   │   ├── net.ph                  Network namespace
│   │   ├── ffi.ph                  FFI namespace
│   │   ├── thread.ph               Threading namespace
│   │   ├── device.ph               Device namespace
│   │   ├── tensor.ph               Tensor namespace
│   │   ├── memory.ph               Memory namespace
│   │   └── math.ph                 Math namespace
│   ├── typing/                     Cross-type coercion engine
│   │   ├── mod.rs                  Module root
│   │   ├── types.rs                Type definitions
│   │   ├── operands.rs             Numeric operator dispatch
│   │   ├── float.rs                Float rounding / formatting
│   │   ├── int.rs                  Integer coercion
│   │   ├── bool.rs / bool_coerce.rs  Boolean coercion
│   │   ├── string.rs / string_coerce.rs  String coercion
│   │   ├── tensor_type.rs          Tensor type bridge
│   │   └── util.rs                 Promotion helpers
│   └── utils/                      Utilities
│       ├── errors.rs               Error types & diagnostics
│       ├── logging.rs              pasta_info!/pasta_debug!/etc. macros
│       └── helpers.rs              now_millis, bytes_to_hex, etc.
├── tests/                          Integration test scripts (.ps)
│   ├── 01_basic_print.ps
│   ├── 01_arithmetic_bindings.ps
│   ├── 04_basic_while.ps
│   ├── 05_nested_while.ps
│   ├── 06_functions_and_lambdas.ps
│   ├── 07_do_multi_alias_repeat.ps
│   ├── 08_test_RET.ps
│   ├── 09_big_test.ps           (30-section regression suite)
│   ├── 10_small_test_30.ps
│   ├── test_suite.ps
│   └── test_advanced.ps
├── docs/                           Documentation
│   ├── README1.txt                 (v1.1 reference — superseded)
│   ├── README2.txt                 (v1.2 reference — superseded)
│   ├── meatball_readme.txt
│   ├── shell_readme.txt
│   └── typing_readme.txt
├── examples/                       Example .pasta programs
├── DiskImages/                     fs.img (virtual disk image)
├── tools/
│   └── output_dir_tree.py          Directory tree utility
├── Cargo.toml
└── Cargo.lock

================================================================================
4. CORE LANGUAGE FEATURES
================================================================================

IMPLEMENTED & WORKING (v1.3):

    Core:
    ✓ Variable assignment with type inference
    ✓ Arithmetic: +  -  *  /  %
    ✓ Logical: AND  OR  NOT
    ✓ Comparison: ==  !=  <  >  <=  >=
    ✓ Unicode operators: ≈  ≠  ≡  (token-defined and wired in lexer)
    ✓ Unicode math normalization: ×→*  ÷→/  ⁰⁻⁹→digits  −→-

    Control flow:
    ✓ DO blocks (named threads, alias, repeat count)
    ✓ WHILE loops (named, lambda, multiple targets)
    ✓ Nested DO/WHILE and WHILE+IF interaction
    ✓ IF / OTHERWISE conditionals (fully working)
    ✓ ATTEMPT(err_var): ... ELSE: ... END  try/except syntax

    Functions:
    ✓ DEF name(params): ... END  with named parameter binding
    ✓ RET.NOW(): expr  — immediate return with ControlFlowSignal unwind
    ✓ RET.LATE(ms): expr  — snapshot-now, deliver-later (Value::Pending)
    ✓ resolve(handle)  — block until RET.LATE is ready
    ✓ Lambda: lambda x: expr  (multi-param supported)
    ✓ Lambda dispatch from environment before builtins

    Types & builtins:
    ✓ All data types: Number, String, Bool, List, Tensor, Lambda, Pending, None
    ✓ Type inspection: type(x)
    ✓ Type conversion: int, num, float, bool, str
    ✓ Math builtins: abs, sqrt, pow, floor, ceil, round, min, max, clamp, sign
    ✓ Trig builtins (via headers): sin, cos, tan, log, is_nan, is_inf
    ✓ Random: rand, rand_int, rand_range
    ✓ List builtins (full suite — see section 12)
    ✓ String builtins: upper, lower, trim, split, starts_with, ends_with, replace
    ✓ System: exit, sleep, time, env, pwd

    I/O & shell:
    ✓ File I/O: read_from_file, write_to_file
    ✓ Shell operations: shell.* namespace (ls, cd, mkdir, rm, cp, mv, etc.)
    ✓ VFS: virtual filesystem layer (shell_os/)

    AI/ML:
    ✓ Tensor operations: tensor.* namespace
    ✓ Neural network: ai.* namespace (linear, mlp, relu, softmax, loss.mse, etc.)
    ✓ Autograd engine (library level — reverse-mode autodiff)

    Infrastructure:
    ✓ Proper lexical scoping (scope stack, set_local / set_global)
    ✓ Priority graph (directed, cycle-detected, topo-sorted)
    ✓ Constraint engine (LIMIT OVER expressions)
    ✓ Garbage collector (Strainer, mark-and-sweep)
    ✓ Traceback / error diagnostics with span info
    ✓ REPL with :env, :threads, :keywords, :shell, :diag, :reset
    ✓ Header auto-loading at startup (.ph files)
    ✓ Typing system with cross-type coercion matrix

PARTIALLY IMPLEMENTED:
    ~ OBJ/SPAWN/MUT object family system  (AST + data structures; exec pending)
    ~ Autograd PASTA builtins  (engine exists in src/ai/autograd.rs; not wired)
    ~ Meatball Runtime Agent  (scaffold complete; backends WIP)
    ~ pasta_async sub-crate  (API surface exists; integration WIP)

RESERVED / NOT YET IMPLEMENTED:
    ✗ PAUSE / UNPAUSE / RESTART execution control
    ✗ WAIT / AWAIT condition waiting
    ✗ GROUP thread grouping
    ✗ CLASS type definitions
    ✗ LEARN ML macro keyword
    ✗ RET.LATE(trigger_fn()) polling (WhenTrue condition — placeholder)
    ✗ Value::Object(u64) for object instances as first-class values
    ✗ Dictionary/map type (currently simulated as list-of-pairs)
    ✗ Import / module system (stdlib.pa requires manual include)

================================================================================
5. DATA TYPES
================================================================================

PASTA has 9 value types (Value enum in interpreter/environment.rs):

NUMBER      42          Integer (stored as f64)
            3.14        Floating point
            -10.5       Negative

STRING      "hello"     Double-quoted strings with escape sequences

BOOL        true        Aliases: True, TRUE
            false       Aliases: False, FALSE

LIST        [1, 2, 3]   Homogeneous or mixed
            []          Empty list
            [1, "mixed", true]

TENSOR      tensor.zeros([2, 3])    Zero matrix
            tensor.ones([5, 5])
            tensor.eye(4)           Identity matrix
            tensor.rand([3, 4])

LAMBDA      double = lambda x: x * 2
            result = double(6)      # 12

PENDING     handle = slow_fn()       Returned by RET.LATE
            result = resolve(handle) Blocks until ready
            type(handle)             Returns "pending"

NONE        x = None                 Null / undefined

HEAP        Internal GC-managed handle (not user-facing)

TYPE INSPECTION:

    type(42)            # "number"
    type("hello")       # "string"
    type(true)          # "bool"
    type([1,2,3])       # "list"
    type(None)          # "none"
    type(handle)        # "pending"

TYPE CONVERSION:

    str(42)             # "42"
    num("100")          # 100
    float("3.14")       # 3.14
    int(3.9)            # 3   (truncates)
    bool(1)             # true

================================================================================
6. LEXICAL GRAMMAR & TOKENS
================================================================================

WHITESPACE & INDENTATION:
    - Indentation is SIGNIFICANT (spaces or tabs).
    - Leading space changes emit Indent / Dedent tokens.
    - Newline separates statements.
    - Lines starting with # are comments.
    - DEDENT is emitted before END when END appears at body indentation level;
      the parser guards against consuming END prematurely inside body loops.

IDENTIFIERS:    [A-Za-z_][A-Za-z0-9_]*

NUMBERS:        \d+(\.\d+)?     parsed as f64

STRINGS:        "(?:[^"\\]|\\.)*"   double-quoted, escape sequences

DOT ABSORPTION: The lexer absorbs dots into identifier tokens, so "sys.env"
lexes as a single Identifier token. All dotted namespace dispatch in
call_builtin() relies on this behavior.

UNICODE NORMALIZATION (auto-applied by lexer):
    ×  →  *       ⋅  →  *       ·  →  *
    ÷  →  /       ⁄  →  /
    −  →  -
    ⁰¹²³⁴⁵⁶⁷⁸⁹  →  0-9  (superscripts)

COMPLETE TOKEN TABLE:

    Token       Lexeme          Description
    ---------   -----------     ------------------------------------------
    Indent      (implicit)      Increase block depth
    Dedent      (implicit)      Decrease block depth
    Newline     \n              Statement separator
    Identifier  [A-Za-z_]...   Variable/function names (+ dot-absorbed names)
    Number      \d+(\.\d+)?    Numeric literal (f64)
    String      "..."           Double-quoted string
    Bool        true/false      Boolean literal
    Plus        +               Addition / list concat
    Minus       -               Subtraction / unary negate
    Star        *               Multiplication
    Slash       /               Division
    Percent     %               Modulo
    At          @               Matrix multiply (tensors)
    Eq          =               Assignment
    EqEq        ==              Equality
    Neq         !=              Inequality
    Lt          <               Less than
    Gt          >               Greater than
    Lte         <=              Less or equal
    Gte         >=              Greater or equal
    Approx      ≈               Approximate equality
    NotEq       ≠               Unicode not equal
    StrictEq    ≡               Strict identity (type + value)
    And         AND             Logical AND (also: &&)
    Or          OR              Logical OR  (also: ||)
    Not         NOT             Logical NOT (also: !)
    Dot         .               Member access / RET.NOW / RET.LATE
    Comma       ,               Separator
    Colon       :               Block header terminator
    LParen      (               Left parenthesis
    RParen      )               Right parenthesis
    LBracket    [               Left bracket (list / index)
    RBracket    ]               Right bracket
    Do          DO              DO block keyword
    While       WHILE           While loop keyword
    For         FOR             Repeat count keyword
    As          AS              Thread alias keyword
    End         END             Block terminator
    Def         DEF             Function definition keyword
    Set         SET             Assignment keyword (optional)
    Over        OVER            Priority operator
    Limit       LIMIT           Constraint keyword
    LimitOver   LIMIT OVER      Combined constraint token
    Print       PRINT / ECHO    Output keyword
    If          IF              Conditional
    Otherwise   OTHERWISE       Else clause
    Attempt     ATTEMPT         Try/except keyword
    Obj         OBJ             Object family declaration (partial)
    Spawn       SPAWN           Spawn block (partial)
    True        TRUE/true       Boolean true
    False       FALSE/false     Boolean false
    None        None/NONE       Null value
    Eof         (end)           End of file

NOTE: RET is NOT a dedicated token. It arrives as Identifier("RET") and
is intercepted by the parser when followed by a Dot token.

================================================================================
7. KEYWORDS & ALIASES (COMPLETE)
================================================================================

Keyword     Aliases                         Purpose
-------     ------------------------------- -----------------------------------
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
ATTEMPT     try, attempt                    Try/except
PAUSE       sleep, hold, suspend            (reserved)
UNPAUSE     resume, continue                (reserved)
RESTART     reset, rerun                    (reserved)
WAIT        await, hold_for                 (reserved)
GROUP       bundle                          (reserved)
CLASS       type, kind                      (reserved)
LEARN       build_model, make_net           (reserved)
OBJ         obj                             Object family (partial)
SPAWN       spawn                           Spawn block (partial)

BOOLEAN:
    TRUE        true, True
    FALSE       false, False
    AND         and  (&&)
    OR          or   (||)
    NOT         not, negate  (!)

RETURN KEYWORDS (parser-level — not dedicated tokens):
    RET.NOW():   expr    Immediate return
    RET.LATE(ms): expr   Deferred return

================================================================================
8. OPERATORS & PRECEDENCE
================================================================================

ARITHMETIC:
    a + b       Addition
    a - b       Subtraction
    a * b       Multiplication
    a / b       Division (float result)
    a % b       Modulo
    a @ b       Matrix multiply (tensor operands)

COMPARISON:
    a == b      Equal
    a != b      Not equal
    a < b       Less than
    a > b       Greater than
    a <= b      Less or equal
    a >= b      Greater or equal
    a ≈ b       Approximate equality
    a ≠ b       Unicode not equal
    a ≡ b       Strict identity (type + value)

    NOTE: For Lt / Gt / Lte / Gte, if numeric coercion fails the executor
    falls back to lexicographic (string) comparison.

LOGICAL:
    a AND b     Both true (also &&)
    a OR b      Either true (also ||)
    NOT a       Negate boolean (also !)

INDEXING:
    list[0]     Zero-based index
    list[-1]    Negative index (from end)
    "str"[0]    Character access

OPERATOR PRECEDENCE (highest → lowest):
    1.  ( )                         Parentheses
    2.  * / % @                     Multiplicative / matmul
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

    DEF greet(name):
        PRINT "Hello " name
    END

    DEF add(a, b):
        RET.NOW(): a + b
    END

    result = add(3, 4)   # 7
    greet("World")

    # Zero-parameter, DO-compatible form
    DEF calculate:
        result = 3.14 * 5 * 5
        PRINT result
    END

── RET.NOW(): ──────────────────────────────────────────────────────────────────

    Evaluates expr immediately, returns to the caller, and stops executing
    the function body. Equivalent to Python's `return`.

    DEF abs_val(x):
        IF x < 0:
            RET.NOW(): x * -1
        END
        RET.NOW(): x
    END

    PRINT abs_val(-5)    # 5
    PRINT abs_val(3)     # 3

    DEF factorial(n):
        IF n <= 1:
            RET.NOW(): 1
        END
        RET.NOW(): n * factorial(n - 1)
    END

    NOTES:
    - RET.NOW() cancels any pending RET.LATE in the same function.
    - Works in both named functions and lambdas.
    - Cancels any RET.LATE in the same function scope.

── RET.LATE(ms): ───────────────────────────────────────────────────────────────

    Snapshots the value of expr at declaration time, registers a timer for
    duration_ms milliseconds, and CONTINUES executing the function body.
    Returns Value::Pending to the caller immediately.

    DEF slow_double(x):
        snapshot = x * 2
        RET.LATE(2000): snapshot    # snapshot now, deliver in 2 seconds
        PRINT "still running..."    # executes immediately
    END

    handle = slow_double(21)        # returns Pending immediately
    PRINT "doing other work"
    result = resolve(handle)        # blocks ~2000 ms
    PRINT result                    # 42

    TRIGGER FORM (placeholder):
    - RET.LATE(check_fn()): value  — WhenTrue condition
      Polling not yet implemented; resolve() returns immediately for this form.

── DO BLOCK ────────────────────────────────────────────────────────────────────

    # Simple repeat
    DO worker FOR 3:
        PRINT "Working..."
    END

    # Named with alias
    DO processor AS p FOR 5:
        x = x + 1
    END

    # Inline (no target name)
    DO:
        PRINT "Inline block"
    END

    # Nested DO
    DO outer FOR 2:
        DO inner FOR 3:
            PRINT "nested"
        END
    END

    # Multiple targets (concurrent semantics)
    DO a, b FOR 2:
        PRINT "parallel"
    END

── WHILE LOOP ──────────────────────────────────────────────────────────────────

    counter = 0
    DO loop WHILE counter < 5:
        PRINT counter
        counter = counter + 1
    END

    # Lambda WHILE (no target name)
    i = 0
    DO WHILE i < 3:
        PRINT i
        i = i + 1
    END

    ITERATION LIMIT: Default 1,000,000 per target.
    Override: set while_limit = n  in code.

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

── ATTEMPT / ELSE (Try/Except) ─────────────────────────────────────────────────

    ATTEMPT(err):
        risky_operation()
    ELSE:
        PRINT "Error caught: " err
    END

    NOTE: Parser is wired; executor handler is a stub. Full exception
    propagation is planned for a future iteration.

── LAMBDA EXPRESSION ───────────────────────────────────────────────────────────

    double = lambda x: x * 2
    result = double(21)             # 42

    add = lambda a, b: a + b
    total = add(10, 20)             # 30

── PRIORITY OVERRIDE ───────────────────────────────────────────────────────────

    A OVER B                # Adds directed edge A → B to priority graph
    critical OVER background
    process_a OVER process_b

── CONSTRAINT EXPRESSION ───────────────────────────────────────────────────────

    expr1 [relation] expr2 LIMIT OVER constraint_expr

    velocity distance LIMIT OVER time
    speed < max_speed LIMIT OVER engine_power

── PRINT / ECHO ────────────────────────────────────────────────────────────────

    PRINT x
    PRINT "Value: " x       # Label + value (space-separated)
    ECHO x                  # Alias for PRINT
    print(x)                # Function form
    println(x)              # Function form with newline

================================================================================
10. EXPRESSIONS
================================================================================

BINARY EXPRESSIONS (precedence climbing):

    2 + 3 * 4           # 14 (not 20)
    (2 + 3) * 4         # 20
    x > 5 AND y < 10    # boolean AND

FUNCTION CALLS:

    add(3, 4)           # Named DEF call
    double(6)           # Lambda call
    abs(-7)             # Builtin call
    resolve(handle)     # Deferred value resolution

DISPATCH ORDER for Call expressions:
    1. self.functions map  (DEF with named params)
    2. environment Value::Lambda  (lambda variables)
    3. call_builtin()  (builtins, tensor, AI, shell ops)

LIST INDEXING:

    lst[0]              # First element (0-based)
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
    - Each function call / DO block pushes a new scope on entry.
    - Scope is popped on exit or after RET.NOW().
    - Variable lookup searches innermost scope outward, then globals.
    - set_local()   — writes to current (innermost) scope.
    - set_global()  — always writes to scope[0].

FUNCTION SCOPE RULES:
    - Named params: each param bound to its argument via set_local().
    - RET.NOW() correctly pops scope before returning.
    - RET.LATE() sets __ret_late__ local; caller captures Pending handle.
    - Lambda params use __arg_0__, __arg_1__, ... as local names.
    - ControlFlowSignal::Return(Value) propagates through statement loops
      and is consumed (cleared) at the function boundary — does not escape.

THREAD METADATA:
    - DO blocks register logical thread entries (id, name, alias, priority).
    - Logical threads — not OS threads unless the scheduler feature is enabled.

================================================================================
12. BUILT-IN FUNCTIONS (COMPLETE REFERENCE)
================================================================================

── TYPE & CONVERSION ───────────────────────────────────────────────────────────

    type(value)             "number"|"string"|"bool"|"list"|"tensor"|
                            "lambda"|"pending"|"none"|"heap"
    str(value)              Convert to string
    num(value)              Parse to number
    float(value)            Convert to float (same as num)
    int(value)              Convert to integer (truncates f64)
    bool(value)             Convert to boolean

── MATH ─────────────────────────────────────────────────────────────────────────

    abs(x)                  Absolute value
    sqrt(x)                 Square root
    pow(base, exp)          base ^ exp
    floor(x)                Round down
    ceil(x)                 Round up
    round(x)                Round to nearest integer
    min(a, b)               Minimum of two values
    max(a, b)               Maximum of two values
    clamp(v, lo, hi)        Clamp between lo and hi
    sign(x)                 1, -1, or 0

    # Via math.ph / pasta_G.ph headers:
    sin(x)                  Sine
    cos(x)                  Cosine
    tan(x)                  Tangent
    log(x)                  Natural logarithm
    math.gcd(a, b)          Greatest common divisor
    math.lcm(a, b)          Least common multiple
    math.factorial(n)       n!
    math.is_nan(x)          Boolean: is NaN?
    math.is_inf(x)          Boolean: is infinite?

── RANDOM ──────────────────────────────────────────────────────────────────────

    rand()                  Random float in [0, 1)
    rand_int(lo, hi)        Random integer in [lo, hi]
    rand_range(lo, hi)      Random float in [lo, hi)
    rand.shuffle(list)      Shuffle list (via rand.ph)
    rand.sample(list, n)    Sample n items from list

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
    list_slice(list, s, e)  Elements from index s to e (exclusive)
    list_concat(l1, l2)     Concatenate two lists
    list_flatten(list)      Flatten one level of nesting
    append(list, value)     Append value, return new list
    push(list, value)       Alias for append
    pop(list)               Remove and return last element
    head(list)              First element
    tail(list)              All but first element
    reverse(list)           Reverse list
    sort(list)              Sorted copy (numbers or strings)
    contains(list, v)       Boolean membership test
    index_of(list, v)       Index of v (-1 if not found)
    zip(l1, l2)             Zip two lists into list-of-pairs
    range(n)                [0, 1, ..., n-1]
    range(start, end)       [start, ..., end-1]

── STRING OPERATIONS ────────────────────────────────────────────────────────────

    len(s)                  String length
    concat(s1, s2)          Concatenate
    upper(s)                Uppercase
    lower(s)                Lowercase
    trim(s)                 Trim whitespace
    split(s, delim)         Split by delimiter → list
    join(list, delim)       Join list of strings → string
    starts_with(s, prefix)  Boolean prefix check
    ends_with(s, suffix)    Boolean suffix check
    contains(s, sub)        Substring check
    replace(s, old, new)    Replace all occurrences
    string_reverse(s)       Reverse string

── PENDING / DEFERRED RETURN ────────────────────────────────────────────────────

    resolve(handle)         Block until RET.LATE value is ready, return it.
                            If handle is already resolved: return immediately.
                            If handle is not Pending: pass through unchanged.

── PRINT / OUTPUT ───────────────────────────────────────────────────────────────

    PRINT value             Print value + newline (statement form)
    ECHO value              Alias
    print(value)            Function form
    println(value)          Explicit newline form

── SYSTEM ────────────────────────────────────────────────────────────────────────

    exit(code)              Exit interpreter with status code
    sleep(ms)               Sleep milliseconds (blocks)
    time()                  Current Unix timestamp (float seconds)
    env(name)               Get environment variable (string or None)
    pwd()                   Current working directory (string)

── GRAPHICS (feature: canvas_png) ──────────────────────────────────────────────

    canvas_new(w, h, name)          Create canvas
    canvas_set(name, x, y, r, g, b) Set pixel
    canvas_save(name, path)         Save as PNG
    canvas_fill(name, r, g, b)      Fill with color

── STDLIB NAMESPACES (via .ph headers, auto-loaded) ─────────────────────────────

    sys.env(name)               System environment variable
    sys.exit(code)              Exit via sys namespace
    time.now()                  Current timestamp
    time.sleep(ms)              Sleep
    time.delta(t1, t2)          Time difference
    debug.vars()                Dump current scope variables
    debug.backtrace()           Print call stack (uses .context field)
    gc.collect()                Manual GC trigger
    gc.stats()                  GC statistics
    fs.read(path)               File read
    fs.write(path, data)        File write
    fs.touch(path)              Create/touch file
    fs.basename(path)           File name from path
    fs.dirname(path)            Directory from path
    fs.ext(path)                File extension
    net.get(url)                HTTP GET (stub)
    net.post(url, data)         HTTP POST (stub)
    ffi.call(lib, fn, args)     FFI call (stub)
    thread.spawn(fn)            Spawn thread (stub)
    thread.join(handle)         Join thread (stub)
    device.list()               List detected devices
    device.info(name)           Device info
    tensor.zeros / ones / eye / rand / from_list  (see section 15)
    memory.alloc(n)             Allocate n bytes
    memory.free(handle)         Free allocation
    math.*                      Math namespace functions (see section 12)
    rand.*                      Random namespace functions (see section 12)

================================================================================
13. FILE I/O OPERATIONS
================================================================================

READ FILE:

    read_from_file(path)        Read file as list of byte values (0–255)
    rff(path)                   Short alias

WRITE FILE:

    write_to_file(filename, data)
    write_to_file(filename, data, output_directory)
    wtf(filename, data)         Short alias

    data can be a String or a list of byte numbers (0–255).

EXAMPLE:

    write_to_file("output.txt", "Hello from PASTA!")
    content = read_from_file("output.txt")
    PRINT "Read " len(content) " bytes"

================================================================================
14. SHELL & FILESYSTEM OPERATIONS
================================================================================

All shell operations are accessible via shell.function() or plain function().
The REPL :shell command enters an interactive shell session.

DIRECTORY:

    shell.pwd()                     Current working directory
    shell.cd(path)                  Change directory (abs, rel, ~, .., .)
    shell.ls()                      List current directory → list of names
    shell.ls(path)                  List specific directory
    shell.ls_long(path)             Detailed listing → ["name size type", ...]
    shell.mkdir(path)               Create directory
    shell.mkdir(path, true)         Create with parent directories

FILE OPERATIONS:

    shell.touch(path)               Create empty file or update timestamp
    shell.cp(from, to)              Copy file
    shell.mv(from, to)              Move or rename file
    shell.rm(path)                  Delete file
    shell.rmdir(path)               Delete empty directory
    shell.rmdir_r(path)             Recursive delete
    shell.rmdir_recursive(path)     Long alias for rmdir_r

FILE INFO:

    shell.exists(path)              Boolean: does path exist?
    shell.is_file(path)             Boolean: is it a regular file?
    shell.is_dir(path)              Boolean: is it a directory?
    shell.file_size(path)           Size in bytes (0 if not found)
    shell.realpath(path)            Absolute / canonical path

VIRTUAL FILESYSTEM (shell_os/):
    The shell_os/ subsystem provides a VFS layer (fs.rs, path.rs, node.rs)
    and command handlers that back the integrated :shell REPL session and
    the shell.* namespace. OS operations route through the vfs/ abstraction,
    making cross-platform and sandboxed environments possible.

================================================================================
15. AI/ML & TENSOR OPERATIONS
================================================================================

── TENSOR CREATION ──────────────────────────────────────────────────────────────

    tensor.zeros([rows, cols])      Zero matrix
    tensor.ones([rows, cols])       Ones matrix
    tensor.eye(n)                   n×n identity matrix
    tensor.rand([rows, cols])       Random matrix (uniform)
    tensor.from_list([1, 2, 3])     1D tensor from list

── TENSOR INSPECTION ────────────────────────────────────────────────────────────

    tensor.shape(t)                 Returns shape as list, e.g. [3, 3]
    tensor.dtype(t)                 Returns dtype string, e.g. "float32"
    tensor.sum(t)                   Sum of all elements
    tensor.mean(t)                  Mean of all elements

── TENSOR MANIPULATION ──────────────────────────────────────────────────────────

    tensor.reshape(t, new_shape)    Reshape tensor
    tensor.transpose(t)             Transpose 2D tensor
    tensor.flatten(t)               Flatten to 1D tensor

── TENSOR ARITHMETIC ────────────────────────────────────────────────────────────

    tensor.add(a, b)                Elementwise addition
    tensor.sub(a, b)                Elementwise subtraction
    tensor.mul(a, b)                Elementwise multiplication
    tensor.div(a, b)                Elementwise division
    a @ b                           Matrix multiplication (@ operator)

── TENSOR CONVERSION ────────────────────────────────────────────────────────────

    tensor.from_list(list)          List → tensor
    ai.list_to_tensor(list)         Alias
    ai.tensor_to_list(tensor)       Tensor → list

── NEURAL NETWORK ───────────────────────────────────────────────────────────────

    ai.linear(in_dim, out_dim)      Fully-connected linear layer (Xavier init)
    ai.mlp([in, hidden..., out])    Multi-layer perceptron (auto-ReLU)
    ai.relu(tensor)                 ReLU activation: max(0, x)
    ai.softmax(tensor)              Softmax probabilities (sum = 1.0)
    ai.loss.mse(pred, target)       Mean Squared Error
    ai.loss.crossentropy(logits, class_idx)  Cross-entropy loss

    # Dot and underscore notation both work:
    ai.relu(t)   ≡   ai_relu(t)

── AUTOGRAD ENGINE (src/ai/autograd.rs) ─────────────────────────────────────────

    Reverse-mode autodiff with optional gradient tracking (requires_grad).
    Supported ops: add, sub, mul, div, neg, sum, mean, relu, powf, matmul.
    backward() accumulates gradients.
    Currently at library level — not yet exposed as PASTA builtins.

================================================================================
16. PRIORITY & CONSTRAINT SYSTEM
================================================================================

PRIORITY GRAPH:

    A OVER B                        Adds directed edge A → B
    urgent OVER routine
    process_a OVER process_b
    process_b OVER process_c
    # Creates: process_a > process_b > process_c

    - Executor maintains a PriorityGraph (directed, cycle-detected).
    - Priority weights decay at 0.75 per step from highest.
    - Topological sort via scheduler (feature-gated).
    - Cycles are detected and emitted as diagnostics.

CONSTRAINT ENGINE:

    expr1 [relation] expr2 LIMIT OVER constraint_expr

    velocity distance LIMIT OVER time
    speed < max_speed LIMIT OVER engine_power
    temperature level LIMIT OVER cooling_capacity

    ConstraintEngine.validate_all() is called at end of execute_program().
    Outcomes: Satisfiable | Unsatisfiable | RequiresOptimization | Error.

================================================================================
17. STANDARD LIBRARY (stdlib) REFERENCE
================================================================================

Location: src/stdlib/stdlib.pa
Load with:  include stdlib.pa    (NOT auto-loaded — must be explicit)

Auto-loaded headers (at startup): stdio.ph, pasta_G.ph, and all .ph files
in src/stdlib/ are scanned and loaded during Executor::new().

180+ functions across 19 modules:

MODULE 1:  Utility         assert, range, repeat, is_empty, is_null, identity, const
MODULE 2:  Strings         string_length, string_concat, string_pad_*, upper/lower/trim/split...
MODULE 3:  Lists           list_first, list_last, list_take, list_drop, list_slice,
                           list_flatten, list_unique, list_sum, list_average, list_min,
                           list_max, list_map, list_filter, list_reduce
MODULE 4:  Math            math_abs, math_min, math_max, math_clamp, math_sign,
                           math_is_even, math_is_odd, math_factorial, math_power,
                           math_gcd, math_lcm
MODULE 5:  File            file_read, file_write, file_append, file_exists,
                           file_delete, file_size, file_is_readable, file_is_writable
MODULE 6:  Directory       dir_exists, dir_create, dir_create_recursive, dir_list,
                           dir_delete, dir_delete_recursive
MODULE 7:  Filesystem      fs_copy, fs_move, fs_path_join, fs_basename, fs_dirname,
                           fs_extension
MODULE 8:  Type Checking   is_number, is_string, is_bool, is_list, is_tensor,
                           is_lambda, is_none
MODULE 9:  Control Flow    while_limited, do_n_times, retry
MODULE 10: Priority        priority_set, priority_chain, get_thread_id, get_thread_name
MODULE 11: Validation      validate_number, validate_string, validate_list,
                           validate_range, validate_positive, validate_not_empty
MODULE 12: Formatting      format_number, format_percent, format_list, format_table,
                           format_pad_center
MODULE 13: Tensors         tensor_create_zeros/ones/eye/rand, tensor_from_list,
                           tensor_to_list, tensor_shape, tensor_sum, tensor_mean,
                           tensor_add, tensor_scale, tensor_dot, tensor_reshape,
                           tensor_transpose
MODULE 14: AI/ML           ai_create_linear, ai_create_mlp, ai_relu, ai_softmax,
                           ai_mse_loss, ai_crossentropy, ai_forward, ai_predict
MODULE 15: Data            data_normalize, data_standardize, data_shuffle,
                           data_split, data_batch
MODULE 16: Benchmark       bench_start, bench_end, bench_run
MODULE 17: Logging         log_info, log_warn, log_error, log_debug, debug_dump
           NOTE: log_warn / log_error emit to stderr.
MODULE 18: Collections     set_create, set_add, set_contains, set_union,
                           set_intersect, set_difference, dict_create, dict_set,
                           dict_get, dict_keys, dict_values
MODULE 19: Functional      compose, curry, pipe, memoize, once

================================================================================
18. TYPING SYSTEM
================================================================================

Location: src/typing/

The typing module centralizes numeric promotion, rounding, and downcast logic.

KEY FILES:

    util.rs             Promotion helper and engine-config extraction
    operands.rs         compute_numeric_op and apply_round_and_downcast
    float.rs            Rounding and display formatting helpers
    int.rs              Integer coercion
    bool.rs / bool_coerce.rs    Boolean coercion
    string.rs / string_coerce.rs  String coercion
    tensor_type.rs      Tensor type bridge
    types.rs            Type definitions

COERCION ENGINE:
    DefaultCoercion carries CoercionConfig.
    StandardExecutor attempts to downcast engine to DefaultCoercion to read config.
    For other engines, executor falls back to global float helpers.
    Division produces float results by default (division_always_float = true).

BRIDGE FUNCTIONS:
    bridge_from_env     environment::Value → typing engine Value
    bridge_to_env       typing engine Value → environment::Value
    apply_op_env        Apply typed operator via bridge, return env Value

ROUNDING LEVELS (1–5):
    1: None             2: 2 decimal places     3: 4 decimal places
    4: Round to int     5: Truncate to int

DEPENDENCY:
    once_cell = "1.18" in Cargo.toml (unconditional; no longer feature-gated)

================================================================================
19. MEATBALLS RUNTIME AGENT (MRA)
================================================================================

Location: src/meatballs/

The Meatball Runtime Agent is a sandboxed execution environment scaffold for
running PASTA-adjacent workloads in isolated contexts (VMs, pseudo-VMs, local).

CURRENT STATUS: Skeleton / scaffold complete. Backends are WIP.

STRUCTURE:

    api/            Rust API surface for the MRA (meatball_api.rs)
    agent/          Agent binary communicating via JSON-over-stdio
    backends/       Backend implementations: local, pseudo-vm, vm
    cli/            CLI interface for MRA control
    phase0/         Design artifacts (mra_schema.json, objective.md.txt)
    runtime/        Runtime hooks and scheduler stubs (runtime.rs)
    tests/          MRA-specific tests

NEXT STEPS (from meatball_readme.txt):
    - Wrap run_cli to accept PASTA Environment / Executor types
    - Hook into executor.rs via wrapper method calling shell entrypoint
    - Resolve name collisions and update module paths

DiskImages/fs.img is a virtual disk image used by the vm backend.

================================================================================
20. REPL & INTERACTIVE MODE
================================================================================

    ./target/release/pasta          Start REPL (no arguments)
    ./target/release/pasta --repl   Explicit REPL flag

REPL COMMANDS:

    :help               Show available commands
    :env                Dump all variables in current scope
    :threads            Show active DO thread metadata
    :keywords           List all keywords and AI functions
    :reset              Reset interpreter state
    :diag               Show and clear executor diagnostics
    :clear              Clear screen (ANSI)
    :shell              Enter integrated shell (ls, cd, mkdir, rm, cp, mv)
    :quit / exit        Exit REPL

MULTI-LINE INPUT:
    The REPL accumulates indented blocks automatically.
    Press Enter on a blank line at indent level 0 to execute the block.

REPL EXAMPLE SESSION:

    pasta> x = 10
    pasta> y = x * 2
    pasta> PRINT y
    20
    pasta> net = ai.mlp([3, 4, 2])
    pasta> PRINT net
    ai.MLP: linear_0 -> relu_0 -> linear_1
    pasta> :env
    x = 10
    y = 20
    net = ai.MLP: ...
    pasta> :quit

================================================================================
21. CLI USAGE
================================================================================

    ./target/release/pasta file.pa              Run a PASTA source file
    ./target/release/pasta                      Start REPL
    ./target/release/pasta --eval 'code'        Evaluate inline string
    ./target/release/pasta --verbose file.pa    Verbose diagnostics
    ./target/release/pasta --version            Show version
    ./target/release/pasta --help               Show usage

FLAGS:

    Flag                Purpose                 Example
    ----------------    ----------------------- ----------------------------
    --eval TEXT         Evaluate inline code    pasta --eval 'x = 1\nPRINT x'
    --verbose           Verbose diagnostics     pasta --verbose program.pa
    --repl              Start REPL explicitly   pasta --repl
    --version           Show version            pasta --version
    --help              Show usage              pasta --help

DEBUGGING:

    RUST_BACKTRACE=1 ./target/release/pasta program.pa
    RUST_BACKTRACE=full ./target/release/pasta program.pa
    timeout 10 ./target/release/pasta long_program.pa

================================================================================
22. EXAMPLES
================================================================================

── HELLO WORLD ──────────────────────────────────────────────────────────────────

    message = "Hello, World!"
    PRINT message

── ARITHMETIC ───────────────────────────────────────────────────────────────────

    x = 10
    y = 5
    PRINT "Sum: " x + y          # 15
    PRINT "Product: " x * y      # 50
    PRINT "Division: " x / y     # 2

── FUNCTION WITH RETURN ─────────────────────────────────────────────────────────

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

── DEFERRED RETURN ───────────────────────────────────────────────────────────────

    DEF delayed_square(x):
        snapshot = x * x
        RET.LATE(1000): snapshot
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

── WHILE LOOP ───────────────────────────────────────────────────────────────────

    counter = 0
    DO loop WHILE counter < 5:
        PRINT "Iteration " counter
        counter = counter + 1
    END

── LAMBDA PIPELINE ──────────────────────────────────────────────────────────────

    double = lambda x: x * 2
    square = lambda x: x * x

    PRINT double(5)           # 10
    PRINT square(4)           # 16
    PRINT double(square(3))   # 18

── LIST PROCESSING ──────────────────────────────────────────────────────────────

    nums = [3, 1, 4, 1, 5, 9, 2, 6]
    PRINT list_sum(nums)         # 31
    PRINT list_average(nums)     # 3.875
    PRINT list_reverse(nums)     # [6, 2, 9, 5, 1, 4, 1, 3]
    PRINT list_slice(nums, 2, 5) # [4, 1, 5]

── TENSOR OPERATIONS ────────────────────────────────────────────────────────────

    matrix = tensor.zeros([3, 3])
    identity = tensor.eye(3)
    PRINT tensor.shape(identity)    # [3, 3]
    PRINT tensor.sum(identity)      # 3

── NEURAL NETWORK ───────────────────────────────────────────────────────────────

    net = ai.mlp([784, 256, 128, 10])
    PRINT net

    logits = tensor.from_list([1.0, 2.0, 3.0, 0.5])
    activated = ai.relu(logits)
    probs = ai.softmax(activated)

    x = tensor.from_list([1.0, 2.0, 3.0])
    y = tensor.from_list([1.1, 2.1, 2.9])
    loss = ai.loss.mse(x, y)
    PRINT "MSE Loss: " loss

── FILESYSTEM ───────────────────────────────────────────────────────────────────

    shell.mkdir("/tmp/project", true)
    shell.touch("/tmp/project/notes.txt")
    files = shell.ls("/tmp/project")
    PRINT "Files: " files

    IF shell.is_file("/tmp/project/notes.txt"):
        size = shell.file_size("/tmp/project/notes.txt")
        PRINT "Size: " size " bytes"
    END

── PRIORITY GRAPH ───────────────────────────────────────────────────────────────

    critical OVER important
    important OVER normal
    normal OVER background

── NESTED SCOPING ───────────────────────────────────────────────────────────────

    x = 100

    DO outer FOR 1:
        x = 200
        PRINT x                 # 200
        DO inner FOR 1:
            x = 300
            PRINT x             # 300
        END
        PRINT x                 # 200
    END

    PRINT x                     # 100

================================================================================
23. TESTING & DEBUGGING
================================================================================

RUNNING TESTS:

    cargo test                                      All tests
    cargo test --lib                                Unit tests only
    cargo test -- --nocapture                       With output visible
    RUST_BACKTRACE=1 cargo test <name> --lib -- --nocapture

INTEGRATION TEST SCRIPTS:

    ./target/release/pasta tests/test_suite.ps      Core suite (all pass)
    ./target/release/pasta tests/test_advanced.ps   Advanced (all 18 sections pass)
    ./target/release/pasta tests/09_big_test.ps     30-section regression suite

TEST LOCATIONS:
    src/lexer/lexer.rs              Lexer unit tests
    src/parser/parser.rs            Parser unit tests
    src/interpreter/executor.rs     Executor + builtin tests
    src/interpreter/ai_network.rs   AI module tests
    src/runtime/                    Runtime unit tests

REPL DEBUGGING:

    :env                    Dump all variables
    :diag                   Show executor diagnostics
    :threads                Show DO thread state
    type(x)                 Inspect value type
    PRINT x                 Print any value

RUST-SIDE DEBUG:
    - Add eprintln! in Environment::assign, push_scope / pop_scope,
      and Executor::DoBlock to trace runtime state.

PYTHON TEST HARNESS:
    Automated testing with timeouts; writes test_logs.txt with timestamps,
    stdout/stderr, return codes.

COMMON DIAGNOSTICS:

    "Undefined variable"         Check spelling, check scope (:env in REPL)
    "Unexpected token"           Check indentation, colons, END keywords
    "Iteration limit exceeded"   WHILE loop ran > 1,000,000 iterations
    "Type error"                 Use type(x) to inspect
    "Permission denied"          chown target/ and rebuild

================================================================================
24. PERFORMANCE & LIMITATIONS
================================================================================

STRENGTHS:
    - Rapid scripting and prototyping
    - AI/ML experimentation without external dependencies
    - System automation via shell / VFS
    - Priority and constraint specification for scheduling
    - Async-style deferred returns (RET.LATE / resolve())
    - Meatball sandboxed execution scaffold for isolated workloads

LIMITATIONS:
    - Pure tree-walking interpreter (no JIT, no bytecode compilation)
    - Execution is single-threaded (DO threads are logical, not OS threads)
    - WHILE loops capped at 1,000,000 iterations by default
    - RET.LATE(trigger_fn()) polling not yet implemented
    - No lazy evaluation
    - OBJ/CLASS data structures exist but execution is not yet wired
    - Dictionary/map type simulated as list-of-pairs
    - Import / module system requires manual include

OPTIMIZATION TIPS:
    1. Use cargo build --release for ~10x speedup over debug
    2. Prefer vectorized tensor operations over manual loops
    3. Use DEF with params instead of globals for cleaner recursion
    4. Batch filesystem operations
    5. Use RET.LATE for non-blocking compute patterns

================================================================================
25. TROUBLESHOOTING
================================================================================

ISSUE: "Undefined variable"
    Check spelling and scope. Use :env in REPL to inspect all bindings.

ISSUE: Unexpected token near RET
    Ensure syntax is: RET.NOW(): expr   or   RET.LATE(ms): expr
    (colon after the closing parenthesis, no space before colon)

ISSUE: resolve() returns immediately without waiting
    RET.LATE(trigger_fn()) WhenTrue form is a placeholder — resolves immediately.
    Use RET.LATE(ms) with a millisecond integer for timed delivery.

ISSUE: DEDENT before END causes parse failures
    The lexer emits DEDENT before END when END appears at body indentation
    level. The parser guards against this in DEF body loops and parse_do_body.
    Ensure you are on a build that includes the parser.rs DEDENT/END fix.

ISSUE: Parser errors about missing constructor calls
    Fixed in v1.2 — all identifier(args) are now parsed as Call, not
    ConstructorCall. Ensure you are on a current build.

ISSUE: Permission denied on build
    sudo chown -R $USER:$USER target && rm -rf target

ISSUE: Program hangs
    WHILE default limit is 1,000,000. Use:
        timeout 5 ./target/release/pasta program.pa

ISSUE: Tensor shape mismatch
    PRINT tensor.shape(t) before operations.

ISSUE: Headers not loading
    Run from project root. Headers expected at src/stdlib/*.ph.
    Check :diag in REPL for load error messages.

ISSUE: canvas_png feature fails to build
    Feature is named "canvas_png", not "image".
    Use: cargo build --features canvas_png

================================================================================
26. ARCHITECTURE OVERVIEW
================================================================================

EXECUTION PIPELINE:

    Source Code (.pa / .ps file)
         │
         ▼
    ┌──────────────┐
    │    LEXER     │  src/lexer/lexer.rs
    │  Tokenizes   │  Unicode normalization, indent/dedent, alias expansion
    └──────┬───────┘
           │ Vec<Token>
           ▼
    ┌──────────────┐
    │   PARSER     │  src/parser/parser.rs
    │  Builds AST  │  Precedence climbing, RET.NOW/LATE parsing,
    └──────┬───────┘  DEDENT/END guard in DEF + DO body loops
           │ Program (AST)
           ▼
    ┌──────────────┐
    │   EXECUTOR   │  src/interpreter/executor.rs
    │  Walks AST   │  ControlFlowSignal, function dispatch, all builtins
    └──────┬───────┘
           │
           ├── Environment      (scope stack + globals, Value enum)
           ├── PriorityGraph    (directed edges, topo sort, cycle detection)
           ├── ConstraintEngine (constraint validation)
           ├── Shell            (filesystem operations via shell_os/)
           ├── Strainer GC      (mark-and-sweep, runs after each top-level stmt)
           ├── Rng              (hardware RNG preferred, software fallback)
           └── ai_network       (neural network runtime)

CONTROL FLOW (ControlFlowSignal):

    ControlFlowSignal::Return(Value)
        Set by RetNow handler in execute_statement().
        Propagates through statement loops inside execute_body().
        Cleared (taken) at the function call boundary — does not escape to caller.

VALUE ENUM (environment.rs):

    Number(f64)
    String(String)
    Bool(bool)
    List(Vec<Value>)
    Tensor(RuntimeTensor)
    Lambda(Vec<Statement>)
    Pending(Box<Value>, u64)    // snapshotted_value + deliver_at_epoch_ms
    Heap(GcRef)
    None

AST STATEMENT NODES (ast.rs):

    Assignment { target, value }
    FunctionDef { name, params: Vec<Identifier>, body }
    RetNow { value }
    RetLate { value, condition: RetLateCondition }
    DoBlock { targets, alias, repeats, body }
    WhileBlock { targets, alias, condition, body }
    If { conditions, then_body, else_body }
    Attempt { error_var, try_body, else_body }
    PriorityOverride { higher, lower }
    Constraint { left, relation, right, constraint }
    Print { expr }
    ExprStmt { expr }
    ObjDecl { ... }             // data structures wired; execution pending
    SpawnBlock { entries }      // pending
    Return { value }            // alias for RetNow (reserved)

AST EXPR NODES:

    Number, String, Bool, Identifier
    Binary { op, left, right }
    Call { callee, args }       // dispatches: named fn → lambda → builtin
    Lambda(Vec<Statement>)
    List { items }
    Index { base, indices }
    ConstructorCall { ... }     // OBJ system; execution pending
    TensorBuilder { expr }

GARBAGE COLLECTOR (Strainer):
    Simple mark-and-sweep on heap-allocated values.
    Runs automatically after each top-level statement.
    Manual trigger available via gc.collect() or executor.collect_garbage().

CRITICAL API CONSTRAINTS (costly to get wrong):
    - ParseError::new(span, message)  — span is the FIRST argument
    - env.set_local()  not  env.set()
    - TraceFrame uses .context field, not .label
    - DEDENT is emitted before END at body-level indentation;
      parser guards against prematurely consuming END in body loops
    - canvas_png feature flag (NOT "image") to avoid crate name collision
    - Lexer absorbs dots into identifiers: "sys.env" is ONE token

================================================================================
27. VERSION HISTORY & CHANGELOG
================================================================================

v1.3 (March 12, 2026) — current
    ✓ Unified and updated README (this document)
    ✓ ATTEMPT(err_var): ... ELSE: ... END  try/except syntax wired in parser
    ✓ canvas_png Cargo feature (renamed from "image" to fix crate collision)
    ✓ once_cell dependency made unconditional in Cargo.toml
    ✓ Full cross-type coercion matrix in src/typing/
    ✓ Bridge functions: bridge_from_env, bridge_to_env, apply_op_env
    ✓ Lt/Gt/Lte/Gte lexicographic fallback when numeric coercion fails
    ✓ All 13 stdlib .ph headers expanded from stubs to real PASTA source
    ✓ DEF wrappers and numeric constants in all .ph files
    ✓ Full namespace dispatch in executor.rs for all 13 stdlib namespaces
    ✓ Auto-discovery of stdlib/ in Executor::new()
    ✓ Trig/advanced math builtins: sin, cos, tan, log, gcd, lcm, factorial,
      is_nan, is_inf
    ✓ Additional builtins: import, debug.vars, rand.shuffle, rand.sample,
      math.gcd, math.lcm, math.factorial, math.is_nan, math.is_inf, math.log,
      fs.touch, fs.basename, fs.dirname, fs.ext, time.delta,
      tensor.add/sub/mul/div, type_of
    ✓ debug.backtrace field name fixed: .context (was .label)
    ✓ IMPORT aliases added to alias.rs and documented in grammar.rs
    ✓ Meatball Runtime Agent scaffold in src/meatballs/
    ✓ Virtual filesystem (shell_os/vfs/) integrated
    ✓ pasta_async sub-crate scaffold (src/pasta_async/)
    ✓ Saucey utility module (src/saucey/saucey.rs)
    ✓ 30-section regression suite (tests/09_big_test.ps) all passing
      Root cause fixed: DEDENT before END in DEF bodies dropped function defs
      Fix: End-skip guard in DEF body loop + optional closing match_token(End)
           + same guard in parse_do_body

v1.2 (March 7, 2026)
    ✓ DEF with named parameters — positional arg binding at call site
    ✓ Lambda: lambda x: expr  (multi-param)
    ✓ Lambda calls dispatch through environment before builtins
    ✓ All identifier(args) forms parsed as Call (not ConstructorCall)
    ✓ RET.NOW(): expr — immediate return with ControlFlowSignal unwind
    ✓ RET.LATE(ms): expr — snapshot-now / deliver-later (Value::Pending)
    ✓ resolve(handle) builtin
    ✓ list_flatten() correctly derefs heap-wrapped inner lists
    ✓ do_print() and print() deref heap values before display
    ✓ Lists print as [a, b, c] (single line)
    ✓ ExprStmt returns evaluated value
    ✓ Math, conversion, random, system builtins expanded
    ✓ type() returns "pending" for Value::Pending
    ✓ All 18 advanced test sections passing

v1.1 (March 1, 2026)
    ✓ Fixed AND/OR operator tokenization
    ✓ Added NOT token to TokenType enum
    ✓ Fixed WHILE loop thread ID allocation for lambda targets
    ✓ AI neural network module (ai_network.rs, 285 lines)
    ✓ 10 new AI builtins (ai.relu, ai.softmax, ai.loss.mse, etc.)
    ✓ IF/ELSE AST nodes + executor
    ✓ New comparison operators (Approx, NotEq, StrictEq) in AST
    ✓ Traceback system (TraceFrame, Traceback)
    ✓ Strainer GC integrated
    ✓ ~145 tests passing

================================================================================
28. TODO / ROADMAP
================================================================================

PRIORITY 1 — Language completeness:

    [ ] RET.LATE(trigger_fn()) — implement WhenTrue polling in resolve()
        Needs: poll trigger_fn() in a sleep loop until truthy.

    [ ] RETURN statement — alias for RET.NOW() (Python-style)
        Simple: parse "return expr" → Statement::RetNow { value }

    [ ] Object system execution — OBJ/SPAWN/MUT:
        a. Add Value::Object(u64) to Value enum
        b. Wire Statement::ObjDecl → register_object_family()
        c. Wire Expr::ConstructorCall → instantiate_object()
        d. Wire mutation invocation syntax
        e. Wire field access in eval_expr

    [ ] TRY/OTHERWISE exception handling — executor handler
        (Parser AST nodes already exist; executor stub only)

PRIORITY 2 — Ergonomics:

    [ ] DEF with default parameter values:  DEF f(x, y=0):
    [ ] Variadic functions:  DEF f(args...):
    [ ] Multi-line lambda body (currently single expression only)
    [ ] String interpolation:  "Hello {name}"
    [ ] Dictionary / map type (native, not list-of-pairs)

PRIORITY 3 — Runtime:

    [ ] Autograd builtins exposed as PASTA functions
    [ ] PAUSE / UNPAUSE / RESTART execution control
    [ ] WAIT / AWAIT (pairs with RET.LATE WhenTrue)
    [ ] GROUP thread grouping
    [ ] Import / module system (auto-include stdlib.pa)
    [ ] REPL history (up-arrow recall)
    [ ] pasta_async integration with executor

PRIORITY 4 — Meatball Runtime Agent:

    [ ] Wire run_cli to PASTA Environment / Executor types
    [ ] Implement pseudo-vm and vm backends
    [ ] MRA agent binary (JSON-over-stdio) test suite
    [ ] DiskImages/fs.img VFS mount integration

PRIORITY 5 — Tooling:

    [ ] cargo clippy clean (zero warnings target)
    [ ] Formatter for .pa files
    [ ] LSP / syntax highlighting definitions
    [ ] CLASS keyword full implementation
    [ ] LEARN ML macro

================================================================================
SUPPORT & CONTRIBUTIONS
================================================================================

DEVELOPMENT WORKFLOW:

    cargo build             Build (debug)
    cargo build --release   Build (release)
    cargo test              Run all tests
    cargo test --lib        Unit tests only
    cargo fmt               Format code
    cargo clippy            Lint

CONTRIBUTING:
    - Keep changes small and focused
    - Include tests for new behavior
    - Run cargo test before opening PR
    - Never commit with sudo artifacts in target/
    - Patch scripts use Python str.replace (not line numbers), are idempotent
      with [SKIP] on re-runs, and support --dry-run

================================================================================
END OF PASTALANG UNIFIED REFERENCE
================================================================================
Version:      1.3
Last Updated: March 12, 2026
PASTA — Program for Assignment, Statements, Threading, and Allocation
Contact:      Travis Garrison <Brainboy2487@gmail.com>
================================================================================
