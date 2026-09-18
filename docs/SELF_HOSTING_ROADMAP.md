# 🍝 PASTA Self-Hosting Roadmap
## **From Partial Rewrites to Production Bootstrap Compiler**

**Status**: ~25-30% complete | **Priority**: CRITICAL (Phase 4 Primary Track)  
**Timeline**: 6-8 weeks | **Outcome**: PASTA compiles PASTA

---

## 🎯 **What is Self-Hosting?**

Self-hosting means the PASTA compiler can compile its own source code. This is a transformational milestone:
- ✅ Validates compiler correctness against real, complex code
- ✅ Eliminates dependency on Rust version for bug fixes
- ✅ Accelerates language evolution (iterate in PASTA, not Rust)
- ✅ Enables community contributions without Rust knowledge
- ✅ Proves PASTA is production-ready

**Target Milestone**: By end of Phase 6, PASTA v2.0 ships as self-hosting compiler

---

## 📊 **Current State (3,600+ Lines of .ps Code)**

### Completion Matrix

| Component | Status | Lines | Progress | Priority |
|-----------|--------|-------|----------|----------|
| **Lexer** | ✅ Complete | 423 | 100% | Reference |
| **Tokens** | ✅ Complete | 33 | 100% | Reference |
| **AST** | ✅ Complete | 315 | 100% | Reference |
| **Unicode Helpers** | ✅ Complete | 120 | 100% | Reference |
| **Parser** | ⚠️ Partial | 1,892 | ~70% | **HIGH** |
| **Executor** | 🔄 Stub | 230 | ~15% | **CRITICAL** |
| **Module Loader** | ⚠️ Partial | 278 | ~40% | **HIGH** |
| **IR Generator** | 🔴 Not Started | — | 0% | **CRITICAL** |
| **Backend Codegen** | 🔴 Not Started | — | 0% | **CRITICAL** |
| **Linker Integration** | 🔴 Not Started | — | 0% | **CRITICAL** |

### Files Currently in `.ps` Format
```
✅ src/lexer/lexer.ps              (fully functional)
✅ src/lexer/tokens.ps             (fully functional)
✅ src/lexer/unicode.ps            (fully functional)
✅ src/lexer/alias.ps              (fully functional)
✅ src/parser/ast.ps               (fully functional)
✅ src/parser/grammar.ps           (documentation)
⚠️  src/parser/parser.ps           (mostly done, needs completion)
⚠️  src/interpreter/executor.ps    (stub/skeleton)
⚠️  src/mod_loader/mod_load.ps     (partial implementation)
🔴 [Missing] IR generator
🔴 [Missing] Backend codegen
```

---

## 🚀 **Self-Hosting Phase Breakdown**

### **Phase 4A: Complete Core Compiler in PASTA (4 weeks)**
**Goal**: Full working compiler entirely written in PASTA

#### **Week 1: Complete Parser (Target: 100% → 95%+ functionality)**
**Current State**: ~70% complete (1,320 lines done, ~572 lines remaining)

**Tasks**:
1. **Review existing parser.ps** (lines 1-500)
   - Confirm basic precedence, token matching working
   - Map out which statement parsers are missing
   
2. **Complete statement parsers** (~200 lines)
   - `parse_assignment`, `parse_function_def`, `parse_class_def`
   - `parse_if_stmt`, `parse_while_stmt`, `parse_for_stmt`
   - `parse_do_block`, `parse_spawn_stmt`, `parse_import_stmt`
   - `parse_try_except_stmt`, `parse_with_stmt`

3. **Complete expression parsers** (~150 lines)
   - Prefix operators (`not`, `-`, `+`, `@`)
   - Infix operators (all 50+ operators with correct precedence)
   - Postfix operators (method call, indexing, slicing)
   - Pipelines, comprehensions, lambda expressions

4. **Error recovery & diagnostics** (~100 lines)
   - Synchronization after parse errors
   - Line/column tracking
   - Helpful error messages

5. **Testing**: Create `tests/phase4a_parser_pasta.rs`
   - Parse all existing `.ps` files
   - Verify AST matches Rust parser output
   - Validate 50+ test cases (arithmetic, functions, classes, control flow)

**Outcome**: Parser completely written in PASTA, all tests passing

---

#### **Week 2: Implement IR Generator (Target: 0% → 80%)**
**Current State**: Not started

**Tasks**:
1. **Design IR format in PASTA**
   - Define IR node types (Function, Block, Instruction, etc.)
   - Create IR value representation (Literal, Variable, BinOp, Call, etc.)
   - Write IR constructor functions

