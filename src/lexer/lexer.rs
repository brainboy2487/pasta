// src/lexer/lexer.rs
//
// Core lexer for PASTA (Rust).
//
// Changes from previous version:
// - Fixed decimal number literals (e.g. `1.57079632679`) being split into
//   three tokens (Number, Identifier("."), Number) by absorbing a single '.'
//   into the token buffer when it appears mid-number and is followed by a digit.
// - Added leading-dot numeric literals (e.g. `.5`) so they parse as Number(".5").
// - Hardened indentation: tabs are normalised to 4 spaces (not rejected).
// - Emits Bool tokens for true/false.
// - Emits `Comma` tokens so parser can parse `DO a, b` target lists.
// - Emits `For` and `Do` tokens and preserves contextual `is_do_line` for
//   alias normalisation.
// - Keeps token positions (line/col) 1-based; emits Newline at end of each
//   logical line.
// - Leaves combination of LIMIT + OVER to the parser (lexer emits separately).
// - Emits Approx (≈), NotEq (≠), and StrictEq (≡) tokens for unicode operators.
// - Emits At (@) token for matrix multiply operator.
// - Emits Dot token for '.' separators and Obj/Spawn tokens via alias normalization.

use crate::lexer::{Token, TokenType};
use crate::lexer::alias::AliasTable;
use crate::lexer::unicode::normalize_unicode;
use std::fmt;
use std::mem;

// ─────────────────────────────────────────────────────────────────────────────
// LexError
// ─────────────────────────────────────────────────────────────────────────────

/// Structured lexical error returned when lexing fails.
#[derive(Debug, Clone)]
pub struct LexError {
    /// 1-based source line where the error occurred.
    pub line: usize,
    /// 1-based column within the line.
    pub col: usize,
    /// Human-readable description of the lex failure.
    pub message: String,
}

