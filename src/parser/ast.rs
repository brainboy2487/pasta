#![allow(missing_docs)]
//! AST node definitions for PASTA parser
//!
//! Canonical AST used by the parser and executor. Nodes carry `Span`
//! information for diagnostics and for RTX incremental compilation keys.
//!
//! All AST types derive Serialize/Deserialize for binary module support.

use std::fmt;
use serde::{Serialize, Deserialize};

/// Source span for diagnostics and error messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Span {
    /// 1-based line where the node begins.
    pub start_line: usize,
    /// 1-based column where the node begins.
    pub start_col: usize,
    /// 1-based line where the node ends.
    pub end_line: usize,
    /// 1-based column where the node ends.
    pub end_col: usize,
}

impl Span {
    /// Construct a span from start and end line/column pairs.
    pub fn new(sl: usize, sc: usize, el: usize, ec: usize) -> Self {
        Self { start_line: sl, start_col: sc, end_line: el, end_col: ec }
    }
    /// A zero-span used as a placeholder in synthesised nodes.
    pub fn dummy() -> Self { Self::new(0, 0, 0, 0) }
}

/// Top-level AST node: a program is a sequence of statements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Program {
    /// Top-level sequence of statements in the compiled program.
    pub statements: Vec<Statement>,
}

impl Program {
    /// Wrap a list of statements into a `Program`.
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements }
    }
}

/// Simple identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identifier {
    /// The raw identifier string.
    pub name: String,
    /// Source location of this identifier.
    pub span: Span,
}

impl Identifier {
    /// Construct an identifier with a name and source span.
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self { name: name.into(), span }
    }
}

/// Field declaration inside an object family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FieldDecl {
    /// Name of the declared field.
    pub name: Identifier,
    /// Default value expression.
    pub value: Expr,
    /// Source location.
    pub span: Span,
}

/// Mutation table entry (a named mutation rule).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MutEntry {
    /// Name of this mutation rule.
    pub name: Identifier,
    /// Body can be a block of statements or a single expression.
    pub body: MutBody,
    /// Source location.
    pub span: Span,
}

/// Body of a mutation table entry — a block or a single expression.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum MutBody {
    /// A `DO: ... END` block of statements.
    Block(Vec<Statement>),
    /// A single inline expression.
    Expr(Expr),
}

/// Constructor node for object families.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Constructor {
    pub params: Vec<Identifier>,
    pub body: Vec<Statement>,
    pub span: Span,
}

/// Method declaration inside an object family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MethodDecl {
    /// Method name.
    pub name: Identifier,
    /// Formal parameter names.
    pub params: Vec<Identifier>,
    /// Body statements.
    pub body: Vec<Statement>,
    /// Source location.
    pub span: Span,
}

/// Spawn entry (lhs @ rhs : actions END)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SpawnEntry {
    pub father_expr: Expr,
    pub father_family: Option<String>,
    pub mother_expr: Expr,
    pub mother_family: Option<String>,
    pub actions: Vec<Statement>,
    pub span: Span,
}

/// Module import group used by FROM/USE blocks.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleImportGroup {
    /// Module name string (as identifier).
    pub module: Identifier,
    /// List of items imported from the module.
    pub uses: Vec<UseItem>,
    /// Source location.
    pub span: Span,
}

/// Single item in a `USE:` list, with optional aliasing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UseItem {
    pub name: Identifier,
    pub alias: Option<Identifier>,
    pub span: Span,
}

/// Top-level module declaration: `MOD Name:` ... `END`
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModuleDecl {
    pub name: Identifier,
    pub exports: Vec<Identifier>,
    pub body: Vec<Statement>,
    pub span: Span,
}

/// DEF DO ... UNTIL with optional LX annotation
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DefDoUntil {
    pub name: Identifier,
    pub until_condition: Expr,
    pub body: Vec<Statement>,
    pub lx_condition: Option<Expr>,
    pub span: Span,
}

