# Phases 7-10: Detailed Implementation Guide

**Date:** 2026-07-16  
**Status:** Planning & Task Breakdown  
**Target Version:** v1.7.0 (4-5 weeks)

---

## PHASE 7: Developer Experience & Tooling

### Overview
- **Duration:** 7-10 days
- **Goal:** Make PASTA delightful to code with
- **Deliverables:** LSP, formatter, linter, debugger

### 7.1 LSP Language Server (3-4 days)

#### Architecture
```
User (VS Code, Vim, Emacs)
       ↓ LSP Protocol (JSON-RPC)
    LSP Server (pasta-lsp)
       ↓
    AST Parser
    Symbol Table
    Type Checker
       ↓
    Response (autocomplete, hover, etc.)
```

#### Implementation Tasks

**Day 1: LSP Foundation**
```
1.1 JSON-RPC server skeleton
    - Listen on stdio/TCP
    - Handle initialize request
    - Manage document lifecycle

1.2 Basic message handling
    - textDocument/didOpen
    - textDocument/didChange
    - textDocument/didClose

1.3 Parser integration
    - Use existing PASTA parser
    - Build symbol table
    - Track source locations
```

**Day 2: Core Features**
```
2.1 Autocomplete (textDocument/completion)
    - Keywords (IF, WHILE, etc.)
    - Built-in functions
    - User variables
    - Scoring/ranking

2.2 Go-to-definition (textDocument/definition)
    - Symbol location tracking
    - Module resolution
    - Cross-file support

2.3 Hover information (textDocument/hover)
    - Function signatures
    - Type information
    - Documentation
```

**Day 3: Advanced Features**
```
3.1 Symbol renaming (textDocument/rename)
    - Find all usages
    - Rename across scopes
    - Confirm refactoring

3.2 Diagnostics (textDocument/publishDiagnostics)
    - Syntax errors
    - Type errors
    - Warnings

3.3 Quick fixes (textDocument/codeAction)
    - Fix unused imports
    - Add missing parentheses
    - Suggest alternatives
```

**Day 4: IDE Plugins**
```
4.1 VS Code extension (pasta-vscode)
    - Extension manifest
    - LSP client setup
    - Keybindings
    - Syntax highlighting

4.2 Vim plugin (vim-pasta)
    - LSP client integration
    - ALE integration
    - Keybindings

4.3 Emacs plugin (emacs-pasta)
    - LSP-mode integration
    - Flycheck integration
```

#### Deliverables
- [ ] pasta-lsp binary (src/bin/lsp_server.rs)
- [ ] VS Code extension (extensions/vscode-pasta/)
- [ ] Vim plugin (extensions/vim-pasta/)
- [ ] 40+ LSP tests
- [ ] Documentation

#### Success Criteria
- [x] Autocomplete works in VS Code
- [x] Go-to-definition jumps correctly
- [x] Hover shows function signatures
- [x] Error diagnostics appear
- [x] Symbol renaming works

---

### 7.2 Code Formatter (1-2 days)

#### Design Principles
```
1. Deterministic (same output always)
2. Opinionated (no config options at first)
3. Fast (format large files instantly)
4. Preserves semantics (no code changes)
```

#### Implementation

**Day 1: Formatter Core**
```
1.1 AST pretty-printer
    - Convert AST to formatted string
    - Use consistent indentation (2 spaces)
    - Line breaking strategy

1.2 Special handling
    - String literals (no changes)
    - Comments (preserve, indent)
    - Blank lines (normalize)

1.3 CLI integration
    - pasta fmt file.ps
    - pasta fmt --check (CI mode)
    - pasta fmt --diff file.ps
    - Recursive directory format
```

**Day 2: Polish**
```
2.1 Edge cases
    - Long lines (break at reasonable width)
    - Nested functions
    - Complex expressions
    - Comments between statements

2.2 Testing
    - Format consistency
    - Idempotent (format twice = same)
    - Preserve semantics

2.3 Documentation
    - Format guide
    - Configuration (future)
    - Pre-commit hook setup
```

