//! AST node definitions for PASTA parser
//!
//! This file defines the canonical AST used by the parser and executor.
//! It is intentionally compact and carries `Span` information on nodes so
//! diagnostics and runtime errors can point to source locations.

use std::fmt;

/// Source span for diagnostics and error messages.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Span {
    pub start_line: usize,
    pub start_col: usize,
    pub end_line: usize,
    pub end_col: usize,
}

impl Span {
    pub fn new(sl: usize, sc: usize, el: usize, ec: usize) -> Self {
        Self { start_line: sl, start_col: sc, end_line: el, end_col: ec }
    }
    pub fn dummy() -> Self { Self::new(0, 0, 0, 0) }
}

/// Top-level AST node: a program is a sequence of statements.
#[derive(Debug, Clone)]
pub struct Program {
    pub statements: Vec<Statement>,
}

impl Program {
    pub fn new(statements: Vec<Statement>) -> Self {
        Self { statements }
    }
}

/// Statement kinds in PASTA.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Assignment: `set x = expr` or `x = expr`
    Assignment {
        target: Identifier,
        value: Expr,
        span: Span,
    },

    /// Function definition: `DEF name DO ... END`
    FunctionDef {
        name: Identifier,
        body: Vec<Statement>,
        span: Span,
    },

    /// DO block:
    /// - `DO name [AS alias] [FOR expr] : <body>`
    /// - `DO a, b FOR x, y:` (multiple targets)
    DoBlock {
        targets: Vec<Identifier>,
        alias: Option<Identifier>,
        repeats: Option<Vec<Expr>>,
        body: Vec<Statement>,
        span: Span,
    },

    /// WHILE loop:
    /// - `DO name WHILE <condition>:`
    /// - `DO a, b WHILE <condition>:`
    WhileBlock {
        targets: Vec<Identifier>,
        alias: Option<Identifier>,
        condition: Expr,
        body: Vec<Statement>,
        span: Span,
    },

    /// Priority override: `A OVER B`
    PriorityOverride {
        higher: Identifier,
        lower: Identifier,
        span: Span,
    },

    /// Constraint: `<expr> [relation] <expr> LIMIT OVER <expr>`
    Constraint {
        left: Expr,
        relation: Option<RelationToken>,
        right: Expr,
        constraint: Expr,
        span: Span,
    },

    /// Expression statement
    ExprStmt {
        expr: Expr,
        span: Span,
    },

    /// PRINT statement
    Print {
        expr: Expr,
        span: Span,
    },

    /// IF statement: `IF <cond> DO <block>` or `IF <cond> ELSE <block>`
    /// Supports OR-chained conditions: `IF c1 OR c2 OR c3 DO ...`
    If {
        /// List of conditions (OR-chained)
        conditions: Vec<Expr>,
        /// True branch (executed if any condition is truthy)
        then_body: Vec<Statement>,
        /// False branch (optional; executed if all conditions are falsy)
        else_body: Option<Vec<Statement>>,
        span: Span,
    },

    /// END marker
    End {
        span: Span,
    },

    /// Catch-all for unimplemented statement kinds
    Other {
        kind: String,
        payload: Option<String>,
        span: Span,
    },
}

/// Simple identifier wrapper.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Identifier {
    pub name: String,
    pub span: Span,
}

impl Identifier {
    pub fn new(name: impl Into<String>, span: Span) -> Self {
        Self { name: name.into(), span }
    }
}