2. **Implement AST → IR translation** (~400 lines)
   - `ir_from_ast(ast_node)` recursive function
   - Handle all expression types (literals, variables, operators, calls, etc.)
   - Handle all statement types (assignments, function definitions, control flow)
   - Scope management and variable binding

3. **Add built-in function inlining**
   - Recognize calls to `print`, `len`, `range`, etc.
   - Generate direct IR for common operations
   - Avoid function call overhead for builtins

4. **Testing**: Create `tests/phase4a_ir_generator.rs`
   - Generate IR for 30+ test programs
   - Verify IR correctness via execution traces
   - Validate optimization decisions

**Outcome**: IR generator working for all core language features

---

#### **Week 3: Implement Basic Backend (Target: 0% → 70%)**
**Current State**: Not started

**Tasks**:
1. **Choose code generation target**
   - Option A: LLVM IR (compatibility with Rust backend)
   - Option B: Bytecode VM (simpler, faster iteration)
   - **Recommendation**: Start with bytecode VM, LLVM as Phase 5

2. **Implement IR → Bytecode compiler** (~300 lines)
   - Bytecode instruction set (LOAD, STORE, CALL, JUMP, etc.)
   - Register allocation for expressions
   - Stack management for function calls

3. **Add symbol table generation**
   - Export public functions, classes, constants
   - Import table for dependencies
   - Visibility rules (public/private)

4. **Testing**: Create `tests/phase4a_codegen.rs`
   - Generate bytecode for 20+ programs
   - Execute bytecode to verify correctness
   - Compare output with interpreter

**Outcome**: Full compilation pipeline working: PASTA source → IR → bytecode

---

#### **Week 4: Integration & Testing (Target: Full system validation)**

**Tasks**:
1. **Create pasta-compiler-in-pasta**
   - Main entry point: `main()` function in PASTA
   - Command-line argument parsing
   - File I/O for reading `.pasta` source
   - Bytecode output to `.pobj` or executable

2. **Cross-compile test suite**
   - Use existing Rust compiler to compile bootstrapped compiler
   - Run bootstrapped compiler on test suite
   - Verify identical output vs. Rust compiler

3. **Stress testing** (~50 test programs)
   - Existing examples/ directory
   - Stress tests (nested loops, recursion, large functions)
   - Real-world PASTA code from stdlib

4. **Benchmark**
   - Compile time comparison (Rust vs. PASTA compiler)
   - Runtime performance (bytecode vs. current interpreter)

**Outcome**: Production-ready compiler entirely in PASTA, ready for bootstrapping

---

### **Phase 4B: Bootstrap & Validation (2-3 weeks)**
**Goal**: PASTA compiler compiles itself, producing identical output

**Tasks**:
1. **Build Phase 4A compiler using Rust toolchain**
   - Compile all `.ps` files using existing PASTA interpreter
   - Generate initial PASTA compiler binary

2. **Run bootstrapped compiler on itself**
   - `pasta-compiler.exe compile src/lexer/lexer.ps → lexer.pobj`
   - Repeat for parser, IR generator, codegen
   - Produce complete bootstrapped compiler

3. **Verify bit-identical output**
   - Compare bytecode output (PASTA-compiled vs. Rust-compiled)
   - Compare against reference output from Rust compiler
   - Ensure deterministic builds (reproducible output)

4. **Publish milestone**: **v2.0.0-alpha1 (Self-Hosting Bootstrap)**

---

### **Phase 5: LLVM Integration & Optimization (3 weeks)**
**Goal**: Bytecode → native binaries with competitive performance

**Tasks**:
1. **Replace bytecode with LLVM IR generation**
   - `ir_generator.pasta` now generates LLVM IR directly
   - Use LLVM C++ API via FFI (via Rust bindings)

2. **Add optimization passes**
   - Dead code elimination
   - Constant folding
   - Inlining of hot functions
   - Tail call optimization

3. **Benchmark against Rust compiler**
   - Compilation time
   - Runtime performance
   - Binary size

4. **Publish milestone**: **v2.0.0-beta1 (LLVM Codegen)**

---

### **Phase 6: Full Self-Hosting Release (1-2 weeks)**
**Goal**: Official v2.0.0 with 100% self-hosted compiler

**Tasks**:
1. **Remove Rust compiler from critical path**
   - Keep Rust version as reference only
   - Distribute PASTA v2.0 with bootstrapped binary
   - Documentation for building from source

2. **ABI stabilization**
   - Finalize calling conventions
   - Stabilize symbol export/import rules
   - Version `.pobj` format

3. **Release v2.0.0**
   - Self-hosting compiler
   - 100% feature parity with v1.6.2
   - All tests passing

---