/// Scope modifier for WHILE/FOR/IF blocks.
///
/// - `None` (default): no new scope is created; variables are visible in the enclosing scope.
/// - `UnbindScope`: push a Block scope — variables die when the block exits.
/// - `BindScope`: push a Block scope but hoist all new variables to the nearest Function/Global scope on exit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScopeModifier {
    UnbindScope,
    BindScope,
}

/// Statement kinds in PASTA.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Statement {
    /// Assignment: `x = expr` or `set x = expr`
    Assignment {
        target: Identifier,
        value: Expr,
        span: Span,
    },

    /// Constant declaration: `CONST x = expr` — immutable after first assignment.
    ConstAssignment {
        target: Identifier,
        value: Expr,
        span: Span,
    },

    /// Multi-label assignment: `a, b, c = expr` — assigns the same value to each target.
    MultiAssignment {
        targets: Vec<Identifier>,
        value: Expr,
        span: Span,
    },

    /// Object declaration: `OBJ.<GROUP>.MUT Name(params): ... END`
    ObjDecl {
        name: Identifier,
        family_group: String,
        params: Vec<Identifier>,
        fields: Vec<FieldDecl>,
        mutation_table: Vec<MutEntry>,
        constructor: Option<Constructor>,
        methods: Vec<MethodDecl>,
        span: Span,
    },

    /// SPAWN block containing multiple spawn entries
    SpawnBlock {
        /// The individual spawn pair entries.
        entries: Vec<SpawnEntry>,
        /// Source location.
        span: Span,
    },

    /// DEF DO ... UNTIL (with optional LX)
    DefDoUntil(DefDoUntil),

    /// DO block (generic): DO targets [AS alias] [FOR repeats] : body
    DoBlock {
        /// Variables driven by this block.
        targets: Vec<Identifier>,
        /// Optional alias for the iteration value.
        alias: Option<Identifier>,
        /// Optional repeat-count expression list.
        repeats: Option<Vec<Expr>>,
        /// Optional timed duration in milliseconds (`DO x FOR 500ms END`).
        /// When set, the block loops for this many milliseconds instead of a fixed count.
        duration_ms: Option<Expr>,
        /// Body statements.
        body: Vec<Statement>,
        /// Source location.
        span: Span,
    },

    /// Function definition: `DEF name(params): ... END`
    FunctionDef {
        name: Identifier,
        params: Vec<Identifier>,
        body: Vec<Statement>,
        span: Span,
    },

    /// WHILE block variant
    WhileBlock {
        /// Variables driven by this block.
        targets: Vec<Identifier>,
        /// Optional loop variable alias.
        alias: Option<Identifier>,
        /// Loop continuation condition.
        condition: Expr,
        /// Body statements.
        body: Vec<Statement>,
        /// Optional scope modifier (UNBIND_SCOPE / BIND_SCOPE).
        scope_modifier: Option<ScopeModifier>,
        /// Source location.
        span: Span,
    },

    /// FOR x IN iterable: ... END — iterates over list, string, or range.
    ForIn {
        /// The loop variable bound on each iteration.
        var: Identifier,
        /// The expression producing the iterable value.
        iterable: Expr,
        /// Body statements executed once per element.
        body: Vec<Statement>,
        /// Optional scope modifier (UNBIND_SCOPE / BIND_SCOPE).
        scope_modifier: Option<ScopeModifier>,
        /// Source location.
        span: Span,
    },

    /// Module declaration: `MOD Name: ... END` with `export` declarations.
    ModuleDecl {
        /// Module name identifier.
        name: Identifier,
        /// Explicit export list (names exported by the module).
        exports: Vec<Identifier>,
        /// Module body statements.
        body: Vec<Statement>,
        /// Source location.
        span: Span,
    },

    /// BREAK — exits the enclosing loop immediately.
    Break {
        /// Source location.
        span: Span,
    },

    /// CONTINUE — skips to the next iteration of the enclosing loop.
    Continue {
        /// Source location.
        span: Span,
    },

    /// Priority override: `A OVER B`
    PriorityOverride {
        /// The higher-priority thread identifier.
        higher: Identifier,
        /// The lower-priority thread identifier.
        lower: Identifier,
        /// Source location.
        span: Span,
    },

    /// Constraint: `<expr> [relation] <expr> LIMIT OVER <expr>`
    Constraint {
        /// Left-hand side of the relation.
        left: Expr,
        /// Optional relational operator.
        relation: Option<RelationToken>,
        /// Right-hand side of the relation.
        right: Expr,
        /// The limit expression after `LIMIT OVER`.
        constraint: Expr,
        /// Source location.
        span: Span,
    },

    /// Expression statement
    ExprStmt {
        /// The expression evaluated for side effects.
        expr: Expr,
        /// Source location.
        span: Span,
    },

    /// FROM block for importing module symbols lazily.
    FromBlock {
        /// List of per-module import groups.
        imports: Vec<ModuleImportGroup>,
        /// Source location.
        span: Span,
    },

    /// PRINT statement
    Print {
        /// Expression whose value is printed.
        expr: Expr,
        /// Source location.
        span: Span,
    },

    /// IF statement with optional OTHERWISE
    If {
        /// One or more condition expressions.
        conditions: Vec<Expr>,
        /// Statements executed when the condition holds.
        then_body: Vec<Statement>,
        /// Optional alternative branch (OTHERWISE / ELSE).
        else_body: Option<Vec<Statement>>,
        /// Optional scope modifier (UNBIND_SCOPE / BIND_SCOPE).
        scope_modifier: Option<ScopeModifier>,
        /// Source location.
        span: Span,
    },

    /// END marker
    End {
        span: Span,
    },

    /// RET.NOW(): expr  — return expr immediately, stop function execution
    RetNow {
        /// Value to return.
        value: Expr,
        /// Source location.
        span: Span,
    },

    /// RET.LATE(condition): expr  — snapshot expr now, deliver when condition met
    RetLate {
        /// Value snapshotted at the call site.
        value: Expr,
        /// Delivery condition (time or boolean guard).
        condition: RetLateCondition,
        /// Source location.
        span: Span,
    },

    /// ATTEMPT(err_var): DO: <try_body> END  ELSE: DO: <else_body> END  END
    /// Pasta equivalent of try/except.
    /// `err_var` is bound to the runtime error message string inside else_body scope.
    AttemptBlock {
        err_var: Identifier,
        try_body: Vec<Statement>,
        else_body: Vec<Statement>,
        span: Span,
    },

    /// TRY: <try_body> OTHERWISE: <else_body> END
    /// Simple exception handling without error capture.
    TryBlock {
        try_body: Vec<Statement>,
        else_body: Vec<Statement>,
        span: Span,
    },

    // ═══════════════════════════════════════════════════════════════════════════
    // v1.4.4 POINTER SYSTEM STATEMENTS
    // ═══════════════════════════════════════════════════════════════════════════

    /// `name = LOOP ... END` — named loop block; `GOTO name` jumps back to its top.
    LoopBlock {
        /// The label name assigned to this loop (from `name = LOOP`).
        name: String,
        /// Body statements executed on each iteration.
        body: Vec<Statement>,
        /// Source location.
        span: Span,
    },

    /// `GOTO <label>` — jumps to the top of the named LoopBlock.
    GotoLabel {
        /// Name of the LoopBlock to jump to.
        label: String,
        /// Source location.
        span: Span,
    },

    /// `GOTO <ptr>: ... END` — sets the active pointer context for the body.
    GotoBlock {
        /// Name of the variable holding the pointer.
        name: String,
        /// Body statements (run exactly once, unless GOTO name restarts).
        body: Vec<Statement>,
        /// Source location.
        span: Span,
    },

    /// PULL.<TYPE> [ptr] -> target
    /// Reads from active pointer context or an explicit pointer
    Pull {
        /// Data type (BYTE, INT, FLOAT, STR, BYTES)
        dtype: String,
        /// Explicit pointer expression (optional; uses context if absent)
        explicit_ptr: Option<Box<Expr>>,
        /// Optional arguments (e.g., length)
        args: Vec<Expr>,
        /// Variable to store result (if any)
        target: Option<Identifier>,
        /// Source location
        span: Span,
    },

    /// PUSH.<TYPE> [ptr,] <expr>
    /// Writes to active pointer context or an explicit pointer
    Push {
        /// Data type (BYTE, INT, FLOAT, STR, BYTES)
        dtype: String,
        /// Explicit pointer expression (optional; uses context if absent)
        explicit_ptr: Option<Box<Expr>>,
        /// Value to write
        value: Expr,
        /// Optional arguments
        args: Vec<Expr>,
        /// Source location
        span: Span,
    },

    /// <var> = ALLOC.<KIND>(args)
    /// Allocates a new pointer resource
    Alloc {
        /// Variable to store the pointer ID
        target: Identifier,
        /// Pointer kind (MEM, FILE, DEV, NET)
        kind: String,
        /// Allocation arguments (size, path, etc.)
        args: Vec<Expr>,
        /// Optional WITH metadata block
        metadata: Vec<(String, Expr)>,
        /// Source location
        span: Span,
    },

    /// FREE <expr>
    /// Releases a pointer resource
    Free {
        /// Expression evaluating to pointer ID
        pointer_expr: Expr,
        /// Source location
        span: Span,
    },

    /// INFO <expr>
    /// Returns metadata about a pointer
    Info {
        /// Expression evaluating to pointer ID
        pointer_expr: Expr,
        /// Variable to store result
        target: Option<Identifier>,
        /// Source location
        span: Span,
    },

    /// SEEK <pointer>, <offset>
    /// Sets the read/write offset for a pointer
    Seek {
        /// Expression evaluating to pointer ID
        pointer_expr: Expr,
        /// Offset expression (integer)
        offset_expr: Expr,
        /// Source location
        span: Span,
    },

    /// SWAP <var1>, <var2>
    /// Swaps the values of two variables
    Swap {
        /// First variable name
        var1: Identifier,
        /// Second variable name
        var2: Identifier,
        /// Source location
        span: Span,
    },

    /// Catch-all for unimplemented or future statement kinds.
    Other {
        /// String tag identifying the statement kind.
        kind: String,
        /// Optional payload string for debugging.
        payload: Option<String>,
        /// Source location.
        span: Span,
    },

    /// ::USE UNSAFE-READ:: or ::USE UNSAFE-WRITE:: pragma
    UseUnsafe {
        write_access: bool,  // false = READ only, true = READ+WRITE
        span: Span,
    },
}

