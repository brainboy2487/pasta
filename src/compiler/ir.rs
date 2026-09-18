use crate::parser::ast::BinaryOp;

/// Small compiler IR for the bootstrap native compiler path.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapProgram {
    /// Global variables assigned in top-level execution order.
    pub globals: Vec<BootstrapGlobal>,
    /// Reachable compiled function definitions.
    pub functions: Vec<BootstrapFunction>,
    /// Optional shared-module metadata for native `.so` emission.
    pub module: Option<BootstrapModule>,
    /// Ordered top-level statements in the compiled program body (`main`).
    pub main: Vec<BootstrapStmt>,
}

/// A compiled global variable definition.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapGlobal {
    pub name: String,
    pub ty: ValueType,
}

/// A compiled function definition.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapFunction {
    pub name: String,
    pub params: Vec<BootstrapParam>,
    pub return_type: ValueType,
    pub body: Vec<BootstrapStmt>,
}

/// A compiled function parameter.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapParam {
    pub name: String,
    pub ty: ValueType,
}

/// Shared-module metadata emitted alongside compiled functions.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapModule {
    pub name: String,
    pub exports: Vec<BootstrapExport>,
}

/// Exported compiled module function signature.
#[derive(Debug, Clone, PartialEq)]
pub struct BootstrapExport {
    pub name: String,
    pub params: Vec<ValueType>,
    pub return_type: ValueType,
}

/// Statement subset currently supported by the bootstrap compiler.
#[derive(Debug, Clone, PartialEq)]
pub enum BootstrapStmt {
    /// Variable assignment.
    Assign {
        name: String,
        value: BootstrapExpr,
        ty: ValueType,
    },
    /// Conditional execution with optional otherwise branch.
    If {
        condition: BootstrapExpr,
        then_body: Vec<BootstrapStmt>,
        else_body: Option<Vec<BootstrapStmt>>,
    },
    /// A plain `WHILE <cond>:` loop.
    While {
        condition: BootstrapExpr,
        body: Vec<BootstrapStmt>,
    },
    /// Exit the nearest enclosing loop.
    Break,
    /// Continue the nearest enclosing loop.
    Continue,
    /// Print a value using the current static subset formatting rules.
    Print {
        value: BootstrapExpr,
    },
    /// Evaluate an expression for side effects.
    Expr {
        value: BootstrapExpr,
    },
    /// Call a runtime bridge helper for side effects.
    RuntimeCall {
        builtin: RuntimeBuiltin,
        args: Vec<BootstrapExpr>,
    },
    /// Return from a compiled function.
    Return {
        value: BootstrapExpr,
    },
}

/// Expression subset currently supported by the bootstrap compiler.
#[derive(Debug, Clone, PartialEq)]
pub enum BootstrapExpr {
    Number(f64),
    Bool(bool),
    String(String),
    None,
    Variable {
        name: String,
        ty: ValueType,
    },
    Call {
        name: String,
        args: Vec<BootstrapExpr>,
        ty: ValueType,
    },
    RuntimeCall {
        builtin: RuntimeBuiltin,
        args: Vec<BootstrapExpr>,
        ty: ValueType,
    },
    ListNumberLiteral {
        items: Vec<BootstrapExpr>,
    },
    DictStringNumberLiteral {
        pairs: Vec<(String, BootstrapExpr)>,
    },
    Color {
        r: Box<BootstrapExpr>,
        g: Box<BootstrapExpr>,
        b: Box<BootstrapExpr>,
    },
    Binary {
        op: BootstrapBinaryOp,
        left: Box<BootstrapExpr>,
        right: Box<BootstrapExpr>,
        ty: ValueType,
    },
    Not(Box<BootstrapExpr>),
}

impl BootstrapExpr {
    /// Return the static type for this bootstrap expression.
    pub fn ty(&self) -> ValueType {
        match self {
            BootstrapExpr::Number(_) => ValueType::Number,
            BootstrapExpr::Bool(_) => ValueType::Bool,
            BootstrapExpr::String(_) => ValueType::String,
            BootstrapExpr::None => ValueType::AbiValue,
            BootstrapExpr::Variable { ty, .. } => ty.clone(),
            BootstrapExpr::Call { ty, .. } => ty.clone(),
            BootstrapExpr::RuntimeCall { ty, .. } => ty.clone(),
            BootstrapExpr::ListNumberLiteral { .. } => ValueType::AbiValue,
            BootstrapExpr::DictStringNumberLiteral { .. } => ValueType::AbiValue,
            BootstrapExpr::Color { .. } => ValueType::Number,
            BootstrapExpr::Binary { ty, .. } => ty.clone(),
            BootstrapExpr::Not(_) => ValueType::Bool,
        }
    }
}

/// Runtime helper calls used by compiled executables for dynamic builtins.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RuntimeBuiltin {
    WindowNew,
    WindowPoll,
    WindowKey,
    WindowClose,
    SetDrawTarget,
    SetColorPacked,
    CanvasFillRect,
    SwapBuffer,
    FpsInit,
    FpsBegin,
    FpsEnd,
    FpsTick,
    RandInt2,
    ListLen,
    ListSlice,
    ListConcat,
    ListIndexNumber,
    DictGetNumber,
    NumberToString,
    StringConcat,
}

/// Static value types supported by the bootstrap compiler subset.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueType {
    Number,
    Bool,
    String,
    AbiValue,
}

/// Binary operators supported in the bootstrap compiler subset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BootstrapBinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    And,
    Or,
}

impl TryFrom<&BinaryOp> for BootstrapBinaryOp {
    type Error = ();

    fn try_from(value: &BinaryOp) -> Result<Self, Self::Error> {
        Ok(match value {
            BinaryOp::Add => Self::Add,
            BinaryOp::Sub => Self::Sub,
            BinaryOp::Mul => Self::Mul,
            BinaryOp::Div => Self::Div,
            BinaryOp::Mod => Self::Mod,
            BinaryOp::Eq => Self::Eq,
            BinaryOp::Neq => Self::Neq,
            BinaryOp::Lt => Self::Lt,
            BinaryOp::Gt => Self::Gt,
            BinaryOp::Lte => Self::Lte,
            BinaryOp::Gte => Self::Gte,
            BinaryOp::And => Self::And,
            BinaryOp::Or => Self::Or,
            _ => return Err(()),
        })
    }
}