## 🛠️ **Detailed Implementation Tasks**

### **Task 1: Complete Parser (Week 1)**
**File**: `src/parser/parser.ps`  
**Current**: Lines 1-1000 (precedence, helpers, basic expression parsing)  
**Missing**: Lines 1001-1892 (statement parsers, error recovery)

```pasta
# Pseudocode structure for missing parsers:

DEF parse_assignment():
    # Parse: x = expr
    # Parse: obj.field = expr
    # Parse: list[idx] = expr
END

DEF parse_function_def():
    # Parse: DEF name(params) ... END
    # Handle decorators, type hints, defaults
END

DEF parse_class_def():
    # Parse: CLASS name ... END
    # Handle inheritance, fields, methods
END

DEF parse_if_stmt():
    # Parse: IF cond ... ELIF ... ELSE ... END
END

DEF parse_try_except_stmt():
    # Parse: TRY ... EXCEPT ... FINALLY ... END
END

DEF parse_import_stmt():
    # Parse: FROM module IMPORT name, name
    # Parse: USE module AS alias
END
```

**Test Coverage**: 50+ test cases
- Simple expressions (arithmetic, comparison)
- Complex expressions (pipelines, comprehensions, lambdas)
- All statement types (assignment, function, class, control flow)
- Error cases (malformed syntax, missing terminators)

**Validation Metric**: 100% of existing `.pasta` files parse correctly

---

### **Task 2: IR Generator (Week 2)**
**File**: `[NEW] src/ir/ir_gen.ps`

```pasta
# IR node types
DEF IrNode(type, data):
    RETURN({"type": type, "data": data})
END

DEF IrFunction(name, params, body, ...):
    # Function IR node
END

DEF IrBlock(statements):
    # Sequence of IR statements
END

DEF IrAssign(target, value):
    # x = expr
END

DEF IrCall(func, args):
    # Function call
END

# Main IR generator
DEF ir_from_ast(ast_node):
    # Recursive translation from AST → IR
    IF ast_node["type"] == "BinOp":
        left = ir_from_ast(ast_node["left"])
        right = ir_from_ast(ast_node["right"])
        RETURN(IrBinOp(ast_node["op"], left, right))
    ELIF ast_node["type"] == "FunctionDef":
        # Translate function body to IR
    ELIF ...
    END
END
```

**Test Coverage**: 30+ test programs
- Arithmetic expressions
- Function definitions and calls
- Control flow (if, while, for)
- Data structures (lists, maps, objects)
- Error handling (try/except)

**Validation Metric**: IR evaluation matches original interpreter output

---

### **Task 3: Bytecode Backend (Week 3)**
**File**: `[NEW] src/codegen/bytecode_gen.ps`

```pasta
# Bytecode instruction set
BYTECODE_OPCODES = {
    "LOAD_CONST": 1,      # Push constant
    "LOAD_VAR": 2,        # Load variable
    "STORE_VAR": 3,       # Store variable
    "CALL_FUNC": 4,       # Call function
    "JUMP": 5,            # Unconditional jump
    "JUMP_IF_FALSE": 6,   # Conditional jump
    "RETURN": 7,          # Return from function
    "BINOP": 8,           # Binary operation
    "UNOP": 9,            # Unary operation
    ...
}

DEF bytecode_from_ir(ir_node):
    # Translate IR → bytecode instructions
    # Return list of bytecode bytes
END
```

**Test Coverage**: 20+ bytecode programs
- Loop execution
- Function recursion
- Variable scoping
- Memory allocation

**Validation Metric**: Bytecode execution produces same output as IR

---

### **Task 4: Pasta Compiler Main (Week 4)**
**File**: `[NEW] pasta_compiler.pasta` (entry point)

```pasta
DEF main(args):
    # Parse command-line arguments
    input_file = args[0]  # .pasta source file
    output_file = args[1] or "output.pobj"
    
    # Compile
    source = read_file(input_file)
    tokens = lexer_tokenize(source)
    ast = parser_parse(tokens)
    ir = ir_gen(ast)
    bytecode = bytecode_gen(ir)
    
    # Write output
    write_file(output_file, bytecode)
    
    PRINT("✅ Compiled to " .. output_file)
END
```

---

## 📋 **Integration with Phase 4-10 Roadmap**

### Current Phases 4-10 (from PHASES_7_TO_10_IMPLEMENTATION_GUIDE.md):

