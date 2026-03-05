// src/tokenizer.rs
//! Tokenizer for the PASTA language
//!
//! This module provides a standalone, well-tested tokenizer (lexer) that
//! converts source text into a stream of `Token`s with `Span` information.
//! It is intentionally small and conservative: ASCII-focused, supports numbers,
//! identifiers, string literals with escapes, comments, and common punctuation
//! and operators used by the parser and runtime.
//!
//! The tokenizer is independent of the parser implementation but produces a
//! compact token set suitable for the rest of the codebase.

use std::fmt;

/// Byte offset span in the source text (start inclusive, end exclusive).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    pub fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }
}

/// Token kinds produced by the tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Structural
    Eof,
    Newline,
    Indent,
    Dedent,

    // Literals
    Identifier(String),
    Number(f64),
    String(String),

    // Keywords
    KwSet,
    KwIf,
    KwElse,
    KwWhile,
    KwDo,
    KwEnd,
    KwFn,
    KwReturn,
    KwTrue,
    KwFalse,
    KwLet,

    // Operators and punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    And,
    Or,
    Not,
    Assign,      // =
    Eq,          // ==
    Neq,         // !=
    Lt,          // <
    Gt,          // >
    Le,          // <=
    Ge,          // >=
    LParen,
    RParen,
    LBrace,
    RBrace,
    Comma,
    Colon,
    Semicolon,
    Arrow,       // ->
    Dot,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, start: usize, end: usize) -> Self {
        Self {
            kind,
            span: Span::new(start, end),
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use TokenKind::*;
        match self {
            Eof => write!(f, "<EOF>"),
            Newline => write!(f, "<NEWLINE>"),
            Indent => write!(f, "<INDENT>"),
            Dedent => write!(f, "<DEDENT>"),
            Identifier(s) => write!(f, "Ident({})", s),
            Number(n) => write!(f, "Number({})", n),
            String(s) => write!(f, "String({})", s),
            KwSet => write!(f, "set"),
            KwIf => write!(f, "if"),
            KwElse => write!(f, "else"),
            KwWhile => write!(f, "while"),
            KwDo => write!(f, "do"),
            KwEnd => write!(f, "end"),
            KwFn => write!(f, "fn"),
            KwReturn => write!(f, "return"),
            KwTrue => write!(f, "true"),
            KwFalse => write!(f, "false"),
            KwLet => write!(f, "let"),
            Plus => write!(f, "+"),
            Minus => write!(f, "-"),
            Star => write!(f, "*"),
            Slash => write!(f, "/"),
            Percent => write!(f, "%"),
            Caret => write!(f, "^"),
            And => write!(f, "&&"),
            Or => write!(f, "||"),
            Not => write!(f, "!"),
            Assign => write!(f, "="),
            Eq => write!(f, "=="),
            Neq => write!(f, "!="),
            Lt => write!(f, "<"),
            Gt => write!(f, ">"),
            Le => write!(f, "<="),
            Ge => write!(f, ">="),
            LParen => write!(f, "("),
            RParen => write!(f, ")"),
            LBrace => write!(f, "{{"),
            RBrace => write!(f, "}}"),
            Comma => write!(f, ","),
            Colon => write!(f, ":"),
            Semicolon => write!(f, ";"),
            Arrow => write!(f, "->"),
            Dot => write!(f, "."),
        }
    }
}

/// Tokenizer (lexer) implementation.
pub struct Tokenizer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    len: usize,
}

impl<'a> Tokenizer<'a> {
    /// Create a new tokenizer for the given source text.
    pub fn new(src: &'a str) -> Self {
        let bytes = src.as_bytes();
        let len = bytes.len();
        Self {
            src,
            bytes,
            pos: 0,
            len,
        }
    }

    fn peek_byte(&self) -> Option<u8> {
        if self.pos < self.len {
            Some(self.bytes[self.pos])
        } else {
            None
        }
    }

    fn peek_byte_at(&self, offset: usize) -> Option<u8> {
        let p = self.pos + offset;
        if p < self.len {
            Some(self.bytes[p])
        } else {
            None
        }
    }

    fn bump(&mut self) -> Option<u8> {
        if self.pos < self.len {
            let b = self.bytes[self.pos];
            self.pos += 1;
            Some(b)
        } else {
            None
        }
    }

