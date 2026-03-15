================================================================================
PASTALANG - UNIFIED COMPREHENSIVE REFERENCE
================================================================================
Version: 1.1 (March 1, 2026)
PASTA: Program for Assignment, Statements, Threading, and Allocation

PASTA is a domain-specific language built in Rust for describing concurrent
threads, priority relationships, constraint expressions, tensor/AI operations,
and general scripting with comprehensive file system support.

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

    # Release build (optimized)
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

MAINTENANCE COMMANDS:

    cargo fmt          # Format code
    cargo clippy       # Lint
    cargo build -v     # Verbose build output

IMPORTANT: Never use sudo cargo — this causes permission issues in target/.
If target/ has permission errors:
    sudo chown -R $USER:$USER target
    rm -rf target && cargo build

================================================================================
3. PROJECT STRUCTURE
================================================================================

    src/
    ├── lib.rs                  Main library exports
    ├── ai/                     AI/ML subsystems
    │   ├── autograd.rs         Automatic differentiation engine
    │   ├── tensor.rs           Tensor utilities
    │   ├── datasets.rs         Data loading utilities
    │   ├── generate.rs         Generation utilities
    │   ├── models.rs           Pre-built model templates
    │   └── tokenizer.rs        Tokenization
    ├── interpreter/            Core interpreter
    │   ├── executor.rs         Statement execution & builtins (100KB+)
    │   ├── environment.rs      Variable storage, scopes, Value enum
    │   ├── repl.rs             Interactive REPL mode
    │   ├── shell.rs            Shell/filesystem operations
    │   ├── ai_network.rs       Neural network backend
    │   ├── errors.rs           RuntimeError, Traceback types
    │   └── mod.rs              Module exports
    ├── lexer/                  Tokenization
    │   ├── lexer.rs            Main lexer with indent/dedent handling
    │   ├── tokens.rs           TokenType enum definitions
    │   ├── alias.rs            Keyword alias table
    │   └── unicode.rs          Unicode support
    ├── parser/                 Syntax analysis
    │   ├── parser.rs           Main parser (precedence climbing)
    │   ├── ast.rs              Abstract Syntax Tree node types
    │   └── grammar.rs          Grammar helpers
    ├── runtime/                Runtime subsystems
    │   ├── devices.rs          Device detection & auto-configure
    │   ├── threading.rs        Thread metadata
    │   ├── scheduler.rs        Task scheduler (feature-gated)
    │   ├── rng.rs              Random number generation (hardware RNG)
    │   ├── asm.rs              Assembly/low-level runtime helpers
    │   ├── bitwise.rs          Bitwise operations
    │   └── strainer.rs         Garbage collector
    ├── semantics/              Semantic analysis
    │   ├── constraints.rs      Constraint engine
    │   ├── priority.rs         Priority graph (directed)
    │   └── resolver.rs         Symbol resolution
    └── utils/                  Utilities
        ├── errors.rs           Error types & diagnostics
        ├── logging.rs          Logging system
        └── helpers.rs          Helper functions

    headers/                    Standard library header files (.ph)
    tests/                      Integration tests
    docs/                       Additional documentation
    examples/                   Example .pasta programs

================================================================================
4. CORE LANGUAGE FEATURES
================================================================================

IMPLEMENTED & WORKING:
    ✓ Variable assignments with type inference
    ✓ Arithmetic operators (+, -, *, /, %)
    ✓ Logical operators (&&, ||, NOT/!)
    ✓ Comparison operators (==, !=, <, >, <=, >=)
    ✓ DO blocks (named threads with optional alias & repeat count)
    ✓ WHILE loops (all variants: named, lambda, multiple targets)
    ✓ Function definitions (DEF keyword)
    ✓ Lambda expressions (anonymous functions)
    ✓ List creation and indexing
    ✓ String literals and manipulation
    ✓ Priority relationships (A OVER B)
    ✓ Constraint expressions (LIMIT OVER)
    ✓ Proper lexical scoping (scope stack)
    ✓ File I/O (read_from_file, write_to_file)
    ✓ Shell & filesystem operations (shell.* namespace)
    ✓ AI/ML tensor operations (tensor.* namespace)
    ✓ Neural network operations (ai.* namespace)
    ✓ REPL (interactive mode)
    ✓ Header file loading (.ph files)
    ✓ Garbage collection (Strainer GC)
    ✓ Traceback / error diagnostics
    ✓ Boolean operators: AND (&&), OR (||), NOT

PARTIALLY IMPLEMENTED (stubs/placeholders):
    ~ IF/ELSE conditional statements (AST nodes defined, executor stub present)
    ~ Unicode operators (≈, ≠, ≡ — tokens defined, not fully wired in lexer)
    ~ FOR loops (separate from DO FOR repetitions)

RESERVED / NOT YET IMPLEMENTED:
    ✗ IF/THEN/OTHERWISE full execution
    ✗ TRY/CATCH exception handling
    ✗ PAUSE/UNPAUSE/RESTART execution control
    ✗ WAIT/AWAIT condition waiting
    ✗ GROUP thread grouping
    ✗ CLASS type definitions
    ✗ LEARN ML macro keyword
    ✗ Backpropagation/gradient descent training loop

================================================================================
5. DATA TYPES
================================================================================

PASTA has 8 value types (Value enum in environment.rs):

NUMBER:         42              Integer (stored as f64)
                3.14            Floating point
                -10.5           Negative

STRING:         "hello"         Double-quoted strings
                "PASTA rocks"

BOOL:           TRUE            (aliases: true, True)
                FALSE           (aliases: false, False)

LIST:           [1, 2, 3]       Homogeneous or mixed
                ["a", "b"]
                [1, "mixed", 3]

TENSOR:         tensor.zeros([2, 3])    2×3 zero matrix
                tensor.ones([5, 5])
                tensor.eye(4)           Identity matrix
                tensor.rand([3, 4])

LAMBDA:         fn = lambda x: x * 2   Anonymous function
                result = fn(5)

NONE:           x = None                Null/undefined value
                x = NONE                (alias)

HEAP:           Internal GC-managed handle type (not user-facing)

TYPE INSPECTION:

    type(42)            # Returns "number"
    type("hello")       # Returns "string"
    type(true)          # Returns "bool"
    type([1,2,3])       # Returns "list"
    type(None)          # Returns "none"