/// Expression kinds used in statements and constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Number(f64, Span),
    String(String, Span),
    Bool(bool, Span),
    Identifier(Identifier),

    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        span: Span,
    },

    Call {
        callee: Box<Expr>,
        args: Vec<Expr>,
        span: Span,
    },

    List {
        items: Vec<Expr>,
        span: Span,
    },

    Raw(String, Span),

    /// A deferred block of statements stored as a first-class value.
    Lambda(Vec<Statement>, Span),

    /// Tensor builder: `BUILD TENSOR: <list>` where list may be nested
    /// e.g. [[1,2],[3,4]] for 2D or [[[1],[2]],[[3],[4]]] for 3D etc.
    TensorBuilder {
        expr: Box<Expr>,
        span: Span,
    },

    /// Subscript/index: `expr[idx]` or `expr[i, j]`
    /// Used for tensor indexing and list indexing.
    Index {
        base: Box<Expr>,
        indices: Vec<Expr>,
        span: Span,
    },
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::Number(_, s) => s.clone(),
            Expr::String(_, s) => s.clone(),
            Expr::Bool(_, s) => s.clone(),
            Expr::Identifier(id) => id.span.clone(),
            Expr::Binary { span, .. } => span.clone(),
            Expr::Call { span, .. } => span.clone(),
            Expr::List { span, .. } => span.clone(),
            Expr::Raw(_, s) => s.clone(),
            Expr::Lambda(_, s) => s.clone(),
            Expr::TensorBuilder { span, .. } => span.clone(),
            Expr::Index { span, .. } => span.clone(),
        }
    }
}

/// Binary operators.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    /// `@` — matrix multiply
    MatMul,
    Eq,
    Neq,
    Lt,
    Gt,
    Lte,
    Gte,
    /// Loose approximate equality (numbers within tolerance or strings similar)
    Approx,
    /// Not equal with type checking (unicode ≠)
    NotEq,
    /// Strict identity (type + value match, unicode ≡)
    StrictEq,
    And,
    Or,
    Not,
}

impl fmt::Display for BinaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use BinaryOp::*;
        let s = match self {
            Add => "+",
            Sub => "-",
            Mul => "*",
            Div => "/",
            MatMul => "@",
            Eq => "==",
            Neq => "!=",
            Lt => "<",
            Gt => ">",
            Lte => "<=",
            Gte => ">=",
            Approx => "≈",
            NotEq => "≠",
            StrictEq => "≡",
            And => "and",
            Or => "or",
            Not => "not",
        };
        write!(f, "{}", s)
    }
}

/// Relation token used in constraint expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationToken {
    pub text: String,
    pub span: Span,
}

impl RelationToken {
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
    fn lambda_expr_node() {
        let span = Span::dummy();
        let body = vec![Statement::Print {
            expr: Expr::Identifier(Identifier::new("X", span.clone())),
            span: span.clone(),
        }];
        let lam = Expr::Lambda(body, span.clone());
        assert!(matches!(lam, Expr::Lambda(_, _)));
    }

    #[test]
    fn doblock_multiple_targets() {
        let span = Span::dummy();
        let t1 = Identifier::new("a", span.clone());
        let t2 = Identifier::new("b", span.clone());
        let stmt = Statement::DoBlock {
            targets: vec![t1.clone(), t2.clone()],
            alias: None,
            repeats: None,
            body: vec![],
            span: span.clone(),
        };
        match stmt {
            Statement::DoBlock { targets, .. } => {
                assert_eq!(targets.len(), 2);
            }
            _ => panic!("expected do block"),
        }
    }

    #[test]
    fn bool_expr_node() {
        let span = Span::new(1, 1, 1, 5);
        let e = Expr::Bool(true, span.clone());
        assert!(matches!(e, Expr::Bool(true, _)));
    }

    #[test]
    fn while_block_node() {
        let span = Span::dummy();
        let target = Identifier::new("worker", span.clone());
        let cond = Expr::Bool(true, span.clone());
        let stmt = Statement::WhileBlock {
            targets: vec![target],
            alias: None,
            condition: cond,
            body: vec![],
            span: span.clone(),
        };
        match stmt {
            Statement::WhileBlock { targets, condition, .. } => {
                assert_eq!(targets.len(), 1);
                assert!(matches!(condition, Expr::Bool(true, _)));
            }
            _ => panic!("expected while block"),
        }
    }

    #[test]
    fn matmul_binop_display() {
        assert_eq!(format!("{}", BinaryOp::MatMul), "@");
    }

    #[test]
    fn index_expr_node() {
        let span = Span::dummy();
        let base = Expr::Identifier(Identifier::new("X", span.clone()));
        let idx = Expr::Number(0.0, span.clone());
        let e = Expr::Index {
            base: Box::new(base),
            indices: vec![idx],
            span: span.clone(),
        };
        assert!(matches!(e, Expr::Index { .. }));
    }
}
