# 🚀 PASTA Phase 4A: Complete Compiler in PASTA — Sprint Plan
## 4-Week Implementation Guide with Daily Tasks

**Scope**: Complete parser, IR generator, bytecode backend, integration  
**Goal**: Full working compiler entirely in PASTA  
**Timeline**: 20 working days (4 weeks)  
**Outcome**: `pasta_compiler.pasta` compiles PASTA source to bytecode

---

## 📅 **WEEK 1: Complete Parser (Lines 1000-1892)**

### **Goal**: Parser.ps 100% complete, all statement/expression types working

### **Day 1-2: Parser Analysis & Gap Identification**

**Tasks**:
1. **Review existing parser.ps** (lines 1-1000)
   - Map all implemented functions
   - Create gap list (missing statement parsers)
   - Document all precedence values
   
2. **Analyze Rust parser.rs** (reference)
   - Identify all statement types: `Assignment`, `FunctionDef`, `ClassDef`, `If`, `While`, `For`, `Do`, `Try`, `Import`, `From`, `Spawn`, `With`, `Return`, `Break`, `Continue`
   - Map each to PASTA parser function signature
   - Note error recovery patterns

3. **Output**: Gap document listing ~15 missing parser functions + line count estimates

**Code to Review**:
```
src/parser/parser.rs lines 1-500     (parse_statement, parse_expression)
src/parser/parser.rs lines 500-1200  (parse_assignment, parse_if_stmt, etc.)
src/parser/parser.ps lines 1-1000    (what we have)
```

---

### **Day 3-4: Implement Statement Parsers**

**Tasks**:
1. **Assignment Parser** (60 lines)
   ```pasta
   DEF parse_assignment():
       # Pattern: target = expr
       # Handle: x = y = z (chained)
       # Handle: obj.field = expr
       # Handle: list[idx] = expr
   END
   ```

2. **Function Definition Parser** (100 lines)
   ```pasta
   DEF parse_function_def():
       # Pattern: DEF name(params) ... END
       # Handle: default parameters
       # Handle: *args, **kwargs
       # Handle: type hints (future)
       # Handle: decorators
   END
   ```

3. **Class Definition Parser** (100 lines)
   ```pasta
   DEF parse_class_def():
       # Pattern: CLASS name(base) ... END
       # Handle: inheritance
       # Handle: field declarations
       # Handle: method definitions
       # Handle: property decorators
   END
   ```

4. **Control Flow Parsers** (150 lines)
   ```pasta
   DEF parse_if_stmt():        # IF ... ELIF ... ELSE ... END
   DEF parse_while_stmt():     # WHILE cond ... END
   DEF parse_for_stmt():       # FOR x IN iter ... END
   DEF parse_do_block():       # DO ... END (parallel)
   DEF parse_spawn_stmt():     # SPAWN task
   END
   ```

5. **Import/Module Parsers** (80 lines)
   ```pasta
   DEF parse_import_stmt():    # FROM mod IMPORT x, y
   DEF parse_use_stmt():       # USE mod AS alias
   DEF parse_with_stmt():      # WITH expr AS x ... END
   END
   ```

6. **Exception Handling** (100 lines)
   ```pasta
   DEF parse_try_except_stmt():    # TRY ... EXCEPT ... FINALLY ... END
   END
   ```

7. **Other Statements** (80 lines)
   ```pasta
   DEF parse_return_stmt():    # RETURN expr
   DEF parse_break_stmt():     # BREAK
   DEF parse_continue_stmt():  # CONTINUE
   DEF parse_yield_stmt():     # YIELD expr
   END
   ```

**Subtotal**: ~670 lines of parser logic

---

### **Day 5: Expression Parsing Completion**

**Tasks**:
1. **Prefix Operators** (60 lines)
   - `not`, `-`, `+`, `@` (each with correct precedence)
   
2. **Postfix Operators** (100 lines)
   - Method calls: `obj.method(args)`
   - Indexing: `list[idx]`
   - Slicing: `list[start:end:step]`
   - Function calls: `func(args)`