TYPE CONVERSION:

    str(42)             # "42"        - Convert to string
    num("100")          # 100         - Convert to number
    float("3.14")       # 3.14        - Convert to float
    int(3.9)            # 3           - Convert to integer (truncate)
    bool(1)             # true        - Convert to boolean

================================================================================
6. LEXICAL GRAMMAR & TOKENS
================================================================================

WHITESPACE & INDENTATION:
    - Indentation is SIGNIFICANT (spaces only, not tabs).
    - Leading space changes produce Indent/Dedent tokens.
    - Newline separates statements.
    - Lines starting with # are comments.
    - // comments are also supported.

IDENTIFIERS:    [A-Za-z_][A-Za-z0-9_]*

NUMBERS:        \d+(\.\d+)?   (parsed as f64)

STRINGS:        "(?:[^"\\]|\\.)*"  (double-quoted, escape sequences)

COMPLETE TOKEN TABLE:

    Token       Lexeme          Description
    ---------   -----------     ----------------------------------
    Indent      (implicit)      Increase block depth
    Dedent      (implicit)      Decrease block depth
    Newline     \n              Statement separator
    Identifier  [A-Za-z_]...   Variable/function names
    Number      \d+(\.\d+)?    Numeric literal (f64)
    String      "..."           Double-quoted string
    Plus        +               Addition
    Minus       -               Subtraction
    Star        *               Multiplication
    Slash       /               Division
    Percent     %               Modulo
    Eq          =               Assignment
    EqEq        ==              Equality
    Neq         !=              Inequality
    Lt          <               Less than
    Gt          >               Greater than
    Lte         <=              Less or equal
    Gte         >=              Greater or equal
    And         &&              Logical AND
    Or          ||              Logical OR
    Not         !               Logical NOT
    Comma       ,               List / argument separator
    Colon       :               Block header terminator
    LParen      (               Left parenthesis
    RParen      )               Right parenthesis
    LBracket    [               Left bracket (list)
    RBracket    ]               Right bracket
    Do          DO              DO block keyword
    While       WHILE           While loop keyword
    For         FOR             Repeat count keyword
    As          AS              Alias keyword
    End         END             Block terminator
    Def         DEF             Function definition keyword
    Set         set / SET       Assignment keyword
    Over        OVER            Priority operator
    Limit       LIMIT           Constraint keyword
    LimitOver   LIMIT OVER      Combined constraint token
    Print       PRINT / ECHO    Output keyword
    Lambda      lambda          Lambda expression keyword
    If          IF              Conditional (partially implemented)
    Else        ELSE            Else clause (partially implemented)
    True        TRUE/true       Boolean true
    False       FALSE/false     Boolean false
    None        None/NONE       Null value
    Approx      ≈               Approximate equality (token defined)
    NotEq       ≠               Unicode not equal (token defined)
    StrictEq    ≡               Strict equality (token defined)
    Eof         (end)           End of file

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
    IF          when, provided                  Conditional (partial)
    THEN        -                               Conditional (reserved)
    OTHERWISE   else, catch                     Else clause (reserved)
    TRY         attempt                         Exception handling (reserved)
    PAUSE       sleep, hold, suspend            Pause (reserved)
    UNPAUSE     resume, continue                Resume (reserved)
    RESTART     reset, rerun                    Restart (reserved)
    WAIT        await, hold_for                 Wait (reserved)
    GROUP       bundle                          Group threads (reserved)
    CLASS       type, kind                      Type definition (reserved)
    LEARN       build_model, make_net           ML macro (reserved)

BOOLEAN KEYWORDS:

    TRUE        true, True
    FALSE       false, False
    AND         AND (keyword alias for &&)
    OR          OR  (keyword alias for ||)
    NOT         NOT, negate (keyword alias for !)

================================================================================
8. OPERATORS & PRECEDENCE
================================================================================

ARITHMETIC:
    a + b       Addition
    a - b       Subtraction
    a * b       Multiplication
    a / b       Division (float result)
    a % b       Modulo (remainder)

COMPARISON:
    a == b      Equal
    a != b      Not equal
    a < b       Less than
    a > b       Greater than
    a <= b      Less than or equal
    a >= b      Greater than or equal

LOGICAL:
    a && b      AND (both true)
    a || b      OR  (either true)
    NOT a       Negate
    !a          Negate (symbol form)

LIST:
    list[0]     Index access (0-based)
    [1] + [2]   List concatenation (via + operator)

ASSIGNMENT:
    x = expr    Bind value to name

OPERATOR PRECEDENCE (highest to lowest):

    1.  ( )                         Parentheses
    2.  * /                         Multiplicative
    3.  + -                         Additive
    4.  < > <= >= == !=             Comparison
    5.  &&                          Logical AND
    6.  ||                          Logical OR
    7.  =                           Assignment (lowest)

================================================================================
9. STATEMENTS & SYNTAX
================================================================================

── ASSIGNMENT ──────────────────────────────────────────────────────────────────

    x = 10
    SET x = 10           # SET keyword optional
    y = x + 5

── FUNCTION DEFINITION (DEF) ───────────────────────────────────────────────────

    DEF function_name(param1, param2):
        # body
        PRINT param1 + param2
    END

    DEF greet(name):
        msg = "Hello, " name
        PRINT msg
    END

    greet("World")

    # Alternative form (no-arg, with DO)
    DEF calculate DO
        result = 3.14 * 5 * 5
        PRINT result
    END

    DO calculate FOR 1:
    END

── DO BLOCK ────────────────────────────────────────────────────────────────────

    Syntax:
        DO target [AS alias] [FOR repeats] :
            statements
        END

    # Execute named block 3 times
    DO worker FOR 3:
        PRINT "Working..."
    END

    # With alias
    DO processor AS p FOR 5:
        x = x + 1
    END

    # Inline (lambda target)
    DO :
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

    Syntax:
        DO target WHILE condition :
            statements
        END

    # Named target while
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

    # Multiple targets while
    n = 0
    DO a, b WHILE n < 2:
        PRINT n
        n = n + 1
    END

    # Store as lambda and invoke
    counter = 0
    my_loop = DO print_counter WHILE counter < 3:
        counter = counter + 1
    END
    DO my_loop FOR 1:
    END

    ITERATION LIMIT: Default 1,000,000 per target.
    Override: executor.set_while_limit(n)  or  while_limit = n  in code.

── PRINT / ECHO ────────────────────────────────────────────────────────────────

    PRINT x                 # Print single value
    PRINT "Hello"           # Print string
    PRINT x + 5             # Print expression result
    PRINT "Value: " x       # Print label + value (space-separated)
    ECHO x                  # Alias for PRINT
    println(x)              # Function form

