// src/parser/mod.rs
//! Parser module for PASTA
//!
//! Submodules:
//!   ast     — AST node definitions
//!   grammar — EBNF grammar reference
//!   parser  — main Parser implementation
//!
//! Everything in `ast` is re-exported here so downstream consumers can do
//! `use crate::parser::*` to get all AST types.

pub mod ast;
pub mod grammar;
pub mod parser;

pub use parser::Parser;

// ── Core AST types ────────────────────────────────────────────────────────────
pub use ast::{
    Program, Statement, Expr, Identifier, Span, BinaryOp, RelationToken, ScopeModifier,
};

// ── Sub-types needed by executor / parser internals ───────────────────────────
pub use ast::{
    ModuleDecl, ModuleImportGroup, UseItem,
    FieldDecl, MutEntry, MutBody,
    Constructor, MethodDecl,
    SpawnEntry, DefDoUntil,
    RetLateCondition,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lexer::Lexer;

    #[test]
    fn parser_smoke_assignment() {
        let src = "set x = 10\n";
        let tokens = Lexer::new(src).lex();
        let mut p = Parser::new(tokens);
        let program = p.parse();
        assert!(!program.statements.is_empty());
    }

    #[test]
    fn parser_smoke_do_block() {
        let src = "DO worker FOR 3:\n    set x = 1\n";
        let tokens = Lexer::new(src).lex();
        let mut p = Parser::new(tokens);
        let program = p.parse();
        assert!(!program.statements.is_empty());
    }

    #[test]
    fn parser_accepts_if_brace_block() {
        let src = "IF true {\n    PRINT 1\n}\n";
        let tokens = Lexer::new(src).lex();
        let mut p = Parser::new(tokens);
        let program = p.parse();
        assert!(matches!(program.statements.first(), Some(Statement::If { .. })));
    }

    #[test]
    fn parser_accepts_for_brace_block() {
        let src = "FOR x IN [1, 2] {\n    PRINT x\n}\n";
        let tokens = Lexer::new(src).lex();
        let mut p = Parser::new(tokens);
        let program = p.parse();
        assert!(matches!(program.statements.first(), Some(Statement::ForIn { .. })));
    }

    #[test]
    fn parser_accepts_try_brace_block() {
        let src = "TRY {\n    PRINT 1\n} OTHERWISE {\n    PRINT 2\n} END\n";
        let tokens = Lexer::new(src).lex();
        let mut p = Parser::new(tokens);
        let program = p.parse();
        assert!(matches!(program.statements.first(), Some(Statement::TryBlock { .. })));
    }
}