#### Configuration (Future)
```toml
[formatting]
indent_width = 2
line_width = 100
break_long_lines = true
space_after_comma = true
space_before_brace = true
```

#### Deliverables
- [ ] pasta fmt command
- [ ] tests/format_*.rs (30+ tests)
- [ ] Documentation
- [ ] Pre-commit hook example

---

### 7.3 Linter (2-3 days)

#### Rule Categories
```
1. Code Smells
   - Unused variables
   - Unused imports
   - Dead code
   - Unreachable statements

2. Safety Issues
   - Missing error handling
   - Potential null access
   - Type mismatches (basic)
   - Division by zero

3. Best Practices
   - Shadowed variables
   - Confusing names
   - Complex functions
   - Deep nesting

4. Performance
   - Unused allocations
   - Inefficient loops
   - Large string copies
```

#### Implementation

**Day 1: Rule Engine**
```
1.1 Rule system architecture
    - Rule trait
    - Rule registry
    - Severity levels (error, warn, info)
    - Auto-fix capability

1.2 AST visitor pattern
    - Walk entire AST
    - Collect violations
    - Report with location

1.3 Configuration
    - .pastalint.toml
    - Enable/disable rules
    - Severity overrides
```

**Day 2: Core Rules**
```
2.1 Basic rules (5 rules)
    - Unused variables
    - Unused imports
    - Unreachable code
    - Missing documentation (optional)
    - Style violations

2.2 Safety rules (5 rules)
    - Missing error handling
    - Potential null dereference
    - Shadowed variables
    - Confusing patterns

2.3 Performance rules (5 rules)
    - Inefficient operations
    - Large copies
    - Unnecessary allocations
```

**Day 3: CLI & Integration**
```
3.1 CLI tool
    - pasta lint file.ps
    - pasta lint --strict (all rules)
    - pasta lint --fix (auto-fix where possible)
    - Output formats (text, JSON, CSV)

3.2 Editor integration
    - LSP diagnostic integration
    - Real-time linting
    - Inline fixes

3.3 CI integration
    - Exit code for CI
    - Fail on warnings (optional)
```

#### Deliverables
- [ ] pasta lint command
- [ ] 15+ lint rules
- [ ] tests/lint_*.rs (40+ tests)
- [ ] Auto-fix for common issues
- [ ] Documentation

---

### 7.4 Debugger (2-3 days)

#### Features
```
1. Breakpoints
   - Line breakpoints
   - Conditional breakpoints (if expression)
   - Function entry breakpoints

2. Execution Control
   - Continue (resume execution)
   - Step (single line)
   - Step over (skip function calls)
   - Step out (return from function)

3. Inspection
   - Variable values
   - Stack frames
   - Local/global scope
   - Watch expressions

4. Integration
   - REPL-like debugger interface
   - VS Code debugger protocol (DAP)
   - Vim/Emacs integration
```

#### Implementation

**Day 1: Debug Infrastructure**
```
1.1 Breakpoint system
    - Store breakpoints
    - Check at each statement
    - Line number mapping

1.2 Execution controller
    - Pause at breakpoint
    - Single-step execution
    - Stack frame tracking

1.3 Variable inspection
    - Collect local variables
    - Format for display
    - Handle nested values
```

**Day 2: CLI Debugger**
```
2.1 Debug REPL
    - pasta debug script.ps
    - (debug) b 10 (set breakpoint)
    - (debug) c (continue)
    - (debug) s (step)
    - (debug) p x (print variable)
    - (debug) bt (backtrace)

2.2 Commands
    - break [line]
    - continue
    - step
    - step-over
    - step-out
    - print [expr]
    - backtrace
    - locals
    - quit
```

**Day 3: IDE Integration**
```
3.1 DAP protocol (Debug Adapter Protocol)
    - Implement DAP server
    - VS Code integration
    - Breakpoint UI
    - Watch expressions

3.2 Testing
    - Debug session tests
    - Breakpoint accuracy
    - Variable inspection
    - Step execution
```