── PRIORITY OVERRIDE ───────────────────────────────────────────────────────────

    A OVER B                # A has higher priority than B
    task_a OVER task_b
    critical OVER background

── CONSTRAINT EXPRESSION ───────────────────────────────────────────────────────

    expr1 [relation] expr2 LIMIT OVER constraint_expr

    velocity distance LIMIT OVER time
    speed < max_speed LIMIT OVER engine_power
    temperature level LIMIT OVER cooling_capacity

── IF/ELSE (PARTIAL — stub implementation) ─────────────────────────────────────

    if condition
        statements
    else
        statements
    end

    NOTE: AST nodes are defined and executor stub is in place,
    but full conditional execution is not yet complete. See TODO.txt.

── LAMBDA EXPRESSION ───────────────────────────────────────────────────────────

    double = lambda x: x * 2
    result = double(21)

    add = lambda a, b: a + b
    sum = add(10, 20)

================================================================================
10. EXPRESSIONS
================================================================================

BINARY EXPRESSIONS (parsed with precedence climbing):

    2 + 3 * 4           # 14 (not 20)
    (2 + 3) * 4         # 20
    10 - 5 + 2          # 7
    x > 5 && y < 10     # boolean AND
    (x + y) > 12 && x < 20

PRIMARY EXPRESSIONS:

    42                  # Number literal
    "hello"             # String literal
    true / false        # Boolean literals
    [1, 2, 3]           # List literal
    x                   # Identifier
    (expr)              # Parenthesized
    f(a, b)             # Function call
    list[0]             # Index access

UNARY:

    -x                  # Negate number
    NOT x               # Logical NOT
    !x                  # Logical NOT (symbol)

================================================================================
11. SCOPING & ENVIRONMENT
================================================================================

SCOPE STACK MODEL:
    - Variables are stored in a stack of HashMaps.
    - Each DO block and function call pushes a new scope on entry.
    - Scope is popped on exit.
    - Assignment writes to the INNERMOST (current) scope.
    - Variable lookup searches from innermost scope outward, then globals.

SCOPE RULES:

    x = 10              # Global (top-level) scope

    DO outer:
        x = 20          # Local to outer, shadows global x
        DO inner:
            x = 30      # Local to inner, shadows outer x
        END
        # x here is still 20 (outer's copy)
    END
    # x here is still 10 (global unchanged)

GLOBALS vs LOCALS:
    - Top-level code writes to globals.
    - assign() prefers scopes.last_mut(); falls back to globals.
    - set_local() explicitly writes to current scope.

THREAD METADATA:
    - DO blocks register thread entries (id, name, alias, priority weight).
    - Logical threads — not OS threads unless scheduler feature is enabled.

================================================================================
12. BUILT-IN FUNCTIONS (COMPLETE REFERENCE)
================================================================================

── TYPE & CONVERSION ───────────────────────────────────────────────────────────

    type(value)         Returns type name string
                        "number" | "string" | "bool" | "list" | "tensor" |
                        "lambda" | "none"

    str(value)          Convert any value to string representation
    num(value)          Parse string/bool to number
    float(value)        Convert to float (same as num)
    int(value)          Convert to integer (truncates f64 to i64)
    bool(value)         Convert to boolean

── MATH ─────────────────────────────────────────────────────────────────────────

    abs(x)              Absolute value
    sqrt(x)             Square root
    pow(base, exp)      Power: base ^ exp
    floor(x)            Floor (round down)
    ceil(x)             Ceiling (round up)
    round(x)            Round to nearest integer
    min(a, b)           Minimum of two numbers
    max(a, b)           Maximum of two numbers
    clamp(v, lo, hi)    Clamp value between lo and hi
    sign(x)             Sign: 1, -1, or 0

    # Via headers (src/pasta_G.ph):
    __pasta_math_sin(x)     Sine
    __pasta_math_cos(x)     Cosine
    __pasta_math_tan(x)     Tangent

── RANDOM ──────────────────────────────────────────────────────────────────────

    rand()              Random float in [0, 1)
    rand_int(lo, hi)    Random integer in [lo, hi]
    rand_range(lo, hi)  Random float in [lo, hi)

── LIST OPERATIONS ──────────────────────────────────────────────────────────────

    len(collection)     Length of list, string, or tensor element count
    length(collection)  Alias for len
    append(list, value) Append value to list, returns new list
    push(list, value)   Alias for append
    pop(list)           Remove and return last element
    head(list)          First element
    tail(list)          All but first element
    reverse(list)       Reverse list
    sort(list)          Sort list (numbers or strings)
    contains(list, v)   Boolean: does list contain v?
    index_of(list, v)   Index of v in list (-1 if not found)
    zip(list1, list2)   Zip two lists into list of pairs
    range(n)            Generate [0, 1, 2, ..., n-1]
    range(start, end)   Generate [start, ..., end-1]

── STRING OPERATIONS ────────────────────────────────────────────────────────────

    len(s)              String length
    str(v)              Any value to string
    concat(s1, s2)      Concatenate strings
    upper(s)            Uppercase (via stdlib)
    lower(s)            Lowercase (via stdlib)
    trim(s)             Trim whitespace
    split(s, delim)     Split string by delimiter
    join(list, delim)   Join list of strings
    starts_with(s, p)   Prefix check
    ends_with(s, suf)   Suffix check
    contains(s, sub)    Substring check
    replace(s, old, new) Replace occurrences
    reverse(s)          Reverse string (via stdlib)

── PRINT / OUTPUT ───────────────────────────────────────────────────────────────

    PRINT value         Print value + newline
    ECHO value          Alias for PRINT
    print(value)        Function form
    println(value)      Print with explicit newline

── SYSTEM / ENVIRONMENT ─────────────────────────────────────────────────────────

    pwd()               Get current working directory (string)
    exit(code)          Exit interpreter with code
    sleep(ms)           Sleep for ms milliseconds
    time()              Current Unix timestamp (float seconds)
    env(name)           Get environment variable value

── GRAPHICS (feature: image) ────────────────────────────────────────────────────

    canvas_new(w, h, name)      Create canvas
    canvas_set(name, x, y, r, g, b)  Set pixel
    canvas_save(name, path)     Save as PNG
    canvas_fill(name, r, g, b)  Fill canvas with color

================================================================================
13. FILE I/O OPERATIONS
================================================================================

