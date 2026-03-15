// src/semantics/constraints.rs
//! Constraint engine for PASTA (minimal, debug-friendly)

use std::collections::HashMap;
use anyhow::{Result, anyhow};


/// Simple representation of a constraint expression used by the engine.
#[derive(Debug, Clone)]
pub struct ConstraintExpr {
    /// Left-hand side of the constraint relation.
    pub left: ExprSimple,
    /// Optional relational operator between left and right.
    pub relation: Option<Relation>,
    /// Right-hand side of the constraint relation.
    pub right: ExprSimple,
    /// The bounding expression following `LIMIT OVER`.
    pub constraint: ExprSimple,
}

impl ConstraintExpr {
    /// Construct a `ConstraintExpr` from its components.
    pub fn new(left: ExprSimple, relation: Option<Relation>, right: ExprSimple, constraint: ExprSimple) -> Self {
        Self { left, relation, right, constraint }
    }
}

/// Minimal relation enum used by constraints.
#[derive(Debug, Clone, Copy)]
pub enum Relation {
    /// `equals` / `==` relation.
    Equals,
    /// `<` relation.
    LessThan,
    /// `>` relation.
    GreaterThan,
}

impl Relation {
    /// Parse a relation keyword string into a `Relation` variant.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "==" | "=" => Some(Relation::Equals),
            "<" => Some(Relation::LessThan),
            ">" => Some(Relation::GreaterThan),
            _ => None,
        }
    }
}

/// Simplified expression form for constraints.
#[derive(Debug, Clone)]
pub enum ExprSimple {
    /// A named variable reference.
    Identifier(String),
    /// A numeric literal.
    Number(f64),
    /// A raw/unparsed expression fragment.
    Raw(String),
}

/// Constraint engine that stores constraints and symbol registrations.
#[derive(Debug)]
pub struct ConstraintEngine {
    symbols: HashMap<String, String>,
    constraints: Vec<ConstraintExpr>,
}

impl ConstraintEngine {
    /// Create a new, empty constraint engine.
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    /// Register a symbol name and its type string for constraint validation.
    pub fn register_symbol(&mut self, name: &str, ty: &str) {
        self.symbols.insert(name.to_string(), ty.to_string());
    }

    /// Add a constraint expression to be validated.
    pub fn add_constraint(&mut self, c: ConstraintExpr) {
        self.constraints.push(c);
    }

    /// Validate all registered constraints against known symbols.
    pub fn validate_all(&self) -> Result<()> {
        // Minimal validation: ensure referenced identifiers exist
        for c in &self.constraints {
            if let ExprSimple::Identifier(ref n) = c.left {
                if !self.symbols.contains_key(n) {
                    return Err(anyhow!("Unknown symbol in constraint: {}", n));
                }
            }
            if let ExprSimple::Identifier(ref n) = c.right {
                if !self.symbols.contains_key(n) {
                    return Err(anyhow!("Unknown symbol in constraint: {}", n));
                }
            }
        }
        Ok(())
    }
}
