// src/semantics/constraints.rs
//! Constraint engine for PASTA (minimal, debug-friendly)

use std::collections::HashMap;
use anyhow::{Result, anyhow};


/// Simple representation of a constraint expression used by the engine.
#[derive(Debug, Clone)]
pub struct ConstraintExpr {
    pub left: ExprSimple,
    pub relation: Option<Relation>,
    pub right: ExprSimple,
    pub constraint: ExprSimple,
}

impl ConstraintExpr {
    pub fn new(left: ExprSimple, relation: Option<Relation>, right: ExprSimple, constraint: ExprSimple) -> Self {
        Self { left, relation, right, constraint }
    }
}

/// Minimal relation enum used by constraints.
#[derive(Debug, Clone, Copy)]
pub enum Relation {
    Equals,
    LessThan,
    GreaterThan,
}

impl Relation {
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
    Identifier(String),
    Number(f64),
    Raw(String),
}

/// Constraint engine that stores constraints and symbol registrations.
#[derive(Debug)]
pub struct ConstraintEngine {
    symbols: HashMap<String, String>,
    constraints: Vec<ConstraintExpr>,
}

impl ConstraintEngine {
    pub fn new() -> Self {
        Self {
            symbols: HashMap::new(),
            constraints: Vec::new(),
        }
    }

    pub fn register_symbol(&mut self, name: &str, ty: &str) {
        self.symbols.insert(name.to_string(), ty.to_string());
    }

    pub fn add_constraint(&mut self, c: ConstraintExpr) {
        self.constraints.push(c);
    }

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