READ FILE:

    read_from_file(path)        Read file as list of byte values (0-255)
    rff(path)                   Short alias

    bytes = read_from_file("/tmp/data.txt")
    PRINT len(bytes)

WRITE FILE:

    write_to_file(filename, data)
    write_to_file(filename, data, output_directory)
    wtf(filename, data)         Short alias

    Parameters:
        filename    - String file name
        data        - String or list of byte numbers (0-255)
        output_dir  - Optional output path (default ".")

    write_to_file("output.txt", "Hello, World!")
    write_to_file("report.txt", "Data", "/tmp/reports")
    bytes = [72, 101, 108, 108, 111]
    write_to_file("binary.bin", bytes)

COMBINED PATTERN:

    data = read_from_file("input.txt")
    PRINT "Read " len(data) " bytes"
    write_to_file("output.txt", data, "results/")

================================================================================
14. SHELL & FILESYSTEM OPERATIONS
================================================================================

All shell functions are accessible as shell.function() or plain function().

DIRECTORY:

    shell.pwd()                     Current working directory
    pwd()

    shell.cd(path)                  Change directory
    cd(path)                        Supports: absolute, relative, "~", "..", "."

    shell.ls()                      List current directory → List of names
    shell.ls(path)                  List specific directory
    ls(path)

    shell.ls_long(path)             Detailed listing → ["name size type", ...]

    shell.mkdir(path)               Create directory
    shell.mkdir(path, true)         Create with parent directories
    mkdir(path, parents)

FILE OPERATIONS:

    shell.touch(path)               Create empty file or update timestamp
    touch(path)

    shell.cp(from, to)              Copy file
    cp(from, to)

    shell.mv(from, to)              Move or rename file
    mv(from, to)

    shell.rm(path)                  Delete file (not directories)
    rm(path)

    shell.rmdir(path)               Delete empty directory
    rmdir(path)

    shell.rmdir_r(path)             Recursively delete directory + contents
    shell.rmdir_recursive(path)     Long form alias
    rm_r(path)

FILE INFO:

    shell.exists(path)              Boolean: does path exist?
    shell.is_file(path)             Boolean: is it a regular file?
    shell.is_dir(path)              Boolean: is it a directory?
    shell.file_size(path)           Size in bytes (0 if not found)
    shell.realpath(path)            Absolute/canonical path

FILESYSTEM EXAMPLE:

    shell.mkdir("/tmp/backups", true)
    if shell.exists("/tmp/important.txt")
        shell.cp("/tmp/important.txt", "/tmp/backups/important.txt.bak")
        size = shell.file_size("/tmp/backups/important.txt.bak")
        PRINT "Backup created, size: " size
    end

mv(from, to)

    shell.rm(path)                  Delete file (not directories)
    rm(path)

    shell.rmdir(path)               Delete empty directory
    rmdir(path)

    shell.rmdir_r(path)             Recursively delete directory + contents
    shell.rmdir_recursive(path)     Long form alias
    rm_r(path)

FILE INFO:

    shell.exists(path)              Boolean: does path exist?
    shell.is_file(path)             Boolean: is it a regular file?
    shell.is_dir(path)              Boolean: is it a directory?
    shell.file_size(path)           Size in bytes (0 if not found)
    shell.realpath(path)            Absolute/canonical path

FILESYSTEM EXAMPLE:

    shell.mkdir("/tmp/backups", true)
    if shell.exists("/tmp/important.txt")
        shell.cp("/tmp/important.txt", "/tmp/backups/important.txt.bak")
        size = shell.file_size("/tmp/backups/important.txt.bak")
        PRINT "Backup created, size: " size
    end


================================================================================
15. AI/ML & TENSOR OPERATIONS
================================================================================

── TENSOR CREATION ──────────────────────────────────────────────────────────────

    tensor.zeros([rows, cols])      Zero matrix
    tensor.ones([rows, cols])       Ones matrix
    tensor.eye(n)                   n×n identity matrix
    tensor.rand([rows, cols])       Random matrix (uniform)
    tensor.from_list([1, 2, 3])     Create from list
    tensor.from_list(list)          1D tensor from PASTA list

    zeros = tensor.zeros([3, 3])
    ones = tensor.ones([2, 4])
    identity = tensor.eye(5)
    random = tensor.rand([10, 10])
    vec = tensor.from_list([1.0, 2.5, 3.14])

── TENSOR INSPECTION ────────────────────────────────────────────────────────────

    tensor.shape(t)         Returns shape as list, e.g. [3, 3]
    tensor.dtype(t)         Returns data type string, e.g. "float32"
    tensor.sum(t)           Sum of all elements
    tensor.mean(t)          Mean of all elements

── TENSOR MANIPULATION ──────────────────────────────────────────────────────────

    tensor.reshape(t, new_shape)    Reshape tensor
    tensor.transpose(t)             Transpose 2D tensor
    tensor.flatten(t)               Flatten to 1D tensor

── TENSOR CONVERSION ────────────────────────────────────────────────────────────

    tensor.from_list(list)          List → tensor
    ai.list_to_tensor(list)         Alias
    ai.tensor_to_list(tensor)       Tensor → list

── NEURAL NETWORK OPERATIONS ────────────────────────────────────────────────────

    ai.linear(in_dim, out_dim)
        Create fully-connected linear layer (Xavier-style init)
        Example: layer = ai.linear(784, 128)

    ai.mlp([input, hidden..., output])
        Create multi-layer perceptron
        Automatically inserts ReLU between layers
        Example: net = ai.mlp([784, 256, 128, 10])

    ai.relu(tensor)
        Rectified Linear Unit: max(0, x) elementwise
        Example: activated = ai.relu(logits)

    ai.softmax(tensor)
        Softmax: probabilities summing to 1.0
        Example: probs = ai.softmax(scores)

── LOSS FUNCTIONS ───────────────────────────────────────────────────────────────

    ai.loss.mse(pred, target)
        Mean Squared Error (regression tasks)
        Returns single scalar loss value
        Example: loss = ai.loss.mse(predictions, ground_truth)

    ai.loss.crossentropy(logits, target_class)
        Cross-entropy loss for classification
        target_class: integer class index
        Returns single scalar loss value
        Example: loss = ai.loss.crossentropy(logits, 2)

── AI FUNCTION NAMING ───────────────────────────────────────────────────────────

    Functions accessible both as:
        ai.relu(t)          Dot notation
        ai_relu(t)          Underscore notation