impl LexError {
    /// Construct a new `LexError` at the given source position.
    pub fn new(line: usize, col: usize, message: impl Into<String>) -> Self {
        Self {
            line,
            col,
            message: message.into(),
        }
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Lex error at {}:{}: {}", self.line, self.col, self.message)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Lexer
// ─────────────────────────────────────────────────────────────────────────────

/// Converts PASTA source text into a flat [`Token`] stream.
///
/// Create with [`Lexer::new`], then call [`Lexer::lex`] or [`Lexer::lex_result`].
pub struct Lexer {
    lines: Vec<String>,
    line_num: usize,
    col: usize,
    indent_stack: Vec<usize>,
    tokens: Vec<Token>,
    aliases: AliasTable,
}

impl Lexer {
    /// Create a new lexer from a source string.
    pub fn new(source: &str) -> Self {
        Self {
            lines: source.lines().map(|s| s.to_string()).collect(),
            line_num: 0,
            col: 0,
            indent_stack: vec![0],
            tokens: Vec::new(),
            aliases: AliasTable::new(),
        }
    }

    /// Emit a token into the internal token buffer.
    fn emit(&mut self, kind: TokenType, value: Option<String>) {
        self.tokens.push(Token::new(kind, value, self.line_num, self.col));
    }

    /// Run the lexer and return the token stream.
    ///
    /// This convenience wrapper panics on lex errors. Prefer `lex_result()`
    /// for structured error handling.
    pub fn lex(self) -> Vec<Token> {
        self.lex_result().unwrap()
    }

    /// Run the lexer, returning a structured `LexError` on failure.
    pub fn lex_result(mut self) -> Result<Vec<Token>, LexError> {
        let lines = mem::take(&mut self.lines);

        for (i, raw_line) in lines.iter().enumerate() {
            self.line_num = i + 1;
            self.lex_line_result(raw_line)?;
        }

        // Close any remaining open indentation levels.
        while self.indent_stack.len() > 1 {
            self.indent_stack.pop();
            self.emit(TokenType::Dedent, None);
        }

        self.emit(TokenType::Eof, None);
        Ok(self.tokens)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Line lexer
    // ─────────────────────────────────────────────────────────────────────────

    /// Lex a single source line.
    fn lex_line_result(&mut self, raw: &str) -> Result<(), LexError> {
        self.col = 0;

        // Normalise unicode math symbols and superscripts for token content.
        // Use the raw line for indentation so we preserve exact leading bytes.
        let normalized = normalize_unicode(raw);

        // ── Indentation ──────────────────────────────────────────────────────
        // Count leading spaces; treat each tab as 4 spaces.
        let mut leading_spaces = 0usize;
        for ch in raw.chars() {
            match ch {
                ' ' => leading_spaces += 1,
                '\t' => leading_spaces += 4,
                _ => break,
            }
        }

        let indent = leading_spaces;
        let last_indent = *self.indent_stack.last().unwrap_or(&0);

        if indent > last_indent {
            self.indent_stack.push(indent);
            self.emit(TokenType::Indent, None);
        } else if indent < last_indent {
            while indent < *self.indent_stack.last().unwrap_or(&0) {
                if self.indent_stack.len() > 1 {
                    self.indent_stack.pop();
                    self.emit(TokenType::Dedent, None);
                } else {
                    break;
                }
            }
        }

        // ── DO-line context (for alias normalisation) ────────────────────────
        let is_do_line = raw.trim_start().to_lowercase().starts_with("do ");

        // ── Strip leading whitespace and comments ────────────────────────────
        let mut s = normalized.trim_start().to_string();

        // Remove line comments: '#' or '//'
        if let Some(idx) = s.find('#') {
            s.truncate(idx);
        } else if let Some(idx) = s.find("//") {
            s.truncate(idx);
        }

        if s.trim().is_empty() {
            self.emit(TokenType::Newline, None);
            return Ok(());
        }

        // ── Character-by-character scan ──────────────────────────────────────
        let mut chars = s.chars().peekable();

        while let Some(&ch) = chars.peek() {
            // Column is 1-based; increment at the start of each token.
            self.col += 1;

            // ── Whitespace ───────────────────────────────────────────────────
            if ch.is_whitespace() {
                chars.next();
                continue;
            }

            // ── String literals ──────────────────────────────────────────────
            if ch == '"' {
                chars.next();
                let mut buf = String::new();
                let mut escaped = false;
                for c in chars.by_ref() {
                    self.col += 1;
                    if escaped {
                        match c {
                            'n' => buf.push('\n'),
                            'r' => buf.push('\r'),
                            't' => buf.push('\t'),
                            '\\' => buf.push('\\'),
                            '"' => buf.push('"'),
                            other => buf.push(other),
                        }
                        escaped = false;
                    } else if c == '\\' {
                        escaped = true;
                    } else if c == '"' {
                        break;
                    } else {
                        buf.push(c);
                    }
                }
                self.emit(TokenType::String, Some(buf));
                continue;
            }

            // ── Unicode math operators (≈ ≠ ≡) ───────────────────────────────
            // These must be checked BEFORE the ASCII operator block because
            // they are multi-byte chars that won't match any ASCII branch.
            if ch == '\u{2248}' { // ≈  ALMOST EQUAL TO
                chars.next();
                self.emit(TokenType::Approx, None);
                continue;
            }

            if ch == '\u{2260}' { // ≠  NOT EQUAL TO
                chars.next();
                self.emit(TokenType::NotEq, None);
                continue;
            }

            if ch == '\u{2261}' { // ≡  IDENTICAL TO
                chars.next();
                self.emit(TokenType::StrictEq, None);
                continue;
            }

            // ── Multi-character operators ────────────────────────────────────
            if ch == '=' {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    self.col += 1;
                    self.emit(TokenType::EqEq, None);
                } else {
                    self.emit(TokenType::Eq, None);
                }
                continue;
            }

            if ch == '!' {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    self.col += 1;
                    self.emit(TokenType::Neq, None);
                } else {
                    // '!' alone is not a PASTA operator; emit as identifier.
                    self.emit(TokenType::Identifier, Some("!".to_string()));
                }
                continue;
            }

            if ch == '<' {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    self.col += 1;
                    self.emit(TokenType::Lte, None);
                } else {
                    self.emit(TokenType::Lt, None);
                }
                continue;
            }

            if ch == '>' {
                chars.next();
                if chars.peek() == Some(&'=') {
                    chars.next();
                    self.col += 1;
                    self.emit(TokenType::Gte, None);
                } else {
                    self.emit(TokenType::Gt, None);
                }
                continue;
            }

            if ch == '&' {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    self.col += 1;
                    self.emit(TokenType::And, None);
                } else {
                    // bare '&' — emit as identifier
                    self.emit(TokenType::Identifier, Some("&".to_string()));
                }
                continue;
            }

            if ch == '|' {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    self.col += 1;
                    self.emit(TokenType::Or, None);
                } else {
                    self.emit(TokenType::Identifier, Some("|".to_string()));
                }
                continue;
            }