/// Condition for a RET.LATE statement.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetLateCondition {
    /// Fire after N milliseconds (wall clock).
    AfterMs(Expr),
    /// Fire when the named function is called.
    WhenCalled(String),
}

/// Expression kinds used in statements and constraints.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Expr {
    /// A numeric literal.
    Number(f64, Span),
    /// A string literal.
    String(String, Span),
    /// A boolean literal.
    Bool(bool, Span),
    /// A None/null literal.
    None(Span),
    /// A variable or name reference.
    Identifier(Identifier),

    /// Object constructor call: `Name(arg=val, ...)`
    ConstructorCall {
        /// The family/constructor name.
        family_name: Identifier,
        /// Positional or keyword argument expressions.
        args: Vec<Expr>,
        /// Source location.
        span: Span,
    },

    /// Combine operator between objects: `left + right` (semantic typed)
    Combine {
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
        /// Source location.
        span: Span,
    },

    /// Reassignment / family cast
    Reassign {
        target_family: String,
        expr: Box<Expr>,
        span: Span,
    },

    /// A binary operator expression.
    Binary {
        /// The operator.
        op: BinaryOp,
        /// Left operand.
        left: Box<Expr>,
        /// Right operand.
        right: Box<Expr>,
        /// Source location.
        span: Span,
    },

    /// A function or method call expression.
    Call {
        /// The expression that resolves to the callable.
        callee: Box<Expr>,
        /// Argument expressions.
        args: Vec<Expr>,
        /// Source location.
        span: Span,
    },

    /// A list literal: `[a, b, c]`.
    List {
        /// Element expressions.
        items: Vec<Expr>,
        /// Source location.
        span: Span,
    },

    /// A raw/unparsed string expression used as a fallback.
    Raw(String, Span),

    /// A deferred block of statements stored as a first-class value.
    Lambda(Vec<Statement>, Span),

    /// Tensor builder wrapping nested list expressions for tensor literals.
    TensorBuilder {
        /// The nested list expression providing tensor data.
        expr: Box<Expr>,
        /// Source location.
        span: Span,
    },

    /// Subscript/index: `expr[idx]` or `expr[i, j]`
    Index {
        /// The expression being indexed.
        base: Box<Expr>,
        /// One or more index expressions.
        indices: Vec<Expr>,
        /// Source location.
        span: Span,
    },

    // ═══════════════════════════════════════════════════════════════════════════
    // v1.4.4 POINTER SYSTEM EXPRESSIONS
    // ═══════════════════════════════════════════════════════════════════════════

    /// REF.<KIND>(target) WITH { metadata }
    /// Creates a reference/pointer expression
    Ref {
        /// Pointer kind (MEM, FILE, DEV, NET)
        kind: String,
        /// Target expression (size for MEM, path for FILE, etc.)
        target: Box<Expr>,
        /// Optional metadata key-value pairs
        metadata: Vec<(String, Expr)>,
        /// Source location
        span: Span,
    },

    /// OBJ.GROUP[.MUT](parentA, parentB) — create a family node
    ObjFamNew {
        group:    String,  // "LST", "DICT", "TNSR", "NRML", "CSM"
        mutable:  bool,
        parent_a: Box<Expr>,
        parent_b: Box<Expr>,
        span:     Span,
    },
    /// DOES_PARENT_EXIST expr — boolean check
    DoesParentExist {
        target: Box<Expr>,
        span:   Span,
    },

    /// Dict literal: `{"key": expr, ...}`
    Dict {
        /// Key-value pairs; keys are typically string literal exprs.
        pairs: Vec<(Expr, Expr)>,
        /// Source location.
        span: Span,
    },
}