3. **Complex Expressions** (150 lines)
   - List comprehensions: `[x for x in list if cond]`
   - Lambda expressions: `LAMBDA x: x + 1`
   - Pipelines: `expr | func1 | func2`
   - Ternary: `a IF cond ELSE b`

4. **Edge Cases** (80 lines)
   - Operator precedence edge cases
   - Parenthesized expressions
   - Multiline expressions
   - Comments in expressions

**Subtotal**: ~390 lines

---

### **Day 6: Error Recovery & Diagnostics**

**Tasks**:
1. **Error Recovery** (80 lines)
   - Synchronization points (statement boundaries)
   - Skip to next statement on parse error
   - Collect multiple errors instead of failing on first
   
2. **Diagnostic Messages** (60 lines)
   - Line/column reporting
   - Context display (show line with caret)
   - Helpful suggestions ("did you mean...?")
   - Error codes for machine parsing

3. **Testing** (20 lines)
   - ParseError propagation
   - Diagnostic collection

**Subtotal**: ~160 lines

---

### **Day 7: Parser Testing & Validation**

**Create**: `tests/phase4a_parser_pasta.rs` (50+ test cases)

**Test Categories**:
1. **Basic Expressions** (10 tests)
   - Literals: `1`, `"str"`, `[]`, `{}`
   - Variables: `x`, `obj.field`
   - Binary ops: `a + b`, `a and b`

2. **Statements** (15 tests)
   - Assignment: `x = 1`, `a, b = 1, 2`
   - Functions: `DEF f(x): RETURN x END`
   - Classes: `CLASS C: field = 1 END`
   - Control flow: `IF x: ... END`

3. **Complex** (15 tests)
   - Nested functions, classes
   - Comprehensions
   - Pipelines
   - Full programs

4. **Error Cases** (10 tests)
   - Missing terminators
   - Unmatched parentheses
   - Invalid syntax

**Expected Result**: ✅ All 50+ tests passing, no parse errors

---

## 📅 **WEEK 2: IR Generator Implementation**

### **Goal**: AST → IR translation for all node types

### **Day 8-9: IR Design & Constants**

**Tasks**:
1. **Define IR Node Types** (150 lines)
   ```pasta
   # IR node constructors
   DEF ir_literal(value):
       RETURN({"type": "Literal", "value": value})
   END
   
   DEF ir_variable(name, scope_depth):
       RETURN({"type": "Variable", "name": name, "scope": scope_depth})
   END
   
   DEF ir_binop(op, left, right):
       RETURN({"type": "BinOp", "op": op, "left": left, "right": right})
   END
   
   # ... 20+ more node types
   ```

2. **Define Scope Management** (100 lines)
   ```pasta
   DEF scope_new(parent):
       RETURN({
           "parent": parent,
           "vars": {},
           "depth": parent and (parent["depth"] + 1) or 0
       })
   END
   
   DEF scope_declare(scope, name):
       # Track variable declarations
   END
   
   DEF scope_lookup(scope, name):
       # Find variable in scope chain
   END
   ```

3. **Define IR Context** (100 lines)
   ```pasta
   DEF ir_context_new():
       RETURN({
           "scopes": [scope_new(None)],
           "functions": {},
           "classes": {},
           "imports": {}
       })
   END
   ```

**Subtotal**: ~350 lines of IR infrastructure

---

### **Day 10-11: Expression IR Generation**

**Tasks**:
1. **Expression Translation** (200 lines)
   ```pasta
   DEF ir_from_expr(ast_expr, ctx):
       IF ast_expr["type"] == "Literal":
           RETURN(ir_literal(ast_expr["value"]))
       ELIF ast_expr["type"] == "Variable":
           RETURN(ir_variable(ast_expr["name"], ctx["scope_depth"]))
       ELIF ast_expr["type"] == "BinOp":
           left = ir_from_expr(ast_expr["left"], ctx)
           right = ir_from_expr(ast_expr["right"], ctx)
           RETURN(ir_binop(ast_expr["op"], left, right))
       ELIF ast_expr["type"] == "Call":
           # Function call translation
       ELIF ast_expr["type"] == "Index":
           # List/map indexing
       ELIF ast_expr["type"] == "MethodCall":
           # Object method call
       # ... more cases
       END
   END
   ```

