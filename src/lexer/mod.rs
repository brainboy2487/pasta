// src/lexer/mod.rs
//! Lexer module for PASTA
//!
//! This module wires together the lexer submodules:
//! - tokens: token types and Token struct
//! - alias: alias table and normalization
//! - unicode: unicode math normalization helpers
//! - lexer: the main Lexer implementation
//!
//! Public API:
//! - `Lexer` (the main lexer type)
//! - `Token` and `TokenType` for downstream consumers (parser, tests)

pub mod tokens;
pub mod alias;
pub mod unicode;
/// Core lexer implementation (see [`lexer::Lexer`]).
pub mod lexer;

pub use lexer::Lexer;
pub use self::tokens::{Token, TokenType};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::TokenType;

    #[test]
    fn simple_lex_smoke() {
        let src = r#"
DO update_ui AS ui_thread FOR 10
set x to 5 × 10^3
velocity distance LIMIT OVER time
"#;
        let tokens = Lexer::new(src).lex();
        // Basic sanity checks: ensure we produced some tokens and EOF at end
        assert!(!tokens.is_empty());
        assert_eq!(tokens.last().unwrap().kind, TokenType::Eof);
    }

    #[test]
    fn alias_before_contextual() {
        let src_non_do = "before x y\n";
        let tokens_non_do = Lexer::new(src_non_do).lex();
        // "before" outside DO line should be an identifier (no OVER mapping)
        assert!(tokens_non_do.iter().any(|t| t.kind == TokenType::Identifier));

        let src_do = "DO a BEFORE b\n";
        let tokens_do = Lexer::new(src_do).lex();
        // On a DO line, "before" should map to OVER (token type Over)
        assert!(tokens_do.iter().any(|t| t.kind == TokenType::Over));
    }
}