impl Expr {
    /// Return the source span of this expression node.
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, s) => s.clone(),
            Expr::String(_, s) => s.clone(),
            Expr::Bool(_, s) => s.clone(),
            Expr::None(s) => s.clone(),
            Expr::Identifier(id) => id.span.clone(),
            Expr::ConstructorCall { span, .. } => span.clone(),
            Expr::Combine { span, .. } => span.clone(),
            Expr::Reassign { span, .. } => span.clone(),
            Expr::Binary { span, .. } => span.clone(),
            Expr::Call { span, .. } => span.clone(),
            Expr::List { span, .. } => span.clone(),
            Expr::Raw(_, s) => s.clone(),
            Expr::Lambda(_, s) => s.clone(),
            Expr::TensorBuilder { span, .. } => span.clone(),
            Expr::Index { span, .. } => span.clone(),
            Expr::Ref { span, .. } => span.clone(),
            Expr::ObjFamNew { span, .. } => span.clone(),
            Expr::DoesParentExist { span, .. } => span.clone(),
            Expr::Dict { span, .. } => span.clone(),
        }
    }
}

/// Binary operators.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum BinaryOp {
    /// `+` — addition or object combine.
    Add,
    /// `-` — subtraction.
    Sub,
    /// `*` — multiplication.
    Mul,
    /// `/` — division.
    Div,
    /// `%` — modulo / remainder
    Mod,
    /// `^` — exponentiation
    Pow,
    /// `@` — matrix multiply
    MatMul,
    /// `==` — equality comparison.
    Eq,
    /// `!=` — inequality comparison.
    Neq,
    /// `<` — less than.
    Lt,
    /// `>` — greater than.
    Gt,
    /// `<=` — less than or equal.
    Lte,
    /// `>=` — greater than or equal.
    Gte,
    /// Loose approximate equality (≈)
    Approx,
    /// Not equal with type checking (≠)
    NotEq,
    /// Strict identity (≡)
    StrictEq,
    /// `AND` / `&&` — logical conjunction.
    And,
    /// `OR` / `||` — logical disjunction.
    Or,
    /// `NOT` / `!` — logical negation.
    Not,
    // pipeline operators
    Pipe,       // |
    PipeOr,     // ||
    PipeBoth,   // |&|
    PipeMap,    // |:|
    PipeArrow,  // |>
    FloorDiv,   // //
    TruncDiv,   // \  (truncates toward zero)
    Shl,        // <<
    Shr,        // >>
    BitAnd,     // &

}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BinaryOp::*;
        let s = match self {
            Add => "+", Sub => "-", Mul => "*", Div => "/",
            Mod => "%", Pow => "^", MatMul => "@",
            Eq => "==", Neq => "!=", Lt => "<", Gt => ">",
            Lte => "<=", Gte => ">=",
            Approx => "≈", NotEq => "≠", StrictEq => "≡",
            And => "and", Or => "or", Not => "not",
            Pipe => "|",
            PipeOr => "||",
            PipeBoth => "|&|",
            PipeMap => "|:|",
            PipeArrow => "|>",
            FloorDiv => "//",
            TruncDiv => "\\",
            Shl => "<<",
            Shr => ">>",
            BitAnd => "&",
        };
        write!(f, "{}", s)
    }
}

