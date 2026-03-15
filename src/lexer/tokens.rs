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
    /// `DEF` — begins a function or method definition.
    Def,
    /// `DO` — begins a loop or inline block.
    Do,
    /// `AS` — alias clause: `DO x AS y`.
    As,
    /// `FOR` — repeat-count clause: `DO x FOR n`.
    For,
    /// `IN` keyword — used in FOR x IN iterable loops.
    In,
    /// `OVER` — used in `PRIORITY` and `LIMIT OVER` constraints.
    Over,
    /// `LIMIT` — begins a constraint-limit expression.
    Limit,
    /// `END` — closes a block.
    End,
    /// `PAUSE` — suspends a named DO thread.
    Pause,
    /// `UNPAUSE` — resumes a paused DO thread.
    Unpause,
    /// `RESTART` — restarts a DO thread from the beginning.
    Restart,
    /// `WAIT` — delays execution for a duration.
    Wait,
    /// `SET` / `set` / `let` / `make` — variable-assignment prefix.
    Set,
    /// `IF` — conditional branch.
    If,
    /// `OTHERWISE` / `ELSE` — alternative branch of IF or ATTEMPT.
    Otherwise,
    /// `TRY` / `ATTEMPT` — begins an error-catching block.
    Try,
    /// `GROUP` — groups related DO threads.
    Group,
    /// `CLASS` — reserved for future class declarations.
    Class,
    /// `LEARN` — reserved for ML training blocks.
    Learn,
    /// `BUILD` — reserved for build/compile steps.
    Build,
    /// `TENSOR` — tensor literal prefix.
    Tensor,
    /// `PRINT` — print statement keyword.
    Print,
    /// `AND` / `&&` — logical conjunction.
    And,
    /// `OR` / `||` — logical disjunction.
    Or,
    /// `NOT` / `!` — logical negation.
    Not,
    /// `WHILE` — loop-condition keyword.
    While,

    // ── Special keywords used by grammar (explicit tokens) ───────────────────
    /// `OBJ` keyword (explicit token to simplify parsing of `OBJ.<GROUP>.MUT`)
    Obj,
    /// `SPAWN` keyword (explicit token for SPAWN blocks)
    Spawn,

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
    /// `%` (modulo)
    Percent,
    /// `^` (exponentiation)
    Caret,
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
    /// `.`
    Dot,
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
    /// Emitted when indentation increases — signals the start of a new block.
    Indent,
    /// Emitted when indentation decreases — signals the end of a block.
    Dedent,
    /// Emitted at the end of each logical source line.
    Newline,

    // ── Sentinel ─────────────────────────────────────────────────────────────
    /// End-of-file sentinel — always the last token in the stream.
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

    // New tests for added tokens

    #[test]
    fn dot_token_display() {
        let t = Token::new(TokenType::Dot, None, 1, 1);
        let s = format!("{}", t);
        assert!(s.contains("Dot"));
    }

    #[test]
    fn obj_and_spawn_tokens_display() {
        let o = Token::new(TokenType::Obj, None, 1, 1);
        let s = format!("{}", o);
        assert!(s.contains("Obj"));

        let sp = Token::new(TokenType::Spawn, None, 2, 1);
        let s2 = format!("{}", sp);
        assert!(s2.contains("Spawn"));
    }
}