── AUTOGRAD ENGINE (src/ai/autograd.rs) ─────────────────────────────────────────

    The autograd module provides forward/backward autodiff:
    - Tensor type with optional gradient tracking (requires_grad)
    - Supported ops: add, sub, mul, div, neg, sum, mean, relu, powf, matmul
    - backward() accumulates gradients via reverse-mode autodiff
    - Currently at library level — not yet exposed as PASTA builtins

── COMPLETE AI EXAMPLE ──────────────────────────────────────────────────────────

    # 1. Create input
    x = tensor.from_list([1.0, 2.0, 3.0])
    y = tensor.from_list([1.1, 2.1, 2.9])

    # 2. Compute loss
    loss = ai.loss.mse(x, y)
    PRINT loss                          # ~0.01

    # 3. ReLU activation
    logits = tensor.from_list([1.0, -0.5, 2.0, -1.0])
    activated = ai.relu(logits)
    PRINT ai.tensor_to_list(activated)  # [1, 0, 2, 0]

    # 4. Create network
    net = ai.mlp([2, 4, 3])
    PRINT net                           # "ai.MLP: linear_0 -> relu_0 -> linear_1"

    # 5. Softmax
    logits = tensor.from_list([1.0, 2.0, 3.0])
    probs = ai.softmax(logits)
    ce_loss = ai.loss.crossentropy(logits, 2)
    PRINT ce_loss

================================================================================
16. PRIORITY & CONSTRAINT SYSTEM
================================================================================

PRIORITY RELATIONSHIPS:

    A OVER B            # Adds directed edge A → B to priority graph
    urgent OVER routine
    process_a OVER process_b
    process_b OVER process_c

    # This creates: process_a > process_b > process_c

The interpreter maintains a directed priority graph (PriorityGraph).
Cycles are detected and reported as diagnostics.
Used by scheduler (if enabled) for topological ordering.

CONSTRAINT EXPRESSIONS:

    Syntax:
        expr1 [relation] expr2 LIMIT OVER constraint_expr

    velocity distance LIMIT OVER time
    speed < max_speed LIMIT OVER engine_power
    temperature level LIMIT OVER cooling_capacity

The ConstraintEngine collects constraints and produces diagnostics:
    Satisfiable
    Unsatisfiable
    RequiresOptimization
    Error

================================================================================
17. STANDARD LIBRARY (stdlib) REFERENCE
================================================================================

The PASTA Standard Library provides 180+ functions in 19 modules.
Load via:  include stdlib.pa   (auto-load planned for future versions)

MODULE 1: UTILITY FUNCTIONS
    assert(condition, message)      Assert condition, print message if false
    range(n)                        [0..n-1]
    range(start, end)               [start..end-1]
    repeat(value, count)            List with value repeated count times
    is_empty(collection)            Boolean: is list/string empty?
    is_null(value)                  Boolean: is value NONE?
    not_null(value)                 Boolean: is value not NONE?
    identity(x)                     Return x unchanged
    const(value)                    Create constant-returning lambda

MODULE 2: STRING UTILITIES
    string_length(s)                String length
    string_concat(s1, s2)           Concatenate
    string_concat_with_sep(s1,s2,sep)  With separator
    string_repeat(s, count)         Repeat string
    string_pad_left(s, w, char)     Left-pad to width
    string_pad_right(s, w, char)    Right-pad to width
    string_upper(s)                 Uppercase (stub)
    string_lower(s)                 Lowercase (stub)
    string_reverse(s)               Reverse (stub)
    string_starts_with(s, prefix)   Prefix check (stub)
    string_ends_with(s, suffix)     Suffix check (stub)
    string_contains(s, sub)         Substring check (stub)
    string_split(s, delim)          Split by delimiter (stub)
    string_join(list, delim)        Join with delimiter (stub)
    string_replace(s, old, new)     Replace occurrences (stub)
    string_trim(s)                  Trim whitespace (stub)
    string_to_chars(s)              String → list of characters (stub)
    chars_to_string(chars)          List of chars → string (stub)

    NOTE: Functions marked (stub) are defined in stdlib.pa but rely on
    PASTA language primitives. Full native Rust implementations planned.

MODULE 3: LIST OPERATIONS
    list_first(lst)                 First element or NONE
    list_last(lst)                  Last element or NONE
    list_take(lst, n)               First n elements
    list_drop(lst, n)               All after first n
    list_slice(lst, start, end)     Slice [start, end)
    list_reverse(lst)               Reversed copy
    list_concat(lst1, lst2)         Concatenation
    list_flatten(nested)            One-level flatten
    list_unique(lst)                Unique elements (stub)
    list_count(lst, value)          Count occurrences
    list_index_of(lst, value)       Index or -1
    list_contains(lst, value)       Boolean
    list_sum(lst)                   Sum of numbers
    list_average(lst)               Average
    list_min(lst)                   Minimum
    list_max(lst)                   Maximum
    list_map(lst, fn)               Transform elements
    list_filter(lst, fn)            Filter by predicate
    list_reduce(lst, acc, fn)       Reduce to single value

MODULE 4: MATH UTILITIES
    math_abs(x)                     Absolute value
    math_min(a, b)                  Minimum
    math_max(a, b)                  Maximum
    math_clamp(v, lo, hi)           Clamp
    math_sign(x)                    Sign: 1, -1, 0
    math_is_even(n)                 Boolean: even?
    math_is_odd(n)                  Boolean: odd?
    math_is_positive(x)             Boolean: > 0?
    math_is_negative(x)             Boolean: < 0?
    math_is_zero(x)                 Boolean: == 0?
    math_factorial(n)               n!
    math_power(base, exp)           base^exp
    math_gcd(a, b)                  Greatest common divisor
    math_lcm(a, b)                  Least common multiple

MODULE 5: FILE UTILITIES
    file_read(path)                 Read file → bytes list
    file_write(path, data)          Write data to file
    file_append(path, data)         Append to file
    file_exists(path)               Boolean
    file_delete(path)               Delete file
    file_size(path)                 Size in bytes
    file_is_readable(path)          Boolean: readable?
    file_is_writable(path)          Boolean: writable?

MODULE 6: DIRECTORY UTILITIES
    dir_exists(path)                Boolean
    dir_create(path)                Create directory
    dir_create_recursive(path)      Create with parents
    dir_list(path)                  List contents → list
    dir_delete(path)                Delete empty dir
    dir_delete_recursive(path)      Delete recursively