#### Deliverables
- [ ] pasta debug command
- [ ] Debug infrastructure
- [ ] tests/debug_*.rs (25+ tests)
- [ ] DAP server implementation
- [ ] VS Code debug extension
- [ ] Documentation

---

## PHASE 8: Type System & Safety

### Overview
- **Duration:** 10-14 days
- **Goal:** Optional static typing for safety and IDE support
- **Key Principle:** Backward compatible (all existing code works)

### 8.1 Type Annotations (3-4 days)

#### Syntax
```rust
// Variables
x: number = 42
name: string = "Alice"
active: bool = true

// Functions
DEF add(a: number, b: number) -> number
  RETURN a + b
END

// Collections
names: LIST[string] = ["Alice", "Bob"]
counts: DICT[string, number] = {"a": 1, "b": 2}

// Optionals
value: number? = NULL
```

#### Implementation

**Day 1: Parser**
```
1.1 Type annotation syntax
    - Identifier: Type
    - -> ReturnType in functions
    - Generic types LIST[T], DICT[K, V]

1.2 Parser changes
    - Update variable parser
    - Update function parser
    - Handle type expressions

1.3 AST updates
    - Add type field to Variable
    - Add return type to Function
    - Add type to Parameter
```

**Day 2: Type Representation**
```
2.1 Type system design
    - Built-in types: number, string, bool, null
    - Collection types: LIST, DICT, TUPLE
    - Function types: Callable
    - User types: classes (future)

2.2 Type operations
    - Unification
    - Subtyping
    - Type compatibility

2.3 Error types
    - Type mismatch reporting
    - Helpful messages
```

**Day 3: Type Inference**
```
3.1 Inference algorithm
    - Hindley-Milner based
    - Walk AST, collect constraints
    - Solve constraints

3.2 Inferrable patterns
    - Variable assignment: x = 5 → x: number
    - Function call: add(1, 2) → result: number
    - Literals: [1, 2] → LIST[number]

3.3 Annotations override inference
    - Explicit type wins
    - Inference fills in rest
```

**Day 4: Integration & Testing**
```
4.1 Type checker module
    - Standalone type checking
    - Optional in interpreter
    - Report errors without failing

4.2 Testing
    - Type inference tests
    - Type checking tests
    - Error message tests
```

#### Deliverables
- [ ] Type annotation syntax support
- [ ] Type inference engine
- [ ] Type checking module
- [ ] 50+ type tests
- [ ] Error messages
- [ ] Documentation

---

### 8.2 Advanced Error Types (2-3 days)

#### Error Hierarchy
```
Error (base)
├─ IOError
├─ TypeError
├─ ValueError
├─ RuntimeError
├─ SyntaxError
├─ ModuleNotFoundError
└─ Custom errors (user-defined)
```

#### Syntax
```rust
// Catch specific errors
TRY
  file_read("missing.txt")
OTHERWISE IOError as e
  PRINT "File not found: " + e.message
OTHERWISE FilePermissionError as e
  PRINT "Permission denied"
OTHERWISE error
  PRINT "Other error: " + error.message
END

// Error properties
e.code          # E0001
e.message       # "File not found"
e.stack_trace   # Stack frames
e.context       # Surrounding variables (optional)
```

#### Implementation

**Day 1: Error Type System**
```
1.1 Error struct
    - code (E0001, E0002, etc.)
    - message (string)
    - stack_trace (Vec<Frame>)
    - context (HashMap<String, Value>)

1.2 Error registry
    - Define all error types
    - Code assignments
    - Default messages

1.3 Stack trace capture
    - Collect at throw time
    - Function names, line numbers
    - Variable snapshots (optional)
```

**Day 2: Error Throwing**
```
2.1 RAISE keyword
    - RAISE "message"
    - RAISE IOError("message")
    - RAISE error

2.2 Automatic errors
    - Type mismatches
    - Division by zero
    - Null dereference
    - Module not found
```