| Phase | Original Purpose | Self-Hosting Impact |
|-------|------------------|-------------------|
| **Phase 4** | Graphics backend | **NOW: Complete Compiler (4A + 4B)** |
| **Phase 5** | Multi-platform testing | **NOW: LLVM integration** |
| **Phase 6** | CI/CD & distribution | **NOW: Self-hosting release** |
| Phase 7 | LSP, formatter, linter | After self-hosting is stable |
| Phase 8 | Type system | After self-hosting is stable |
| Phase 9 | Bytecode VM, async | After self-hosting is stable |
| Phase 10 | Package manager | After self-hosting is stable |

### **Revised Roadmap (Self-Hosting First)**

```
Phase 3 (Current)     ✅ DONE: Windows MVP
  ↓
Phase 4A (NEW)        → Complete Compiler in PASTA (4 weeks)
  ↓
Phase 4B (NEW)        → Bootstrap & Validation (2-3 weeks)
  ↓
Phase 5 (NEW)         → LLVM Integration (3 weeks)
  ↓
Phase 6 (NEW)         → Self-Hosting Release v2.0.0 (1-2 weeks)
  ↓
Phase 7 (Graphics)    → Windows Win32 graphics backend
  ↓
Phase 8 (Ecosystem)   → CI/CD, package manager, distribution
  ↓
Phase 9+ (Developer Experience) → LSP, formatter, linter, debugger
```

**Timeline**: 6-8 weeks to self-hosting (v2.0.0), then 2-3 weeks for graphics (Phase 7)

---

## 🎯 **Success Criteria**

### Phase 4A Complete:
- [ ] `src/parser/parser.ps` is 100% complete
- [ ] `src/ir/ir_gen.ps` handles all AST node types
- [ ] `src/codegen/bytecode_gen.ps` generates correct bytecode
- [ ] `pasta_compiler.pasta` compiles & runs end-to-end
- [ ] All 50+ test cases pass

### Phase 4B Bootstrap:
- [ ] PASTA compiler can compile itself
- [ ] Bytecode output is bit-identical across runs
- [ ] All 216+ library tests pass with bootstrapped compiler
- [ ] Performance is acceptable (< 10 seconds for full compile)

### Phase 5 LLVM:
- [ ] LLVM IR generation replaces bytecode
- [ ] Native binary performance competitive with Rust version
- [ ] Compilation time reasonable (< 30 seconds for large projects)

### Phase 6 Release:
- [ ] v2.0.0 published with self-hosting compiler
- [ ] Zero breaking changes from v1.6.2
- [ ] Community can build from source using PASTA

---

## 📅 **Critical Path Timeline**

```
Week 1 (Parser)              | Parser complete, 50+ tests passing
Week 2 (IR)                  | IR generator done, 30+ test programs
Week 3 (Codegen)             | Bytecode codegen, 20+ programs executing
Week 4 (Integration)         | Full compiler working, stress testing
        ↓↓↓ Phase 4A Complete ↓↓↓
Week 5-6 (Bootstrap)         | PASTA compiles PASTA, validation
        ↓↓↓ Phase 4B Complete ↓↓↓
Week 7 (LLVM)                | Replace bytecode with LLVM IR
        ↓↓↓ Phase 5 Complete ↓↓↓
Week 8 (Release)             | v2.0.0 public, documentation
        ↓↓↓ SELF-HOSTING ACHIEVED ↓↓↓
```

---

## 🔗 **Related Documents**

- **FINISHING_PLAN.md** - Original roadmap (now superseded by this)
- **PHASES_7_TO_10_IMPLEMENTATION_GUIDE.md** - Feature roadmap after self-hosting
- **compiler_todo.txt** - Legacy task tracker (update priorities)
- **docs/phase2_compiler.txt** - Original compiler phase plan

---

## ⚠️ **Risks & Mitigations**

| Risk | Mitigation |
|------|-----------|
| Parser completion overruns | Daily progress reviews, incremental testing |
| IR design flaws | Design review with Rust IR, compare outputs |
| Bytecode performance inadequate | Profile early, migrate to LLVM if needed |
| Bootstrap convergence fails | Keep Rust compiler as fallback, iterate design |
| Timeline slips | Start with MVP (no optimization), add later |

---

## 🎉 **Outcome: PASTA v2.0.0 Self-Hosting Compiler**

Upon completion:
1. **PASTA compiles PASTA** — no Rust dependency for language evolution
2. **Community-driven development** — anyone can contribute to compiler in PASTA
3. **Competitive positioning** — self-hosting is major milestone for production language
4. **Accelerated iteration** — bugfixes and features in PASTA, not Rust
5. **Production-ready** — language maturity indicator to ecosystem

This is the moment PASTA becomes a "real" language capable of evolving itself.

---

**Next Action**: Approve self-hosting as Phase 4 primary (ahead of graphics), authorize Phase 4A (Parser completion).