MODULE 7: FILESYSTEM UTILITIES
    fs_copy(from, to)               Copy file
    fs_move(from, to)               Move/rename
    fs_path_join(parts)             Join path components
    fs_basename(path)               File name from path
    fs_dirname(path)                Directory from path
    fs_extension(path)              File extension

MODULE 8: TYPE CHECKING
    is_number(v)                    Boolean
    is_string(v)                    Boolean
    is_bool(v)                      Boolean
    is_list(v)                      Boolean
    is_tensor(v)                    Boolean
    is_lambda(v)                    Boolean
    is_none(v)                      Boolean

MODULE 9: CONTROL FLOW UTILITIES
    while_limited(cond_fn, body_fn, max_iters)  Safe bounded loop
    do_n_times(n, fn)                           Execute fn n times
    retry(fn, max_attempts)                      Retry on failure

MODULE 10: PRIORITY & CONCURRENCY
    priority_set(a, b)              Set A OVER B
    priority_chain(list)            Chain multiple priorities
    get_thread_id()                 Current thread ID
    get_thread_name()               Current thread name

MODULE 11: VALIDATION UTILITIES
    validate_number(v, name)        Assert v is number
    validate_string(v, name)        Assert v is string
    validate_list(v, name)          Assert v is list
    validate_range(v, lo, hi, name) Assert v in range
    validate_positive(v, name)      Assert v > 0
    validate_not_empty(v, name)     Assert not empty

MODULE 12: FORMATTING UTILITIES
    format_number(n, decimals)      Format with decimal places
    format_percent(v)               Format as percentage
    format_list(lst)                Pretty-print list
    format_table(headers, rows)     ASCII table
    format_pad_center(s, width)     Center-padded string

MODULE 13: TENSOR UTILITIES
    tensor_create_zeros(rows, cols) Zero matrix
    tensor_create_ones(rows, cols)  Ones matrix
    tensor_create_eye(n)            Identity matrix
    tensor_create_rand(rows, cols)  Random matrix
    tensor_from_list(lst)           List → tensor
    tensor_to_list(t)               Tensor → list
    tensor_shape(t)                 Shape as list
    tensor_sum(t)                   Sum all elements
    tensor_mean(t)                  Mean
    tensor_add(a, b)                Elementwise addition
    tensor_scale(t, scalar)         Scale all elements
    tensor_dot(a, b)                Dot product
    tensor_reshape(t, shape)        Reshape
    tensor_transpose(t)             Transpose

MODULE 14: AI/ML UTILITIES
    ai_create_linear(in, out)       Linear layer
    ai_create_mlp(dims)             Multi-layer perceptron
    ai_relu(t)                      ReLU activation
    ai_softmax(t)                   Softmax
    ai_mse_loss(pred, target)       MSE loss
    ai_crossentropy(logits, class)  Cross-entropy loss
    ai_forward(net, input)          Forward pass
    ai_predict(net, input)          Predict (argmax of softmax)

MODULE 15: DATA PROCESSING
    data_normalize(lst)             Normalize to [0, 1]
    data_standardize(lst)           Standardize (z-score)
    data_shuffle(lst)               Shuffle list
    data_split(lst, ratio)          Train/test split
    data_batch(lst, size)           Create batches

MODULE 16: BENCHMARK & TIMING
    bench_start()                   Record start time
    bench_end(start)                Time elapsed in ms
    bench_run(fn, iterations)       Benchmark function

MODULE 17: LOGGING & DEBUG
    log_info(msg)                   Log INFO message
    log_warn(msg)                   Log WARN message
    log_error(msg)                  Log ERROR message
    log_debug(msg)                  Log DEBUG message
    debug_dump(name, value)         Print name: value

MODULE 18: COLLECTION UTILITIES
    set_create()                    Create empty set (as list)
    set_add(s, v)                   Add to set
    set_contains(s, v)              Membership check
    set_union(s1, s2)               Union
    set_intersect(s1, s2)           Intersection
    set_difference(s1, s2)          Difference
    dict_create()                   Create empty dict (as list of pairs)
    dict_set(d, key, value)         Set key-value
    dict_get(d, key)                Get value for key
    dict_keys(d)                    All keys
    dict_values(d)                  All values

MODULE 19: FUNCTIONAL UTILITIES
    compose(f, g)                   Function composition: f(g(x))
    curry(f, arg)                   Partial application
    pipe(value, fns)                Pipeline: value → f1 → f2 → ...
    memoize(fn)                     Memoized function (stub)
    once(fn)                        Execute only once (stub)

================================================================================
18. REPL & INTERACTIVE MODE
================================================================================

Start interactive REPL:

    ./target/release/pasta         # No arguments starts REPL
    ./target/release/pasta --repl  # Explicit REPL flag

REPL COMMANDS:
    :help               Show available commands
    :keywords           List all language keywords + AI functions
    :quit / :exit       Exit REPL
    :clear              Clear screen

REPL FEATURES:
    - Full language support (all statements work interactively)
    - History navigation
    - Headers auto-loaded on startup
    - AI & tensor builtins available immediately

REPL EXAMPLE SESSION:

    pasta> x = 10
    pasta> y = x * 2
    pasta> PRINT y
    20
    pasta> net = ai.mlp([3, 4, 2])
    pasta> PRINT net
    ai.MLP: linear_0 -> relu_0 -> linear_1
    pasta> :quit

================================================================================
19. CLI USAGE
================================================================================

RUN PROGRAM FILE:

    ./target/release/pasta program.pa
    ./target/debug/pasta program.pa

INLINE EVALUATION:

    ./target/release/pasta --eval 'x = 42\nPRINT x\n'
    ./target/release/pasta --eval 'PRINT "Hello"\nPRINT 1 + 2'

FLAGS:

    Flag            Purpose                 Example
    -----------     ----------------------- ---------------------------
    --eval TEXT     Evaluate inline code    pasta --eval 'x = 1'
    --help          Show usage              pasta --help
    --version       Show version            pasta --version
    --repl          Start REPL explicitly   pasta --repl

EXAMPLES:

    # Run file
    ./target/release/pasta my_script.pa

    # Run inline
    ./target/release/pasta --eval 'PRINT 123'

    # Multi-line inline
    ./target/release/pasta --eval 'x = 10\ny = 20\nPRINT x + y'

    # With timeout (prevent infinite loops)
    timeout 10 ./target/release/pasta long_program.pa

    # Debug mode
    RUST_BACKTRACE=1 ./target/release/pasta program.pa