/// Relation token used in constraint expressions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RelationToken {
    /// The relation keyword text, e.g. `"approaches"` or `"in"`.
    pub text: String,
    /// Source location.
    pub span: Span,
}

impl RelationToken {
    /// Construct a `RelationToken` from a text value and span.
    pub fn new(text: impl Into<String>, span: Span) -> Self {
        Self { text: text.into(), span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_simple_assignment() {
        let span = Span::new(1, 1, 1, 10);
        let stmt = Statement::Assignment {
            target: Identifier::new("x", span.clone()),
            value: Expr::Number(42.0, span.clone()),
            span: span.clone(),
        };
        match stmt {
            Statement::Assignment { target, value, .. } => {
                assert_eq!(target.name, "x");
                assert!(matches!(value, Expr::Number(n, _) if n == 42.0));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn obj_decl_node_roundtrip() {
        let span = Span::dummy();
        let name = Identifier::new("Monster", span.clone());
        let field = FieldDecl {
            name: Identifier::new("health", span.clone()),
            value: Expr::Number(100.0, span.clone()),
            span: span.clone(),
        };
        let mut_entry = MutEntry {
            name: Identifier::new("heal_small", span.clone()),
            body: MutBody::Expr(Expr::Raw("self.health = self.health + 10".into(), span.clone())),
            span: span.clone(),
        };
        let ctor = Constructor {
            params: vec![Identifier::new("params", span.clone())],
            body: vec![],
            span: span.clone(),
        };
        let stmt = Statement::ObjDecl {
            name,
            family_group: "NRML".into(),
            params: vec![],
            fields: vec![field],
            mutation_table: vec![mut_entry],
            constructor: Some(ctor),
            methods: vec![],
            span: span.clone(),
        };
        match stmt {
            Statement::ObjDecl { family_group, fields, mutation_table, .. } => {
                assert_eq!(family_group, "NRML");
                assert_eq!(fields.len(), 1);
                assert_eq!(mutation_table.len(), 1);
            }
            _ => panic!("expected ObjDecl"),
        }
    }

    #[test]
    fn attempt_block_node() {
        let span = Span::dummy();
        let stmt = Statement::AttemptBlock {
            err_var: Identifier::new("err_num", span.clone()),
            try_body: vec![],
            else_body: vec![],
            span: span.clone(),
        };
        assert!(matches!(stmt, Statement::AttemptBlock { .. }));
    }

    #[test]
    fn ret_late_condition_variants() {
        let span = Span::dummy();
        let c = RetLateCondition::AfterMs(Expr::Number(500.0, span.clone()));
        assert!(matches!(c, RetLateCondition::AfterMs(_)));
        let c2 = RetLateCondition::WhenCalled("on_complete".to_string());
        assert!(matches!(c2, RetLateCondition::WhenCalled(_)));
    }

    #[test]
    fn matmul_binop_display() {
        assert_eq!(format!("{}", BinaryOp::MatMul), "@");
    }
}