2. **Special Operators** (100 lines)
   - Pipeline: `x | func` → `func(x)`
   - Comprehension: `[x for x in list]` → `map(lambda x: x, list)`
   - Lambda: `LAMBDA x: expr` → function IR node

3. **Error Handling** (50 lines)
   - Invalid expression types
   - Missing symbol resolution
   - Type validation (catch obvious errors)

**Subtotal**: ~350 lines

---

### **Day 12-13: Statement IR Generation**

**Tasks**:
1. **Statement Translation** (300 lines)
   ```pasta
   DEF ir_from_stmt(ast_stmt, ctx):
       IF ast_stmt["type"] == "Assignment":
           # x = expr → IR assign
       ELIF ast_stmt["type"] == "FunctionDef":
           # DEF f(x): ... → IR function
       ELIF ast_stmt["type"] == "If":
           # IF ... ELIF ... ELSE ... → IR conditional
       ELIF ast_stmt["type"] == "While":
           # WHILE cond: ... → IR loop
       ELIF ast_stmt["type"] == "For":
           # FOR x IN iter: ... → IR iterator loop
       # ... more statements
       END
   END
   ```

2. **Scope Management** (100 lines)
   - Enter scope on function/class definition
   - Exit scope on statement end
   - Track variable lifetimes

3. **Control Flow** (100 lines)
   - Return statements → IR return
   - Break/continue → IR jump
   - Try/except → IR exception handling

**Subtotal**: ~500 lines

---

### **Day 14: IR Optimization & Testing**

**Tasks**:
1. **Constant Folding** (50 lines)
   - `1 + 2` → `3` (at IR generation time)
   - `"a" .. "b"` → `"ab"`

2. **Dead Code Elimination** (50 lines)
   - Remove unreachable statements after unconditional return
   - Remove unused variables

3. **Create Test Suite**: `tests/phase4a_ir_generator.rs` (30 test programs)
   - Verify IR correctness via execution traces
   - Compare against Rust IR generator output
   - Validate all statement/expression types

**Expected Result**: ✅ 30+ IR programs correct, optimization working

---

## 📅 **WEEK 3: Bytecode Backend Implementation**

### **Goal**: IR → Bytecode compilation

### **Day 15-16: Bytecode Design**

**Tasks**:
1. **Define Bytecode Format** (150 lines)
   ```pasta
   # Bytecode opcodes
   BYTECODE = {
       "LOAD_CONST": 1,        # Push constant (arg: const_id)
       "LOAD_VAR": 2,          # Load variable (arg: var_name)
       "STORE_VAR": 3,         # Store variable (arg: var_name)
       "LOAD_ATTR": 4,         # Load object attribute (arg: attr_name)
       "STORE_ATTR": 5,        # Store object attribute
       "LOAD_INDEX": 6,        # Load from list/map
       "STORE_INDEX": 7,       # Store to list/map
       "CALL_FUNC": 8,         # Call function (arg: num_args)
       "CALL_METHOD": 9,       # Call method (arg: num_args)
       "BINOP": 10,            # Binary operation (arg: op_code)
       "UNOP": 11,             # Unary operation (arg: op_code)
       "JUMP": 12,             # Unconditional jump (arg: target)
       "JUMP_IF_TRUE": 13,     # Jump if true (arg: target)
       "JUMP_IF_FALSE": 14,    # Jump if false (arg: target)
       "RETURN": 15,           # Return from function
       "PUSH_SCOPE": 16,       # Push new scope
       "POP_SCOPE": 17,        # Pop scope
       # ... 20+ more opcodes
   }
   ```