    fn eat_while<F>(&mut self, mut f: F) -> usize
    where
        F: FnMut(u8) -> bool,
    {
        let start = self.pos;
        while let Some(&b) = self.bytes.get(self.pos) {
            if f(b) {
                self.pos += 1;
            } else {
                break;
            }
        }
        self.pos - start
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            let mut progressed = false;
            // Skip spaces and tabs
            while let Some(b) = self.peek_byte() {
                if b == b' ' || b == b'\t' || b == b'\r' {
                    self.pos += 1;
                    progressed = true;
                } else {
                    break;
                }
            }
            // Skip line comments starting with '#' or '//'
            if let Some(b'/') = self.peek_byte() {
                if let Some(b'/') = self.peek_byte_at(1) {
                    // consume until newline or EOF
                    self.pos += 2;
                    while let Some(b) = self.peek_byte() {
                        self.pos += 1;
                        if b == b'\n' {
                            break;
                        }
                    }
                    progressed = true;
                    continue;
                }
            }
            if let Some(b'#') = self.peek_byte() {
                // consume until newline or EOF
                self.pos += 1;
                while let Some(b) = self.peek_byte() {
                    self.pos += 1;
                    if b == b'\n' {
                        break;
                    }
                }
                progressed = true;
                continue;
            }
            if !progressed {
                break;
            }
        }
    }

    /// Tokenize the entire input into a Vec<Token>.
    pub fn tokenize(mut self) -> Vec<Token> {
        let mut toks = Vec::new();
        loop {
            self.skip_whitespace_and_comments();
            let start = self.pos;
            let tok = match self.peek_byte() {
                None => {
                    toks.push(Token::new(TokenKind::Eof, self.pos, self.pos));
                    break;
                }
                Some(b'\n') => {
                    self.pos += 1;
                    Token::new(TokenKind::Newline, start, self.pos)
                }
                Some(b'"') | Some(b'\'') => {
                    // string literal
                    let quote = self.bump().unwrap();
                    let mut s = String::new();
                    while let Some(b) = self.bump() {
                        if b == quote {
                            break;
                        }
                        if b == b'\\' {
                            // escape
                            if let Some(esc) = self.bump() {
                                match esc {
                                    b'n' => s.push('\n'),
                                    b'r' => s.push('\r'),
                                    b't' => s.push('\t'),
                                    b'\\' => s.push('\\'),
                                    b'\'' => s.push('\''),
                                    b'"' => s.push('"'),
                                    b'0' => s.push('\0'),
                                    other => s.push(other as char),
                                }
                            } else {
                                // unterminated escape; treat as literal backslash
                                s.push('\\');
                            }
                        } else {
                            s.push(b as char);
                        }
                    }
                    Token::new(TokenKind::String(s), start, self.pos)
                }
                Some(b) if is_digit(b) || (b == b'.' && self.peek_byte_at(1).map_or(false, |c| is_digit(c))) => {
                    // number: integer or float
                    let mut seen_dot = false;
                    if self.peek_byte() == Some(b'.') {
                        seen_dot = true;
                        self.pos += 1;
                    }
                    while let Some(b) = self.peek_byte() {
                        if is_digit(b) {
                            self.pos += 1;
                        } else if b == b'.' && !seen_dot {
                            seen_dot = true;
                            self.pos += 1;
                        } else {
                            break;
                        }
                    }
                    // optional exponent
                    if let Some(b'e') | Some(b'E') = self.peek_byte() {
                        self.pos += 1;
                        if let Some(b'+') | Some(b'-') = self.peek_byte() {
                            self.pos += 1;
                        }
                        self.eat_while(|c| is_digit(c));
                    }
                    let slice = &self.src[start..self.pos];
                    let val = slice.parse::<f64>().unwrap_or(f64::NAN);
                    Token::new(TokenKind::Number(val), start, self.pos)
                }
                Some(b) if is_ident_start(b) => {
                    // identifier or keyword
                    self.pos += 1;
                    self.eat_while(is_ident_continue);
                    let s = &self.src[start..self.pos];
                    let kind = match s {
                        "set" => TokenKind::KwSet,
                        "if" => TokenKind::KwIf,
                        "else" => TokenKind::KwElse,
                        "while" => TokenKind::KwWhile,
                        "do" => TokenKind::KwDo,
                        "end" => TokenKind::KwEnd,
                        "fn" => TokenKind::KwFn,
                        "return" => TokenKind::KwReturn,
                        "true" => TokenKind::KwTrue,
                        "false" => TokenKind::KwFalse,
                        "let" => TokenKind::KwLet,
                        other => TokenKind::Identifier(other.to_string()),
                    };
                    Token::new(kind, start, self.pos)
                }
                Some(b) => {
                    // Operators and punctuation (handle multi-char first)
                    match b {
                        b'=' => {
                            if self.peek_byte_at(1) == Some(b'=') {
                                self.pos += 2;
                                Token::new(TokenKind::Eq, start, self.pos)
                            } else {
                                self.pos += 1;
                                Token::new(TokenKind::Assign, start, self.pos)
                            }
                        }
                        b'!' => {
                            if self.peek_byte_at(1) == Some(b'=') {
                                self.pos += 2;
                                Token::new(TokenKind::Neq, start, self.pos)
                            } else {
                                self.pos += 1;
                                Token::new(TokenKind::Not, start, self.pos)
                            }
                        }
                        b'<' => {
                            if self.peek_byte_at(1) == Some(b'=') {
                                self.pos += 2;
                                Token::new(TokenKind::Le, start, self.pos)
                            } else {
                                self.pos += 1;
                                Token::new(TokenKind::Lt, start, self.pos)
                            }
                        }
                        b'>' => {
                            if self.peek_byte_at(1) == Some(b'=') {
                                self.pos += 2;
                                Token::new(TokenKind::Ge, start, self.pos)
                            } else {
                                self.pos += 1;
                                Token::new(TokenKind::Gt, start, self.pos)
                            }
                        }
                        b'&' => {
                            if self.peek_byte_at(1) == Some(b'&') {
                                self.pos += 2;
                                Token::new(TokenKind::And, start, self.pos)
                            } else {
                                self.pos += 1;
                                Token::new(TokenKind::And, start, self.pos)
                            }
                        }
                        b'|' => {
                            if self.peek_byte_at(1) == Some(b'|') {
                                self.pos += 2;
                                Token::new(TokenKind::Or, start, self.pos)
                            } else {
                                self.pos += 1;
                                Token::new(TokenKind::Or, start, self.pos)
                            }
                        }
