// src/lexer/tokens.rs
//! Token types and Token struct for the PASTA lexer.
//!
//! `TokenType` is an enum of every token kind the lexer can produce.
//! `Token` carries the kind, an optional string value (for identifiers,
//! numbers, strings, and booleans), and source-location info.

/// All token kinds produced by the PASTA lexer.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TokenType {
    // ── Literals ────────────────────────────────────────────────────────────
    /// Integer or floating-point literal. Value held in `Token::value`.
    Number,
    /// Double-quoted string literal. Value held in `Token::value` (unescaped).
    String,
    /// Boolean literal (`true` / `false`). Value held in `Token::value`.
    Bool,

    // ── Identifiers ─────────────────────────────────────────────────────────
    /// Any identifier not matched as a keyword or alias.
    Identifier,

    // ── Keywords / aliases ───────────────────────────────────────────────────
    Def,
    Do,
    As,
    For,
    Over,
    Limit,
    End,
    Pause,
    Unpause,
    Restart,
    Wait,
    Set,
    If,
    Otherwise,
    Try,
    Group,
    Class,
    Learn,
    Build,
    Tensor,
    Print,
    And,
    Or,
    Not,
    While,

    // ── Operators ────────────────────────────────────────────────────────────
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `@` (matrix multiply)
    At,
    /// `=`  (assignment)
    Eq,
    /// `==` (equality comparison)
    EqEq,
    /// `!=`
    Neq,
    /// `<`
    Lt,
    /// `>`
    Gt,
    /// `<=`
    Lte,
    /// `>=`
    Gte,
    /// `≈` (approximate equality)
    Approx,
    /// `≠` (not equal — unicode, distinct from ASCII !=)
    NotEq,
    /// `≡` (strict identity)
    StrictEq,

    // ── Punctuation ──────────────────────────────────────────────────────────
    /// `,`
    Comma,
    /// `:`
    Colon,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `[`
    LBracket,
    /// `]`
    RBracket,

    // ── Layout tokens (produced by indentation logic) ────────────────────────
    Indent,
    Dedent,
    Newline,

    // ── Sentinel ─────────────────────────────────────────────────────────────
    Eof,
}

impl std::fmt::Display for TokenType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to Debug — gives names like `While`, `Do`, `Eof` etc.
        write!(f, "{:?}", self)
    }
}

/// A single lexed token with source-location metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    /// The token kind.
    pub kind: TokenType,
    /// Optional string payload (identifier name, number text, string content,
    /// or `"true"` / `"false"` for booleans). `None` for punctuation / keywords.
    pub value: Option<String>,
    /// 1-based source line.
    pub line: usize,
    /// 1-based column within the line.
    pub col: usize,
}

impl Token {
    /// Construct a new token.
    pub fn new(kind: TokenType, value: Option<String>, line: usize, col: usize) -> Self {
        Self { kind, value, line, col }
    }

    /// Return `true` if this token is `Eof`.
    pub fn is_eof(&self) -> bool {
        self.kind == TokenType::Eof
    }
}

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.value {
            Some(v) => write!(f, "{:?}({})", self.kind, v),
            None    => write!(f, "{:?}", self.kind),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_is_eof() {
        let t = Token::new(TokenType::Eof, None, 1, 1);
        assert!(t.is_eof());
    }

    #[test]
    fn token_display_with_value() {
        let t = Token::new(TokenType::Identifier, Some("foo".into()), 1, 1);
        let s = format!("{}", t);
        assert!(s.contains("foo"));
    }

    #[test]
    fn token_display_no_value() {
        let t = Token::new(TokenType::Plus, None, 1, 1);
        let s = format!("{}", t);
        assert!(s.contains("Plus"));
    }

    #[test]
    fn at_token_display() {
        let t = Token::new(TokenType::At, None, 1, 1);
        let s = format!("{}", t);
        assert!(s.contains("At"));
    }

    #[test]
    fn approx_token_display() {
        let t = Token::new(TokenType::Approx, None, 1, 1);
        let s = format!("{}", t);
        assert!(s.contains("Approx"));
    }
}