            // Range operator '..' — detect before leading-dot numeric logic
            if ch == '.' {
                let mut tmp = chars.clone();
                tmp.next();
                if tmp.peek() == Some(&'.') {
                    chars.next();
                    chars.next();
                    self.col += 2;
                    self.emit(TokenType::DotDot, None);
                    continue;
                }
            }

            // ── Leading-dot numeric literal (.5, .25, etc.) ──────────────────
            // A '.' followed immediately by an ASCII digit starts a float literal.
            if ch == '.' {
                // Look one character ahead (after the dot) to decide.
                let mut tmp = chars.clone();
                tmp.next(); // consume '.'
                let next_is_digit = tmp.next().is_some_and(|c| c.is_ascii_digit());

                if next_is_digit {
                    // consume '.' and the following digits/underscores
                    chars.next(); // consume '.'
                    let mut buf = String::from(".");
                    while let Some(&c2) = chars.peek() {
                        // accept digits and underscores in a leading-dot literal
                        if c2.is_ascii_digit() || c2 == '_' {
                            buf.push(c2);
                            chars.next();
                            self.col += 1;
                        } else {
                            break;
                        }
                    }
                    self.emit(TokenType::Number, Some(buf));
                    continue;
                } else {
                    // Not a leading-dot number: emit Dot token and continue.
                    chars.next(); // consume '.'
                    self.emit(TokenType::Dot, None);
                    continue;
                }
            }