2. **Bytecode Instruction Format** (100 lines)
   ```pasta
   DEF bytecode_instruction(opcode, arg1, arg2, arg3):
       # Pack into byte sequence
       RETURN([opcode, arg1 or 0, arg2 or 0, arg3 or 0])
   END
   ```

3. **Constant/Variable Tables** (100 lines)
   ```pasta
   DEF bytecode_context_new():
       RETURN({
           "constants": [],     # List of literal values
           "variables": {},     # Map: name → var_id
           "functions": {},     # Map: name → function_id
           "code": []          # List of bytecode instructions
       })
   END
   ```

**Subtotal**: ~350 lines

---

### **Day 17-18: IR → Bytecode Compiler**

**Tasks**:
1. **Expression Compilation** (200 lines)
   ```pasta
   DEF bytecode_from_expr(ir_expr, ctx):
       IF ir_expr["type"] == "Literal":
           const_id = list_len(ctx["constants"])
           list_append(ctx["constants"], ir_expr["value"])
           RETURN([bytecode_instruction("LOAD_CONST", const_id)])
       ELIF ir_expr["type"] == "Variable":
           var_id = ctx["variables"].get(ir_expr["name"])
           RETURN([bytecode_instruction("LOAD_VAR", var_id)])
       ELIF ir_expr["type"] == "BinOp":
           left_code = bytecode_from_expr(ir_expr["left"], ctx)
           right_code = bytecode_from_expr(ir_expr["right"], ctx)
           op_code = binop_code(ir_expr["op"])
           RETURN(left_code .. right_code .. 
                  [bytecode_instruction("BINOP", op_code)])
       # ... more cases
       END
   END
   ```

2. **Statement Compilation** (200 lines)
   - Assignment → STORE_VAR
   - Function definition → package bytecode + register
   - Control flow → JUMP instructions
   - Try/except → exception handling opcodes

3. **Function Compilation** (100 lines)
   - Compile function body to bytecode
   - Generate function prologue/epilogue
   - Handle return statements

**Subtotal**: ~500 lines

---

### **Day 19: Optimization & Linking**

**Tasks**:
1. **Bytecode Optimization** (80 lines)
   - Constant folding at bytecode level
   - Dead code elimination
   - Jump optimization (remove redundant jumps)

2. **Bytecode Linking** (100 lines)
   - Resolve forward references (jumps, function calls)
   - Generate final bytecode file (.pobj format)
   - Symbol table serialization

3. **Create Test Suite**: `tests/phase4a_codegen.rs` (20 test programs)
   - Generate bytecode for simple programs
   - Execute bytecode and verify output
   - Validate stack operations

**Expected Result**: ✅ 20+ bytecode programs executing correctly

---

## 📅 **WEEK 4: Integration & End-to-End Testing**

### **Goal**: Full compilation pipeline working, stress tested

### **Day 20-21: Create `pasta_compiler.pasta`**

**File**: `[NEW] pasta_compiler.pasta` (main entry point)

```pasta
# ============================================================================
# PASTA Compiler v2.0 — Self-Hosted Bootstrap Compiler
# Compiles .pasta source files to .pobj object files
# ============================================================================

USE pasta::io
USE pasta::sys
USE pasta::cli

DEF main(args):
    # Parse command-line arguments
    IF list_len(args) < 2:
        PRINT("Usage: pasta_compiler <input.pasta> [output.pobj]")
        RETURN(1)
    END
    
    input_file = args[1]
    output_file = args[2] or (input_file .. ".pobj")
    
    TRY:
        # Step 1: Read source file
        PRINT("📖 Reading " .. input_file)
        source = read_file(input_file)
        
        # Step 2: Lexical analysis
        PRINT("📝 Tokenizing...")
        tokens = lexer_tokenize(source)
        
        # Step 3: Parsing
        PRINT("🌳 Parsing...")
        parser = Parser_new(tokens)
        ast = parser_parse(parser)
        
        # Step 4: IR generation
        PRINT("🔧 Generating IR...")
        ir_ctx = ir_context_new()
        ir = ir_from_ast(ast, ir_ctx)
        
        # Step 5: Bytecode generation
        PRINT("⚙️  Generating bytecode...")
        bytecode_ctx = bytecode_context_new()
        bytecode = bytecode_from_ir(ir, bytecode_ctx)
        
        # Step 6: Write output
        PRINT("💾 Writing " .. output_file)
        write_file(output_file, bytecode)
        
        PRINT("✅ Compilation successful!")
        RETURN(0)
    EXCEPT err:
        PRINT("❌ Compilation failed: " .. err)
        RETURN(1)
    END
END

# Run compiler
IF name == "__main__":
    exit_code = main(sys_argv())
    sys_exit(exit_code)
END
```