**Day 3: Integration**
```
3.1 OTHERWISE improvements
    - Multiple catch types
    - Error binding (as e)
    - Error filtering

3.2 Testing
    - Error creation
    - Error throwing
    - Error catching
    - Stack trace accuracy
```

#### Deliverables
- [ ] Error hierarchy
- [ ] Stack trace capture
- [ ] Error throwing mechanism
- [ ] Enhanced OTHERWISE blocks
- [ ] 30+ error tests
- [ ] Error documentation

---

### 8.3 Nullable Type Control (1-2 days)

#### Syntax
```rust
// Non-nullable (default)
x: number = 5      # Can never be null
y: string = "hi"   # Can never be null

// Nullable (explicit)
x: number? = NULL         # Can be null or number
y: string? = "maybe"      # Can be null or string

// Null checks (required)
value: number? = get_optional()
IF value != NULL THEN
  PRINT value + 1  # OK - we checked
END

// Option-style
maybe_value: number? = NULL
MATCH maybe_value
  WHEN NULL → PRINT "Not available"
  WHEN x → PRINT "Value: " + x
END
```

#### Implementation

**Day 1: Type System**
```
1.1 Nullable type representation
    - Type = Core | Optional
    - Can be: number, number?

1.2 Type checking
    - Track nullability in type inference
    - Error on nullable use without check
    - Allow null checks to narrow type
```

**Day 2: Type Narrowing**
```
2.1 Narrowing in conditionals
    - IF x != NULL THEN → type narrows
    - Pattern matching → type narrows
    - Guard clauses

2.2 Error reporting
    - "value is nullable, must check for NULL"
    - Suggest null checks
    - Show code examples

2.3 Integration with defaults
    - Default values prevent nullability
    - Optional parameters (future)
```

#### Deliverables
- [ ] Nullable type syntax
- [ ] Type narrowing
- [ ] Tests (20+)
- [ ] Documentation

---

### 8.4 Generics (3-4 days)

#### Syntax
```rust
// Generic functions
DEF first[T](list: LIST[T]) -> T
  RETURN list[0]
END

first[number]([1, 2, 3])  # → number
first[string](["a", "b"]) # → string

// Inferred generics
first([1, 2, 3])     # T inferred as number

// Generic types (future)
CLASS Box[T]
  value: T
END
```

#### Implementation

**Day 1: Generic Syntax**
```
1.1 Parser support
    - DEF name[Type1, Type2, ...](...) END
    - Generic parameters
    - Generic function calls
    - Type argument inference

1.2 AST representation
    - Generic parameters in Function
    - Type arguments in Call
```

**Day 2: Type Resolution**
```
2.1 Generic instantiation
    - For each call: substitute type arguments
    - Generate specialized versions
    - Cache compiled versions

2.2 Type inference
    - Infer T from LIST[T] argument
    - Infer from call context
    - Error on ambiguous generics
```

**Day 3: Specialization**
```
3.1 Monomorphization
    - Generate specialized code for each usage
    - Or: Runtime generic handling

3.2 Performance
    - Cache specializations
    - No performance penalty
    - Compile time trade-off
```

**Day 4: Testing & Integration**
```
4.1 Generic tests
    - Generic functions
    - Generic type inference
    - Compilation
    - Runtime execution

4.2 Error cases
    - Missing type arguments
    - Type mismatch in generics
    - Circular generics
```

#### Deliverables
- [ ] Generic syntax
- [ ] Type argument inference
- [ ] Specialization
- [ ] 30+ generic tests
- [ ] Documentation

---

## PHASE 9: Performance & Async

### Overview
- **Duration:** 12-16 days
- **Goal:** 3-5x faster, native async support
- **Key Achievement:** Bytecode VM baseline

### 9.1 Bytecode Compiler (5-7 days)

#### Architecture
```
Source Code
    ↓
Lexer (tokens)
    ↓
Parser (AST)
    ↓
Compiler (Bytecode)
    ↓
VM (execution)
```