================================================================================
20. EXAMPLES
================================================================================

EXAMPLE 1: Hello World

    message = "Hello, World!"
    PRINT message
    # Output: Hello, World!

EXAMPLE 2: Arithmetic

    x = 10
    y = 5
    PRINT "Sum: " x + y
    PRINT "Product: " x * y
    PRINT "Division: " x / y
    # Output: Sum: 15 / Product: 50 / Division: 2

EXAMPLE 3: Lists

    numbers = [1, 2, 3, 4, 5]
    PRINT len(numbers)          # 5
    PRINT numbers[0]            # 1
    PRINT numbers[4]            # 5

EXAMPLE 4: Functions

    DEF multiply(a, b):
        result = a * b
        PRINT result
    END

    multiply(6, 7)              # 42
    multiply(10, 20)            # 200

EXAMPLE 5: While Loop

    counter = 0
    DO loop WHILE counter < 3:
        PRINT "Iteration " counter
        counter = counter + 1
    END
    # Output: Iteration 0 / Iteration 1 / Iteration 2

EXAMPLE 6: Lambda Expressions

    double = lambda x: x * 2
    add = lambda a, b: a + b
    PRINT double(21)            # 42
    PRINT add(10, 20)           # 30

EXAMPLE 7: Boolean Logic

    x = 10
    y = 5
    result1 = x > 5 && y < 10   # true
    result2 = x == 10 || y == 5  # true
    result3 = NOT (x == 5)       # true
    PRINT result1
    PRINT result2
    PRINT result3

EXAMPLE 8: DO Block Repetition

    counter = 0
    DO worker FOR 3:
        PRINT "Working " counter
        counter = counter + 1
    END
    # Output: Working 0 / Working 1 / Working 2

EXAMPLE 9: Nested Scoping

    x = 100

    DO outer FOR 1:
        x = 200
        PRINT x                 # 200 (outer scope)
        DO inner FOR 1:
            x = 300
            PRINT x             # 300 (inner scope)
        END
        PRINT x                 # 200 (outer scope restored)
    END

    PRINT x                     # 100 (global unchanged)

EXAMPLE 10: File I/O

    write_to_file("data.txt", "Hello from PASTA!")
    content = read_from_file("data.txt")
    PRINT "Read " len(content) " bytes"

EXAMPLE 11: Filesystem Operations

    shell.mkdir("/tmp/project", true)
    shell.touch("/tmp/project/file.txt")
    files = shell.ls("/tmp/project")
    PRINT "Files: " files
    if shell.is_file("/tmp/project/file.txt")
        PRINT "It's a file!"
    end
    size = shell.file_size("/tmp/project/file.txt")
    PRINT "Size: " size " bytes"

EXAMPLE 12: Tensor Operations

    matrix = tensor.zeros([3, 3])
    shape = tensor.shape(matrix)
    PRINT "Shape: " shape            # [3, 3]
    identity = tensor.eye(3)
    PRINT tensor.sum(identity)       # 3

EXAMPLE 13: Neural Network

    net = ai.mlp([784, 256, 128, 10])
    PRINT net

    logits = tensor.from_list([1.0, 2.0, 3.0, 0.5])
    activated = ai.relu(logits)
    probs = ai.softmax(activated)
    PRINT ai.tensor_to_list(probs)

    x = tensor.from_list([1.0, 2.0, 3.0])
    y = tensor.from_list([1.1, 2.1, 2.9])
    loss = ai.loss.mse(x, y)
    PRINT "Loss: " loss

EXAMPLE 14: Priority System

    critical OVER background
    process_a OVER process_b
    process_b OVER process_c
    # Registers: critical→background, a→b, b→c in priority graph

EXAMPLE 15: Multi-line While with Lambda

    i = 0
    DO WHILE i < 5:
        PRINT i
        i = i + 1
    END

EXAMPLE 16: Combined Workflow (ML + Filesystem)

    shell.mkdir("/tmp/ml_data", true)
    data = tensor.rand([100, 10])
    net = ai.mlp([10, 64, 32, 1])
    write_to_file("model_info.txt", "MLP: 10 -> 64 -> 32 -> 1")
    if shell.exists("model_info.txt")
        PRINT "Setup complete!"
    end

================================================================================
21. TESTING & DEBUGGING
================================================================================

RUNNING TESTS:

    cargo test                  # All tests
    cargo test --lib            # Unit tests only
    RUST_BACKTRACE=1 cargo test <testname> --lib -- --nocapture
    cargo test -- --nocapture   # All tests with output visible

CURRENT TEST STATUS (as of March 1, 2026):
    - Lexer tests: 22 passing
    - While loop integration: 4 passing
    - AI network module: 6 passing
    - Total: ~145 tests passing

TEST LOCATIONS:
    src/lexer/lexer.rs          Lexer unit tests
    src/parser/parser.rs        Parser unit tests
    src/interpreter/executor.rs Executor + builtin tests
    src/interpreter/ai_network.rs AI module tests
    src/runtime/                Runtime unit tests

DEBUGGING TECHNIQUES:

    1. Enable backtraces:
       RUST_BACKTRACE=1 ./target/release/pasta program.pa
       RUST_BACKTRACE=full ./target/release/pasta program.pa

    2. Print debugging in PASTA:
       PRINT "Debug: variable = " x
       PRINT "Type: " type(x)

    3. File/filesystem debugging:
       if shell.exists("file.txt")
           PRINT "File exists"
       end

    4. Check tensor shapes:
       shape = tensor.shape(matrix)
       PRINT shape

    5. Rust-side debug prints:
       Add eprintln! in Environment::assign, push_scope, pop_scope,
       and Executor::DoBlock to trace runtime state.

    6. Python harness (for automated testing with timeouts):
       Writes test_logs.txt with timestamps, stdout/stderr, return codes.

COMMON DIAGNOSTICS:
    - "Undefined variable": check spelling, check scope
    - "Unexpected token": check indentation, colons, END keywords
    - "Iteration limit exceeded": WHILE loop ran > 1,000,000 times
    - "Type error": use type(x) to inspect values
    - "Permission denied": chown target/ and rebuild

BUILD VERBOSITY:

    cargo build -v
    cargo build --release -v
    RUST_LOG=debug cargo run

================================================================================
22. PERFORMANCE & LIMITATIONS
================================================================================