---

### **Day 22: Cross-Compilation Testing**

**Tasks**:
1. **Build bootstrapped compiler using Rust**
   - Compile lexer.ps, parser.ps, ir_gen.ps, codegen.ps using PASTA interpreter
   - Run pasta_compiler.pasta as script
   
2. **Test on small programs**
   ```pasta
   # test1.pasta
   x = 1 + 2
   PRINT(x)
   
   # test2.pasta
   DEF fib(n):
       IF n <= 1:
           RETURN(n)
       END
       RETURN(fib(n-1) + fib(n-2))
   END
   
   PRINT(fib(10))
   ```

3. **Validate output against Rust compiler**
   - Compile same programs with both compilers
   - Compare bytecode output (should be identical)

---

### **Day 23-24: Stress Testing & Documentation**

**Tasks**:
1. **Stress Test Suite** (50+ test programs)
   - All examples/ directory
   - Large programs (100+ lines)
   - Nested functions, classes, control flow
   - Real-world PASTA code

2. **Performance Benchmarking**
   - Measure compilation time
   - Measure bytecode size
   - Compare vs. Rust compiler

3. **Documentation**
   - Create COMPILER_ARCHITECTURE.md
   - Document each component (lexer, parser, IR, codegen)
   - Maintenance guide for future updates

---

## ✅ **Success Criteria (Week 4)**

- [ ] Parser 100% complete (1,892 lines)
- [ ] IR generator handles all node types
- [ ] Bytecode codegen produces correct bytecode
- [ ] `pasta_compiler.pasta` compiles end-to-end
- [ ] 50+ stress tests all passing
- [ ] Output matches Rust compiler
- [ ] Documentation complete

**Outcome**: **Phase 4A Complete — Full compiler in PASTA**

---

## 🎯 **Metrics & Checkpoints**

| Checkpoint | Expected | Critical? |
|-----------|----------|-----------|
| **Day 7 Parser Tests** | 50/50 passing | ✅ YES |
| **Day 14 IR Tests** | 30/30 passing | ✅ YES |
| **Day 19 Codegen Tests** | 20/20 passing | ✅ YES |
| **Day 24 Stress Tests** | 50/50 passing | ✅ YES |
| **Compilation Time** | < 5 sec/file | ⚠️ NO (optimize Phase 5) |

---

## 🔄 **Phase 4B: Bootstrap (Next 2-3 weeks)**

Once Phase 4A is complete:
1. Compile all `.ps` files using PASTA interpreter
2. Generate initial `pasta_compiler` binary
3. Run generated compiler on itself
4. Validate bit-identical output
5. **Milestone**: v2.0.0-alpha1 (Self-Hosting)

---

## 📝 **Daily Standup Template**

```
Day X: [Component]
✅ Completed: [task description]
📊 Progress: [X%] (e.g., 70% of parser done)
🎯 Next: [next day's task]
❌ Blockers: [if any]
```

Use this template in session checkpoints for tracking progress.

---

**Ready to start Phase 4A Week 1?** Authorize parsing work and create initial pull request branch.