#### Bytecode Instructions
```rust
enum Opcode {
    // Constants
    LoadConst(usize),      // Push constant from pool
    
    // Variables
    LoadVar(String),       // Push variable value
    StoreVar(String),      // Pop and store to variable
    
    // Operations
    Add, Sub, Mul, Div, Mod, Pow,
    Equal, NotEqual, Less, Greater, LessEq, GreaterEq,
    And, Or, Not,
    
    // Functions
    Call(usize),           // Call function (arg count)
    Return,
    
    // Control
    Jump(usize),           // Unconditional jump
    JumpIfFalse(usize),    // Conditional jump
    
    // Other
    Pop,                   // Discard top of stack
    Print,                 // Print top of stack
}
```

#### Implementation

**Day 1: IR Design**
```
1.1 Bytecode format
    - Instruction set (30-40 opcodes)
    - Constants pool (strings, numbers)
    - Function table (name, arity, bytecode)
    - Metadata (line info for debugging)

1.2 Code generation
    - Walk AST
    - Emit bytecode for each node
    - Handle basic types
```

**Day 2: Compiler**
```
2.1 Bytecode generator
    - AST → Bytecode
    - Handle expressions
    - Handle statements
    - Function definitions

2.2 Optimizations
    - Constant folding
    - Peephole optimization
    - Dead code elimination
    - Register allocation (stack-based)

2.3 Debugging info
    - Line number mapping
    - Source locations
    - Function names
```

**Day 3: Stack VM**
```
3.1 Virtual machine
    - Stack-based execution
    - Instruction dispatch loop
    - Value stack
    - Call stack

3.2 Built-in functions
    - Implement 80+ functions on VM
    - Fast paths for hot functions
    - Argument marshalling

3.3 Caching
    - Cache compiled bytecode
    - Memoize functions
    - Fast path for repeated calls
```

**Day 4: Integration**
```
4.1 Seamless transition
    - AST interpreter still available
    - Bytecode used by default
    - Option to use AST interpreter

4.2 Testing
    - Run existing tests on bytecode
    - Verify same output
    - Benchmark performance
    - Memory usage comparison

4.3 Debugging
    - Line number mapping
    - Disassembler (view bytecode)
    - Tracer (execution log)
```

**Days 5-7: Optimization & Polish**
```
5.1 Performance tuning
    - Optimize hot paths
    - Reduce instruction count
    - Improve cache locality

5.2 Feature completion
    - Complex control flow
    - Exception handling
    - Module loading

5.3 Polish & documentation
    - Comprehensive tests (100+)
    - Bytecode format spec
    - Disassembler tool
    - Performance report
```

#### Expected Performance
```
Current (AST): Baseline
Bytecode: 3-5x faster
JIT (future): 10-100x faster
```

#### Deliverables
- [ ] Bytecode format specification
- [ ] Compiler (AST → Bytecode)
- [ ] Stack VM interpreter
- [ ] Bytecode caching
- [ ] 80+ tests
- [ ] Performance benchmarks
- [ ] Disassembler tool
- [ ] Documentation

---

### 9.2 Async/Await Keywords (4-5 days)

#### Design
```rust
// Async functions
ASYNC DEF fetch_data(url: string) -> string
  RETURN await http_get(url)
END

// Await expressions
data = await fetch_data("https://example.com")

// Async lambdas
handler = ASYNC LAMBDA (event)
  result = await process(event)
  RETURN result
END

// Async for loops (future)
FOR x IN async_generator() END
  PRINT x
END
```

#### Implementation

**Day 1: Syntax & Parsing**
```
1.1 ASYNC keyword
    - ASYNC DEF name(...) → Function
    - ASYNC LAMBDA ... → Lambda
    - Returns Future[T]

1.2 AWAIT keyword
    - AWAIT expression
    - Suspends execution
    - Resumes with value

1.3 Parser changes
    - Handle async functions
    - Handle await expressions
    - Type inference for futures
```

**Day 2: Runtime**
```
2.1 Future type
    - Represents async computation
    - Not yet executed
    - Can await or spawn

2.2 Executor
    - Run multiple futures concurrently
    - Work-stealing scheduler
    - Efficient context switching

2.3 Async runtime
    - Thread pool (configurable)
    - Event loop
    - I/O integration
```

