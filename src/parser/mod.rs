// src/parser/mod.rs
//! Parser module for PASTA
//!
//! This module wires together the parser submodules:
//! - ast: AST node definitions (Program, Statement, Expr, Span, etc.)
//! - grammar: EBNF grammar reference (human-readable constant)
//! - parser: the main Parser implementation
//!
//! Public API:
//! - `Parser` (the main parser type)
//! - AST types re-exported for downstream consumers (executor, semantics, tests)

pub mod ast;
pub mod grammar;
pub mod parser;

pub use parser::Parser;
pub use ast::{
    Program, Statement, Expr, Identifier, Span, BinaryOp, RelationToken,
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
}