STRENGTHS:
    - Rapid scripting and prototyping
    - AI/ML experimentation without external deps
    - System automation via shell operations
    - Priority and constraint specification
    - Tensor operations (CPU, row-major Vec<f64>)

LIMITATIONS:
    - Pure tree-walking interpreter (no JIT, no bytecode)
    - Execution is single-threaded (threading is logical/simulated)
    - WHILE loops capped at 1,000,000 iterations by default
    - No lazy evaluation
    - String operations are stub-level in stdlib (pattern-matched in Rust needed)

OPTIMIZATION TIPS:
    1. Use vectorized tensor operations over manual loops
    2. Minimize function call overhead in tight WHILE loops
    3. Batch filesystem operations
    4. Use release build (cargo build --release) for 10x+ speedup

================================================================================
23. TROUBLESHOOTING
================================================================================

ISSUE: "Cannot find file"
    Solution: Use absolute paths or check current dir
    PRINT shell.pwd()

ISSUE: Permission denied on build
    Solution:
        sudo chown -R $USER:$USER target
        rm -rf target

ISSUE: Parser errors about missing token variants
    Solution: Ensure TokenType::Variant used with full path in match arms

ISSUE: Directory not empty on rmdir
    Solution: Use shell.rmdir_r(path) for recursive delete

ISSUE: Type mismatch errors
    Solution: PRINT type(x) to inspect value type

ISSUE: Tensor shape mismatch
    Solution: PRINT tensor.shape(t) before operations

ISSUE: Program hangs (infinite loop)
    Solution: Check WHILE condition. Limit is 1,000,000 per target.
    Workaround: timeout 5 ./target/release/pasta program.pa
    Or: while_limit = 1000 (set lower limit in code)

ISSUE: CLI hangs
    Solution: Run with timeout or Python harness.
    Add eprintln! in executor to find blocking operation.

ISSUE: Headers not loading
    Solution: Run from project root. Headers expected at headers/ or src/*.ph
    Check diagnostics array for load error messages.

================================================================================
24. ARCHITECTURE OVERVIEW
================================================================================

EXECUTION PIPELINE:

    Source Code (.pa file)
         │
         ▼
    ┌─────────────┐
    │    LEXER     │  src/lexer/lexer.rs
    │  Tokenizes   │  Produces: Indent/Dedent, all tokens
    └──────┬──────┘
           │ Vec<Token>
           ▼
    ┌─────────────┐
    │   PARSER    │  src/parser/parser.rs
    │  Builds AST │  Precedence climbing for expressions
    └──────┬──────┘
           │ Program (AST)
           ▼
    ┌─────────────┐
    │  EXECUTOR   │  src/interpreter/executor.rs
    │ Walks AST   │  Manages: Environment, PriorityGraph, ConstraintEngine
    │ Executes    │  Shell, RNG, GC, Traceback
    └──────┬──────┘
           │
           ├─── Environment (scope stack + globals)
           ├─── PriorityGraph (directed edges A→B)
           ├─── ConstraintEngine (constraint validation)
           ├─── Shell (filesystem ops)
           ├─── Strainer GC (heap reference counting)
           ├─── Rng (hardware or software RNG)
           └─── ai_network (neural network runtime)

VALUE ENUM (environment.rs):
    Number(f64)
    Str(String)
    Bool(bool)
    List(Vec<Value>)
    Tensor(RuntimeTensor)
    Lambda { params, body, closure }
    None
    Heap(HeapId)         ← GC-managed reference

AST NODE TYPES (ast.rs):
    Statement:
        Assignment { name, value }
        DoBlock { targets, alias, repeats, body, condition }
        WhileLoop { targets, condition, body }
        FunctionDef { name, params, body }
        PriorityOverride { lhs, rhs }
        Constraint { lhs, relation, rhs, limit }
        ExprStmt { expr }
        If { condition, then_body, else_body }   ← partial
        PrintStmt { values }
        Return { value }
        SetLocal { name, value }

    Expr:
        Number(f64)
        Str(String)
        Bool(bool)
        Identifier(String)
        Binary { op, lhs, rhs }
        Unary { op, operand }
        List(Vec<Expr>)
        Index { base, index }
        Call { callee, args }
        Lambda { params, body }
        Raw(Value)

GARBAGE COLLECTOR (Strainer):
    - Simple mark-and-sweep on heap-allocated values.
    - GC runs automatically after each top-level statement.
    - Manual trigger: executor.collect_garbage()

================================================================================
25. VERSION & TEST STATUS
================================================================================

Language:       PASTA (Program for Assignment, Statements, Threading, Allocation)
Implementation: Rust (Edition 2021)
Version:        1.1
Date:           March 1, 2026

RECENT CHANGES (v1.0 → v1.1):
    ✓ Fixed AND (&&) and OR (||) operator tokenization
    ✓ Added NOT token to TokenType enum
    ✓ Fixed WHILE loop thread ID (tid) allocation for lambda targets
    ✓ All WHILE loop variants passing (4/4 integration tests)
    ✓ AI neural network module (ai_network.rs, 285 lines)
    ✓ 10 new AI builtins (ai.relu, ai.softmax, ai.loss.mse, etc.)
    ✓ IF/ELSE AST nodes added (partial implementation)
    ✓ New comparison operators (Approx, NotEq, StrictEq) in AST
    ✓ Traceback system (TraceFrame, Traceback)
    ✓ Strainer garbage collector integrated

TEST RESULTS:
    Lexer tests:            22 passed
    While loop tests:       4 passed
    AI network tests:       6 passed
    Total:                  ~145 passed

OPTIONAL FEATURES STATUS:
    ✓ Core interpreter
    ✓ File I/O (read_from_file, write_to_file)
    ✓ Shell operations (pwd, ls, mkdir, rm, cp, mv, etc.)
    ✓ Tensor operations (tensor.*)
    ✓ Neural networks (ai.*)
    ✓ Priority graph
    ✓ Constraint engine
    ○ image (optional: cargo build --features image)
    ○ ndarray (optional: cargo build --features ndarray)
    ○ scheduler (optional: cargo build --features scheduler)

================================================================================
SUPPORT & CONTRIBUTIONS
================================================================================

1. Check existing documentation in docs/
2. Review test cases in tests/ and src/**/tests
3. Follow Rust community guidelines (rustfmt, clippy)
4. File issues on project repository

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
Last Updated: March 1, 2026
PastaLang Team

CONTACT LEAD DEV Travis Garrison, Brainboy2487@gmail.com