**Day 3: Integration**
```
3.1 Async builtins
    - async_sleep(ms)
    - async_http_get(url)
    - async_file_read(path)
    - async_spawn(future)
    - await_all([future1, future2])

3.2 Error handling
    - Async error propagation
    - Try/catch in async functions
    - Error context preservation

3.3 Cancellation
    - Cancel futures
    - Cleanup on cancel
    - Timeout support
```

**Days 4-5: Testing & Examples**
```
4.1 Tests (50+)
    - Basic async/await
    - Multiple concurrent futures
    - Error handling
    - Timeouts
    - Cancellation

4.2 Examples
    - HTTP client
    - File processing
    - Concurrent tasks
    - Rate limiting

4.3 Benchmarking
    - Performance vs DO: blocks
    - Memory usage
    - Throughput testing
```

#### Deliverables
- [ ] ASYNC/AWAIT keywords
- [ ] Future type
- [ ] Async executor
- [ ] 50+ async tests
- [ ] Async standard library
- [ ] Documentation
- [ ] Examples

---

### 9.3 Thread Pool & Executor (2-3 days)

#### Features
```
1. Work-stealing scheduler
   - Efficient task distribution
   - Load balancing

2. Thread pool
   - Configurable size
   - Dynamic scaling (optional)

3. I/O integration
   - Non-blocking I/O
   - Polling
   - Async system calls

4. Graceful shutdown
   - Drain queue
   - Wait for tasks
   - Resource cleanup
```

#### Implementation

**Day 1: Thread Pool**
```
1.1 Basic pool
    - Create N worker threads
    - Task queue (MPSC)
    - Worker loop

1.2 Work distribution
    - Fair queuing
    - Priority support (future)

1.3 Configuration
    - Pool size
    - Shutdown timeout
    - Panic handling
```

**Day 2: Executor**
```
2.1 Task scheduling
    - Submit tasks to pool
    - Execute futures
    - Handle completion

2.2 Synchronization
    - Shared state
    - Atomic operations
    - Lock-free where possible

2.3 Monitoring
    - Queue depth
    - Task count
    - Performance metrics
```

**Day 3: Integration**
```
3.1 AsyncContext
    - Make executor thread-local
    - Access from async functions
    - Nested executors

3.2 Testing
    - Pool stress tests
    - Executor correctness
    - Edge cases
```

#### Deliverables
- [ ] Thread pool implementation
- [ ] Async executor
- [ ] 30+ executor tests
- [ ] Monitoring/metrics
- [ ] Documentation

---

## PHASE 10: Ecosystem & Distribution

### Overview
- **Duration:** 10-14 days
- **Goal:** Enable third-party packages, standardize distribution
- **Components:** Package manager, registry, build system, stdlib

(Detailed implementation guide follows same pattern as Phase 9)

---

## Summary: Phases 7-10 Timeline

```
Week 1 (Phase 7):
├─ Mon-Tue: LSP server (core)
├─ Wed: Code formatter
├─ Thu: Linter
├─ Fri: Debugger
└─ Weekoff: Testing & polish

Week 2-3 (Phase 8):
├─ Days 1-4: Type annotations & inference
├─ Days 5-6: Error types
├─ Days 7-8: Nullable types
├─ Days 9-12: Generics
└─ Days 13-14: Testing & integration

Week 4-5 (Phase 9):
├─ Days 1-7: Bytecode compiler
├─ Days 8-12: Async/await
├─ Days 13-15: Thread pool & executor
└─ Days 16-17: Performance testing

Week 6-7 (Phase 10):
├─ Days 1-7: Package manager
├─ Days 8-10: Package registry
├─ Days 11-12: Stdlib expansion
├─ Days 13-14: Build system
└─ Days 15-18: Testing & launch

TOTAL: 4-5 weeks to v1.7.0
```

---

*Phases 7-10 Implementation Guide Complete*  
*Ready for sprint planning*