            // ── Single-character punctuation ─────────────────────────────────
            match ch {
                ',' => { chars.next(); self.emit(TokenType::Comma,    None); continue; }
                ':' => {
                    chars.next();
                    if chars.peek() == Some(&':') {
                        chars.next(); self.emit(TokenType::ColonColon, None);
                    } else {
                        self.emit(TokenType::Colon, None);
                    }
                    continue;
                }
                '(' => { chars.next(); self.emit(TokenType::LParen,   None); continue; }
                ')' => { chars.next(); self.emit(TokenType::RParen,   None); continue; }
                '[' => { chars.next(); self.emit(TokenType::LBracket, None); continue; }
                ']' => { chars.next(); self.emit(TokenType::RBracket, None); continue; }
                '{' => { chars.next(); self.emit(TokenType::LBrace,   None); continue; }
                '}' => { chars.next(); self.emit(TokenType::RBrace,   None); continue; }
                ';' => { chars.next(); self.emit(TokenType::Semicolon,None); continue; }
                '~' => { chars.next(); self.emit(TokenType::Tilde,    None); continue; }
                '?' => { chars.next(); self.emit(TokenType::Question,  None); continue; }
                '+' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next(); self.emit(TokenType::PlusEq,  None);
                    } else {
                        self.emit(TokenType::Plus, None);
                    }
                    continue;
                }
                '-' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next(); self.emit(TokenType::MinusEq, None);
                    } else if chars.peek() == Some(&'>') {
                        chars.next(); self.emit(TokenType::Arrow,   None);
                    } else {
                        self.emit(TokenType::Minus, None);
                    }
                    continue;
                }
                '*' => {
                    chars.next();
                    if chars.peek() == Some(&'*') {
                        chars.next(); self.emit(TokenType::StarStar, None);
                    } else if chars.peek() == Some(&'=') {
                        chars.next(); self.emit(TokenType::StarEq,   None);
                    } else {
                        self.emit(TokenType::Star, None);
                    }
                    continue;
                }
                '/' => {
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        chars.next(); self.emit(TokenType::FloorDiv, None);
                    } else if chars.peek() == Some(&'=') {
                        chars.next(); self.emit(TokenType::SlashEq,  None);
                    } else {
                        self.emit(TokenType::Slash, None);
                    }
                    continue;
                }
                '%' => {
                    chars.next();
                    if chars.peek() == Some(&'=') {
                        chars.next(); self.emit(TokenType::PercentEq, None);
                    } else {
                        self.emit(TokenType::Percent, None);
                    }
                    continue;
                }
                '=' => {
                    chars.next();
                    if chars.peek() == Some(&'>') {
                        chars.next(); self.emit(TokenType::FatArrow, None);
                    } else {
                        // fall through — '=' and '==' handled above this block
                        self.emit(TokenType::Eq, None);
                    }
                    continue;
                }
                '|' => {
                    chars.next();
                    if chars.peek() == Some(&'>') {
                        chars.next(); self.emit(TokenType::PipeArrow, None);
                    } else {
                        // bare '|' — bitwise or (|| handled above)
                        self.emit(TokenType::Pipe, None);
                    }
                    continue;
                }
                '&' => {
                    chars.next();
                    // bare '&' — bitwise and (&& handled above)
                    self.emit(TokenType::Ampersand, None);
                    continue;
                }
                '@' => { chars.next(); self.emit(TokenType::At,       None); continue; }
                '^' => { chars.next(); self.emit(TokenType::Caret,    None); continue; }
                '\\' => { chars.next(); self.emit(TokenType::Backslash,None); continue; }
                _ => {}
            }

            // ── Identifiers, keywords, and numeric literals ──────────────────
            if ch.is_alphanumeric() || ch == '_' {
                let mut buf = String::new();

                while let Some(&c2) = chars.peek() {
                    // allow dot as part of identifier names when the buffer does
                    // not begin with a digit (i.e. "tensor.zeros").  numeric
                    // literals are handled by the branch below which only
                    // accepts a "." when the buffer started with a digit and
                    // the dot is followed by another digit.
                    if c2.is_alphanumeric() || c2 == '_'
                        || (c2 == '.' && !buf.chars().next().is_some_and(|c| c.is_ascii_digit()))
                    {
                        buf.push(c2);
                        chars.next();
                        self.col += 1;
                    } else if c2 == '.'
                        && !buf.contains('.')
                        && buf.chars().next().is_some_and(|c| c.is_ascii_digit())
                    {
                        let next_after_dot = {
                            let mut tmp = chars.clone();
                            tmp.next(); // skip '.'
                            tmp.next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                        };

                        if next_after_dot {
                            buf.push(c2); // push the '.'
                            chars.next();
                            self.col += 1;
                        } else {
                            break;
                        }
                    } else {
                        break;
                    }
                }

                // ── Emit as Number if the buffer is a valid float/int ─────────
                // Allow underscores in numeric literals for readability (e.g. 1_000).
                // The lexer still emits the original text; the parser will strip
                // underscores when converting to f64.
                // strip underscores and attempt to parse; but only treat the
                // buffer as a numeric literal if it begins with a digit or a
                // leading dot (e.g. `.5`).  this prevents stray `_0` segments
                // from being mis-classified when an underscore is the first
                // character after a space or operator.
                let mut cleaned = String::new();
                for ch in buf.chars() {
                    if ch != '_' { cleaned.push(ch); }
                }
                let first = buf.chars().next();
                if let Some(c0) = first {
                    // basic decimal/float check
                    if (c0.is_ascii_digit() || c0 == '.') && cleaned.parse::<f64>().is_ok() {
                        self.emit(TokenType::Number, Some(buf));
                        continue;
                    }

                    // hexadecimal, binary, octal integer literals
                    // (underscores already removed in `cleaned`).
                    let lower = cleaned.to_ascii_lowercase();
                    let is_prefixed = if lower.starts_with("0x") {
                        lower.chars().skip(2).all(|c| c.is_ascii_hexdigit())
                    } else if lower.starts_with("0b") {
                        lower.chars().skip(2).all(|c| c == '0' || c == '1')
                    } else if lower.starts_with("0o") {
                        lower.chars().skip(2).all(|c| ('0'..='7').contains(&c))
                    } else {
                        false
                    };
                    if is_prefixed {
                        self.emit(TokenType::Number, Some(buf));
                        continue;
                    }
                }

                // ── Keyword / alias check ─────────────────────────────────────
                if let Some(canonical) = self.aliases.normalize(&buf, is_do_line) {
                    let token_type = match canonical.as_str() {
                        "TRUE"      => TokenType::Bool,
                        "FALSE"     => TokenType::Bool,
                        "DEF"       => TokenType::Def,
                        "DO"        => TokenType::Do,
                        "AND"       => TokenType::And,
                        "OR"        => TokenType::Or,
                        "NOT"       => TokenType::Not,
                        "FOR"       => TokenType::For,
                        "IN"        => TokenType::In,
                            "STEP"      => TokenType::Step,
                        "AS"        => TokenType::As,
                        "OVER"      => TokenType::Over,
                        "LIMIT"     => TokenType::Limit,
                        "END"       => TokenType::End,
                        "PAUSE"     => TokenType::Pause,
                        "UNPAUSE"   => TokenType::Unpause,
                        "RESTART"   => TokenType::Restart,
                        "WAIT"      => TokenType::Wait,
                        "SET"       => TokenType::Set,
                        "IF"        => TokenType::If,
                        "TRY"       => TokenType::Try,
                        "OTHERWISE" => TokenType::Otherwise,
                        "GROUP"     => TokenType::Group,
                        "CLASS"     => TokenType::Class,
                        "LEARN"     => TokenType::Learn,
                        "BUILD"     => TokenType::Build,
                        "TENSOR"    => TokenType::Tensor,
                        "PRINT"     => TokenType::Print,
                        "WHILE"     => TokenType::While,
                        // explicit grammar keywords we want as tokens
                        "OBJ"       => TokenType::Obj,
                        "SPAWN"     => TokenType::Spawn,
                        "UNLESS"    => TokenType::Unless,
                        "UNTIL"     => TokenType::Until,
                        "PASS"      => TokenType::Pass,
                        "ASSERT"    => TokenType::Assert,
                        "TYPEOF"    => TokenType::Typeof,
                        "YIELD"     => TokenType::Yield,
                        "RETURN"    => TokenType::Return,
                        "MATCH"     => TokenType::Match,
                        "WHEN"      => TokenType::When,
                        "WITH"      => TokenType::With,
                        "FROM"      => TokenType::From,
                        "CONST"     => TokenType::Const,
                        "EXPORT"    => TokenType::Export,
                        "AWAIT"     => TokenType::Await,
                        "DRAW"      => TokenType::Draw,
                        "COLOR"     => TokenType::Color,
                        "FRAME"     => TokenType::Frame,
                        "STEP"      => TokenType::Step,

                        _           => TokenType::Identifier,
                    };

                    if token_type == TokenType::Bool {
                        // Preserve the original casing for bool values.
                        self.emit(TokenType::Bool, Some(buf));
                    } else {
                        // For Obj/Spawn we emit the explicit token without value.
                        if token_type == TokenType::Obj || token_type == TokenType::Spawn {
                            self.emit(token_type, None);
                        } else {
                            self.emit(token_type, Some(buf));
                        }
                    }
                    continue;
                }

                // ── Plain identifier ──────────────────────────────────────────
                self.emit(TokenType::Identifier, Some(buf));
                continue;
            }

            // ── Fallback: emit single unknown character as an identifier ──────
            let ch_str = ch.to_string();
            chars.next();
            self.emit(TokenType::Identifier, Some(ch_str));
        }

        self.emit(TokenType::Newline, None);
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::TokenType;

    fn lex(src: &str) -> Vec<Token> {
        Lexer::new(src).lex()
    }

    fn kinds(src: &str) -> Vec<TokenType> {
        lex(src).into_iter().map(|t| t.kind).collect()
    }

    fn values(src: &str) -> Vec<Option<String>> {
        lex(src).into_iter().map(|t| t.value).collect()
    }

    // ── Decimal / float literals ─────────────────────────────────────────────

    #[test]
    fn decimal_literal_is_single_token() {
        let toks = lex("1.57079632679");
        let numbers: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Number).collect();
        assert_eq!(numbers.len(), 1, "expected exactly one Number token for 1.57079632679");
        assert_eq!(numbers[0].value.as_deref(), Some("1.57079632679"));
    }

    #[test]
    fn leading_dot_float() {
        let toks = lex(".5");
        let numbers: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Number).collect();
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].value.as_deref(), Some(".5"));
    }

    #[test]
    fn integer_literal() {
        let toks = lex("42");
        let numbers: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Number).collect();
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].value.as_deref(), Some("42"));
    }

    #[test]
    fn underscore_numeric_literal() {
        let toks = lex("1_000 3.14_15 .5_0");
        // debug output for investigation
        eprintln!("tokens: {:?}", toks);
        let numbers: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Number).collect();
        assert_eq!(numbers.len(), 3);
        assert_eq!(numbers[0].value.as_deref(), Some("1_000"));
        assert_eq!(numbers[1].value.as_deref(), Some("3.14_15"));
        assert_eq!(numbers[2].value.as_deref(), Some(".5_0"));
    }

    #[test]
    fn no_second_dot_in_number() {
        let toks = lex("1.2.3");
        let numbers: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Number).collect();
        assert!(numbers.len() >= 2);
        assert_eq!(numbers[0].value.as_deref(), Some("1.2"));
    }

    #[test]
    fn hex_bin_octal_literals() {
        let toks = lex("0xFF 0xFF_00 0b1010_0011 0o755");
        let nums: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Number).collect();
        assert_eq!(nums.len(), 4);
        assert_eq!(nums[0].value.as_deref(), Some("0xFF"));
        assert_eq!(nums[1].value.as_deref(), Some("0xFF_00"));
        assert_eq!(nums[2].value.as_deref(), Some("0b1010_0011"));
        assert_eq!(nums[3].value.as_deref(), Some("0o755"));
    }

    #[test]
    fn builtin_call_with_float_arg() {
        let src = "v = __pasta_math_sin(1.57079632679)";
        let toks = lex(src);
        let dot_idents: Vec<_> = toks
            .iter()
            .filter(|t| t.kind == TokenType::Identifier && t.value.as_deref() == Some("."))
            .collect();
        assert!(dot_idents.is_empty(), "stray '.' identifier found in token stream");
    }

    // ── Operators ────────────────────────────────────────────────────────────

    #[test]
    fn eq_vs_eqeq() {
        let k = kinds("a = b == c");
        assert!(k.contains(&TokenType::Eq));
        assert!(k.contains(&TokenType::EqEq));
    }

    #[test]
    fn comparison_operators() {
        let k = kinds("a != b <= c >= d < e > f");
        assert!(k.contains(&TokenType::Neq));
        assert!(k.contains(&TokenType::Lte));
        assert!(k.contains(&TokenType::Gte));
        assert!(k.contains(&TokenType::Lt));
        assert!(k.contains(&TokenType::Gt));
    }

    #[test]
    fn unicode_approx_operator() {
        let k = kinds("a ≈ b");
        assert!(k.contains(&TokenType::Approx), "≈ should emit Approx token");
    }

    #[test]
    fn unicode_noteq_operator() {
        let k = kinds("a ≠ b");
        assert!(k.contains(&TokenType::NotEq), "≠ should emit NotEq token");
    }

    #[test]
    fn unicode_stricteq_operator() {
        let k = kinds("a ≡ b");
        assert!(k.contains(&TokenType::StrictEq), "≡ should emit StrictEq token");
    }

    #[test]
    fn at_operator() {
        let k = kinds("a @ b");
        assert!(k.contains(&TokenType::At), "@ should emit At token");
    }

    // ── String literals ──────────────────────────────────────────────────────

    #[test]
    fn string_literal_basic() {
        let toks = lex(r#""hello world""#);
        let strings: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::String).collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].value.as_deref(), Some("hello world"));
    }

    #[test]
    fn string_escape_sequences() {
        let toks = lex(r#""line1\nline2\ttab""#);
        let strings: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::String).collect();
        assert_eq!(strings.len(), 1);
        assert_eq!(strings[0].value.as_deref(), Some("line1\nline2\ttab"));
    }

    // ── Keywords ─────────────────────────────────────────────────────────────

    #[test]
    fn keywords_are_emitted() {
        let k = kinds("set x = 1\nif x\nwhile x\nprint x");
        assert!(k.contains(&TokenType::Set));
        assert!(k.contains(&TokenType::If));
        assert!(k.contains(&TokenType::While));
        assert!(k.contains(&TokenType::Print));
    }

    #[test]
    fn bool_tokens() {
        let toks = lex("true false");
        let bools: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Bool).collect();
        assert_eq!(bools.len(), 2);
    }

    // ── Indentation ──────────────────────────────────────────────────────────

    #[test]
    fn indent_dedent_emitted() {
        let src = "if x\n    y = 1\nz = 2\n";
        let k = kinds(src);
        assert!(k.contains(&TokenType::Indent), "expected Indent");
        assert!(k.contains(&TokenType::Dedent), "expected Dedent");
    }

    #[test]
    fn tab_indentation_normalised() {
        let src = "if x\n\ty = 1\nz = 2\n";
        let result = Lexer::new(src).lex_result();
        assert!(result.is_ok(), "tab indentation should not cause a lex error");
        let k: Vec<_> = result.unwrap().into_iter().map(|t| t.kind).collect();
        assert!(k.contains(&TokenType::Indent));
        assert!(k.contains(&TokenType::Dedent));
    }

    // ── Comments ─────────────────────────────────────────────────────────────

    #[test]
    fn hash_comment_stripped() {
        let toks = lex("x = 1 # this is a comment");
        let idents: Vec<_> = toks.iter().filter(|t| t.kind == TokenType::Identifier).collect();
        assert!(idents.iter().all(|t| t.value.as_deref() != Some("this")));
    }

    #[test]
    fn slash_slash_comment_stripped() {
        let toks = lex("x = 1 // comment");
        assert!(toks.iter().all(|t| t.value.as_deref() != Some("comment")));
    }

    // ── Punctuation ──────────────────────────────────────────────────────────

    #[test]
    fn all_punctuation_tokens() {
        let k = kinds("( ) [ ] , : + - * / @");
        assert!(k.contains(&TokenType::LParen));
        assert!(k.contains(&TokenType::RParen));
        assert!(k.contains(&TokenType::LBracket));
        assert!(k.contains(&TokenType::RBracket));
        assert!(k.contains(&TokenType::Comma));
        assert!(k.contains(&TokenType::Colon));
    }

    // ── New tests for OBJ / Dot / SPAWN tokenization ─────────────────────────

    #[test]
    fn obj_header_tokenization() {
        let src = "OBJ.NRML.MUT Monster(health, size):";
        let toks = lex(src);
        // Expect Obj, Dot, Identifier(NRML) or Identifier token for NRML (alias table may map NRML to Identifier),
        // Dot, Identifier(MUT) or Identifier, Identifier(Monster), LParen, Identifier(health), Comma, Identifier(size), RParen, Colon
        let kinds: Vec<_> = toks.iter().map(|t| t.kind.clone()).collect();
        // Must contain Obj and Dot tokens
        assert!(kinds.contains(&TokenType::Obj), "expected Obj token");
        assert!(kinds.contains(&TokenType::Dot), "expected Dot token");
        assert!(kinds.contains(&TokenType::LParen));
        assert!(kinds.contains(&TokenType::Colon));
    }

    #[test]
    fn spawn_lhs_tokenization() {
        let src = "goblin.NRML.MUT @ player.LIST.MUT:";
        let toks = lex(src);
        let kinds: Vec<_> = toks.iter().map(|t| t.kind.clone()).collect();
        // Should include Dot, At, Dot, Colon
        assert!(kinds.contains(&TokenType::Dot));
        assert!(kinds.contains(&TokenType::At));
        assert!(kinds.contains(&TokenType::Colon));
    }

    #[test]
    fn spawn_keyword_token() {
        let src = "SPAWN:";
        let toks = lex(src);
        let kinds: Vec<_> = toks.iter().map(|t| t.kind.clone()).collect();
        assert!(kinds.contains(&TokenType::Spawn), "expected Spawn token");
        assert!(kinds.contains(&TokenType::Colon));
    }
}