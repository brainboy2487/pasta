// src/parser/parser.rs
//! Recursive-descent / precedence-climbing parser for PASTA (refactored).
//!
//! This parser implements the grammar in `grammar.rs` and produces the AST
//! defined in `ast.rs`. It supports OBJ.<GROUP>.MUT declarations, SPAWN blocks,
//! DEF DO ... UNTIL (LX), combines (`+`), `.MUT` style forms in spawn LHS,
//! and the existing expression/statement forms.

use std::collections::HashMap;

use crate::lexer::{Token, TokenType};
use super::*;
use crate::interpreter::errors::messages as err_msg;

/// A parse error with source location and a human-readable message.
#[derive(Debug, Clone)]
pub struct ParseError {
    /// Source span where the error was detected.
    pub span: Span,
    /// Human-readable description of what was expected or found.
    pub message: String,
}

impl ParseError {
    /// Construct a `ParseError` at the given source span.
    pub fn new(span: Span, message: impl Into<String>) -> Self {
        Self { span, message: message.into() }
    }
}

/// Parser structure
pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    prec: HashMap<TokenType, i32>,
    diagnostics: Vec<ParseError>,
    eof: Token,
}

impl Parser {
    /// Construct a parser from a pre-lexed token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        let mut prec = HashMap::new();
        // precedence: higher number => binds tighter
        prec.insert(TokenType::At,       45); // matmul
        prec.insert(TokenType::Caret,    42); // exponentiation (right-associative)
        prec.insert(TokenType::Star,     40);
        prec.insert(TokenType::Slash,    40);
        prec.insert(TokenType::FloorDiv, 40); // //
        prec.insert(TokenType::Backslash,40); // \ truncating div
        prec.insert(TokenType::Percent,  40);
        prec.insert(TokenType::Plus,     30);
        prec.insert(TokenType::Minus,    30);
        prec.insert(TokenType::LShift,   25); // <<
        prec.insert(TokenType::RShift,   25); // >>
        prec.insert(TokenType::Ampersand,15); // & bitwise AND
        prec.insert(TokenType::EqEq,     20);
        prec.insert(TokenType::Neq,      20);
        prec.insert(TokenType::Lt,       20);
        prec.insert(TokenType::Gt,       20);
        prec.insert(TokenType::Lte,      20);
        prec.insert(TokenType::Gte,      20);
        prec.insert(TokenType::Approx,   20);
        prec.insert(TokenType::NotEq,    20);
        prec.insert(TokenType::StrictEq, 20);
        prec.insert(TokenType::And,      10);
        prec.insert(TokenType::Or,        5);
        prec.insert(TokenType::Pipe,      4); // |  pipeline
        prec.insert(TokenType::PipeOr,    4); // ||
        prec.insert(TokenType::PipeBoth,  4); // |&|
        prec.insert(TokenType::PipeMap,   4); // |:|
        prec.insert(TokenType::PipeArrow, 3); // |>

        Parser {
            tokens,
            pos: 0,
            prec,
            diagnostics: Vec::new(),
            eof: Token::new(TokenType::Eof, None, 0, 0),
        }
    }

    /// Parse and return Program plus diagnostics.
    pub fn parse_with_diagnostics(&mut self) -> (Program, Vec<ParseError>) {
        let mut stmts = Vec::new();
        while !self.is_eof() {
            // At top level, a DEDENT means a block closed — consume it and continue.
            // parse_statement returns None on DEDENT (without consuming), so we must
            // consume it here to avoid spinning.
            if self.check(TokenType::Dedent) { self.advance(); continue; }
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        let diags = std::mem::take(&mut self.diagnostics);
        (Program::new(stmts), diags)
    }

    /// Parse and return a Program, discarding diagnostics.
    /// This restores the legacy `parse()` API used by callers.
    pub fn parse(&mut self) -> Program {
        let (program, _diags) = self.parse_with_diagnostics();
        program
    }

    /// Parse a single expression from the token stream.
    /// Used by string interpolation (`"hello {expr}"`) and eval contexts.
    pub fn parse_single_expr(&mut self) -> Expr {
        self.parse_expression(0)
    }


    // ── Helpers ────────────────────────────────────────────────────────────

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&self.eof)
    }

    fn advance(&mut self) -> Token {
        let t = self.tokens.get(self.pos).cloned().unwrap_or_else(|| self.eof.clone());
        if self.pos < self.tokens.len() { self.pos += 1; }
        t
    }

    fn match_token(&mut self, kind: TokenType) -> bool {
        if self.check(kind.clone()) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: TokenType) -> bool {
        self.peek().kind == kind
    }

    fn is_eof(&self) -> bool {
        self.peek().kind == TokenType::Eof
    }

    fn current_span(&self) -> Span {
        let t = self.peek();
        Span::new(t.line, t.col, t.line, t.col)
    }

    fn get_prec(&self, kind: &TokenType) -> i32 {
        *self.prec.get(kind).unwrap_or(&0)
    }

    fn token_to_binop(&self, kind: &TokenType) -> BinaryOp {
        match kind {
            TokenType::Plus => BinaryOp::Add,
            TokenType::Minus => BinaryOp::Sub,
            TokenType::Star => BinaryOp::Mul,
            TokenType::Slash => BinaryOp::Div,
            TokenType::Percent => BinaryOp::Mod,
            TokenType::Caret => BinaryOp::Pow,
            TokenType::At => BinaryOp::MatMul,
            TokenType::EqEq => BinaryOp::Eq,
            TokenType::Neq => BinaryOp::Neq,
            TokenType::Lt => BinaryOp::Lt,
            TokenType::Gt => BinaryOp::Gt,
            TokenType::Lte => BinaryOp::Lte,
            TokenType::Gte => BinaryOp::Gte,
            TokenType::Approx => BinaryOp::Approx,
            TokenType::NotEq => BinaryOp::NotEq,
            TokenType::StrictEq => BinaryOp::StrictEq,
            TokenType::And => BinaryOp::And,
            TokenType::Or => BinaryOp::Or,
            TokenType::Pipe => BinaryOp::Pipe,
            TokenType::PipeOr => BinaryOp::PipeOr,
            TokenType::PipeBoth => BinaryOp::PipeBoth,
            TokenType::PipeMap => BinaryOp::PipeMap,
            TokenType::PipeArrow => BinaryOp::PipeArrow,
            TokenType::FloorDiv => BinaryOp::FloorDiv,
            TokenType::Backslash => BinaryOp::TruncDiv,
            TokenType::LShift => BinaryOp::Shl,
            TokenType::RShift => BinaryOp::Shr,
            TokenType::Ampersand => BinaryOp::BitAnd,

            _ => BinaryOp::Add, // fallback (shouldn't happen)
        }
    }

    fn recover_to_next_statement(&mut self) {
        while !self.is_eof() && !self.check(TokenType::Newline) && !self.check(TokenType::Dedent) {
            self.advance();
        }
        if self.check(TokenType::Newline) { self.advance(); }
    }

    // ── Statement parsing ────────────────────────────────────────────────────

    fn parse_statement(&mut self) -> Option<Statement> {
        // Skip leading newlines
        while self.check(TokenType::Newline) { self.advance(); }
        if self.is_eof() { return None; }

        let tok = self.peek().clone();
        // Assignment detection: allow both 'set NAME = value' and 'NAME = value' forms
        let res = match tok.kind {
            // DEDENT is a block-end sentinel — return None without consuming.
            // All body-loops (while !check(Dedent)) check BEFORE calling parse_statement,
            // so they will see this None and re-check check(Dedent)->true->exit normally.
            // The only caller that doesn't guard is parse_with_diagnostics (top-level),
            // which we fix below.
            TokenType::Dedent => return None,
            TokenType::Obj => {
                // Disambiguate: OBJ.GROUP.MUT(pa,pb) or OBJ.GROUP(pa,pb) → ObjFamNew expression
                // vs OBJ.GROUP.MUT Name(params): ... END → ObjDecl
                // Peek ahead to decide: if after OBJ.GROUP[.MUT] we see '(' → ObjFamNew
                //
                // Guard: if the very next token is `=`, the user accidentally used
                // `obj` as a variable name.  Emit a clear error.
                let next_is_assign = self.tokens.get(self.pos + 1)
                    .map(|t| t.kind == TokenType::Eq)
                    .unwrap_or(false);
                if next_is_assign {
                    let bad_tok = self.advance(); // consume `obj`
                    let span = Span::new(bad_tok.line, bad_tok.col, bad_tok.line, bad_tok.col);
                    self.recover_to_next_statement();
                    return Some(Statement::Other {
                        kind: "reserved_keyword_error".to_string(),
                        payload: Some("cannot use reserved keyword 'obj' as a variable name — try renaming it (e.g. 'result', 'data', 'record')".to_string()),
                        span,
                    });
                }
                let is_fam_new = {
                    // pos+0=OBJ, pos+1=Dot, pos+2=GROUP
                    let p3 = self.tokens.get(self.pos + 3);
                    let p4 = self.tokens.get(self.pos + 4);
                    let p5 = self.tokens.get(self.pos + 5);
                    match (p3, p4, p5) {
                        // OBJ . GROUP ( → ObjFamNew (no MUT)
                        (Some(t3), _, _) if t3.kind == TokenType::LParen => true,
                        // OBJ . GROUP . MUT ( → ObjFamNew (with MUT)
                        (Some(t3), Some(t4), Some(t5))
                            if t3.kind == TokenType::Dot
                            && t4.kind == TokenType::Identifier
                            && t4.value.as_deref().map(|s| s.eq_ignore_ascii_case("mut")).unwrap_or(false)
                            && t5.kind == TokenType::LParen => true,
                        _ => false,
                    }
                };
                if is_fam_new {
                    self.parse_expr_statement().map(Some)
                } else {
                    self.parse_obj_decl().map(Some)
                }
            },
            TokenType::DoesParentExist => {
                let tok = self.advance();
                let span = Span::new(tok.line, tok.col, tok.line, tok.col);
                while self.check(TokenType::Newline) { self.advance(); }
                let target = self.parse_expression(0);
                while self.check(TokenType::Newline) { self.advance(); }
                Ok(Some(Statement::ExprStmt {
                    expr: Expr::DoesParentExist { target: Box::new(target), span: span.clone() },
                    span,
                }))
            },
            TokenType::ColonColon => {
                // ::USE UNSAFE-READ:: or ::USE UNSAFE-WRITE::
                let start_tok = self.advance(); // consume ::
                let span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
                // expect USE (Identifier)
                if self.check(TokenType::Identifier)
                    && self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("use")).unwrap_or(false)
                {
                    self.advance(); // consume USE
                }
                // expect UNSAFE (Identifier)
                let write_access = if self.check(TokenType::Identifier)
                    && self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("unsafe")).unwrap_or(false)
                {
                    self.advance(); // consume UNSAFE
                    // expect - (Minus)
                    if self.check(TokenType::Minus) { self.advance(); }
                    // expect READ or WRITE (Identifier)
                    if self.check(TokenType::Identifier) {
                        let rw = self.advance();
                        rw.value.as_deref().map(|s| s.eq_ignore_ascii_case("write")).unwrap_or(false)
                    } else { false }
                } else { false };
                // consume closing ::
                if self.check(TokenType::ColonColon) { self.advance(); }
                while self.check(TokenType::Newline) { self.advance(); }
                Ok(Some(Statement::UseUnsafe { write_access, span }))
            },
            // Support `MOD` module declarations lexed as Identifier 'MOD'
            TokenType::Identifier => {
                if let Some(val) = self.peek().value.as_ref() {
                    if val.eq_ignore_ascii_case("mod") {
                        self.parse_module_decl().map(Some)
                    } else {
                        // Recognize RET.NOW / RET.LATE both as a single absorbed token
                        // ("RET.NOW") and as the three-token form ("RET" "." "NOW"/"LATE")
                        // that results when the lexer did not absorb the dot.
                        let is_ret = {
                            let single = self.peek().value.as_deref()
                                .map(|s| { let u = s.to_uppercase(); u.starts_with("RET.NOW") || u.starts_with("RET.LATE") })
                                .unwrap_or(false);
                            let three = self.peek().value.as_deref()
                                .map(|s| s.eq_ignore_ascii_case("ret"))
                                .unwrap_or(false)
                                && self.pos + 2 < self.tokens.len()
                                && self.tokens[self.pos + 1].kind == TokenType::Dot
                                && self.tokens[self.pos + 2].value.as_deref()
                                    .map(|s| s.eq_ignore_ascii_case("now") || s.eq_ignore_ascii_case("late"))
                                    .unwrap_or(false);
                            single || three
                        };
                        if is_ret {
                            self.parse_ret_statement().map(Some)
                        } else if self.peek_is_assign() || self.peek_is_multi_assign() {
                            self.parse_assignment_statement().map(Some)
                        } else if self.peek_is_priority_override() {
                            self.parse_priority_override().map(Some)
                        } else {
                            self.parse_expr_statement().map(Some)
                        }
                    }
                } else {
                    self.parse_expr_statement().map(Some)
                }
            }
            TokenType::Spawn => self.parse_spawn_block().map(Some),
            TokenType::Def => self.parse_def_statement().map(Some),
            TokenType::From => self.parse_from_block().map(Some),
            TokenType::Return => {
                // `return expr` is sugar for `RET.NOW: expr`
                let ret_tok = self.advance();
                let span = Span::new(ret_tok.line, ret_tok.col, ret_tok.line, ret_tok.col);
                if self.check(TokenType::Colon) { self.advance(); }
                while self.check(TokenType::Newline) { self.advance(); }
                let value = self.parse_expression(0);
                while self.check(TokenType::Newline) { self.advance(); }
                Ok(Some(Statement::RetNow { value, span }))
            },
            TokenType::Try => self.parse_try_statement().map(Some),
            // set/let/make — lexer emits Set token; consume it then parse assignment
            TokenType::Set => { self.advance(); self.parse_assignment_statement().map(Some) },
            TokenType::Const => {
                self.advance(); // consume CONST
                self.parse_assignment_statement().map(|stmt| {
                    Some(match stmt {
                        Statement::Assignment { target, value, span } =>
                            Statement::ConstAssignment { target, value, span },
                        other => other,
                    })
                })
            },
            TokenType::If => self.parse_if_statement().map(Some),
            TokenType::Do => self.parse_do_statement().map(Some),
            TokenType::While => self.parse_while_statement().map(Some),
            TokenType::For => self.parse_for_in_statement().map(Some),
            TokenType::Print => self.parse_print_statement().map(Some),
            TokenType::Break => {
                let t = self.advance();
                let span = Span::new(t.line, t.col, t.line, t.col);
                while self.check(TokenType::Newline) { self.advance(); }
                Ok(Some(Statement::Break { span }))
            },
            TokenType::Continue => {
                let t = self.advance();
                let span = Span::new(t.line, t.col, t.line, t.col);
                while self.check(TokenType::Newline) { self.advance(); }
                Ok(Some(Statement::Continue { span }))
            },
            TokenType::End => { self.advance(); while self.check(TokenType::Newline) { self.advance(); } Ok(None) },
            // PASS / NOOP — do nothing
            TokenType::Pass => { self.advance(); while self.check(TokenType::Newline) { self.advance(); } Ok(None) },
            // ASSERT expr — runtime assertion
            TokenType::Assert => {
                let t = self.advance();
                let span = Span::new(t.line, t.col, t.line, t.col);
                let cond = self.parse_expression(0);
                while self.check(TokenType::Newline) { self.advance(); }
                // Compile to: IF NOT cond: error("Assertion failed") END
                let not_cond = Expr::Binary {
                    op: BinaryOp::Not,
                    left: Box::new(Expr::Number(0.0, span.clone())),
                    right: Box::new(cond),
                    span: span.clone(),
                };
                let err_msg = Expr::String("Assertion failed".to_string(), span.clone());
                let raise_fn = Identifier::new("error".to_string(), span.clone());
                let raise_call = Expr::Call { callee: Box::new(Expr::Identifier(raise_fn)), args: vec![err_msg], span: span.clone() };
                Ok(Some(Statement::If {
                    conditions: vec![not_cond],
                    then_body: vec![Statement::ExprStmt { expr: raise_call, span: span.clone() }],
                    else_body: None,
                    scope_modifier: None,
                    span,
                }))
            },
            // UNLESS cond: body END — equivalent to IF NOT cond
            TokenType::Unless => self.parse_unless_statement().map(Some),
            
            // ── v1.4.4 Pointer statements ────────────────────────────────────
            TokenType::Goto => self.parse_goto_statement().map(Some),
            TokenType::Pull => self.parse_pull_statement().map(Some),
            TokenType::Push => self.parse_push_statement().map(Some),
            TokenType::Alloc => self.parse_alloc_statement().map(Some),
            TokenType::Free => self.parse_free_statement().map(Some),
            TokenType::Info => self.parse_info_statement().map(Some),
            TokenType::Seek => self.parse_seek_statement().map(Some),
            TokenType::Swap => self.parse_swap_statement().map(Some),

            // Guard: keyword-as-variable-name detection.
            // If a reserved keyword is immediately followed by `=`, the user
            // is trying to use it as a variable name.  Emit a clear diagnostic
            // and skip the statement so execution can continue.
            ref kind if {
                let next_is_eq = self.tokens.get(self.pos + 1)
                    .map(|t| t.kind == TokenType::Eq)
                    .unwrap_or(false);
                next_is_eq && matches!(kind,
                    TokenType::Limit | TokenType::Over | TokenType::Set |
                    TokenType::Group | TokenType::Class | TokenType::Match |
                    TokenType::When  | TokenType::With  | TokenType::From  |
                    TokenType::NoneVal | TokenType::Pass | TokenType::Yield |
                    TokenType::Return | TokenType::Until | TokenType::Unless |
                    TokenType::Assert | TokenType::Typeof | TokenType::Await |
                    TokenType::Const  | TokenType::Export | TokenType::Draw |
                    TokenType::Frame  | TokenType::Loop   | TokenType::Goto  |
                    TokenType::Build  | TokenType::Learn  | TokenType::Tensor |
                    TokenType::Do     | TokenType::Pause  | TokenType::Unpause |
                    TokenType::Restart | TokenType::Wait  | TokenType::In |
                    TokenType::As     | TokenType::Step   | TokenType::Then
                )
            } => {
                let bad_tok = self.advance(); // consume the keyword
                let name = bad_tok.value.as_deref()
                    .unwrap_or_else(|| "keyword")
                    .to_lowercase();
                let span = Span::new(bad_tok.line, bad_tok.col, bad_tok.line, bad_tok.col);
                self.recover_to_next_statement();
                Ok(Some(Statement::Other {
                    kind: "reserved_keyword_error".to_string(),
                    payload: Some(format!(
                        "cannot use reserved keyword '{}' as a variable name — rename it (e.g. 'my_{}')",
                        name, name
                    )),
                    span,
                }))
            }

            _ => self.parse_expr_statement().map(Some),
        };

        match res {
            Ok(sopt) => sopt,
            Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
        }
    }

    // ── OBJ declaration parsing ──────────────────────────────────────────────

    fn parse_obj_decl(&mut self) -> Result<Statement, ParseError> {
        // Expect: OBJ . FamilyGroup . MUT Name ( params ) : NEWLINE INDENT { members } DEDENT END
        let start_tok = self.advance(); // OBJ
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // expect Dot
        if !self.match_token(TokenType::Dot) {
            return Err(ParseError::new(self.current_span(), "Expected '.' after OBJ".to_string()));
        }

        // family group: Identifier token (NRML, LIST, ARY, TNSR)
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected family group after OBJ.".to_string()));
        }
        let fg_tok = self.advance();
        let family_group = fg_tok.value.unwrap_or_default();

        // expect Dot
        if !self.match_token(TokenType::Dot) {
            return Err(ParseError::new(self.current_span(), "Expected '.' after family group".to_string()));
        }

        // expect MUT (could be Identifier "MUT" or canonical token via alias)
        if !self.check(TokenType::Identifier) && !self.check(TokenType::Def) {
            // allow Identifier "MUT"
        }
        // consume token (prefer Identifier)
        let mut_tok = self.advance();
        let mut_name = mut_tok.value.unwrap_or_default();
        if mut_name.to_uppercase() != "MUT" {
            // tolerate but warn
        }

        // expect Identifier (object name)
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected object name after MUT".to_string()));
        }
        let name_tok = self.advance();
        let name = Identifier::new(name_tok.value.unwrap_or_default(), Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col));

        // params: '(' [ident, ...] ')'
        let mut params = Vec::new();
        if self.match_token(TokenType::LParen) {
            while !self.check(TokenType::RParen) && !self.is_eof() {
                if self.check(TokenType::Identifier) {
                    let p = self.advance();
                    params.push(Identifier::new(p.value.unwrap_or_default(), Span::new(p.line, p.col, p.line, p.col)));
                }
                if self.check(TokenType::Comma) { self.advance(); } else { break; }
            }
            if !self.match_token(TokenType::RParen) {
                return Err(ParseError::new(self.current_span(), "Expected ')' after parameter list".to_string()));
            }
        }

        // expect Colon
        if !self.match_token(TokenType::Colon) {
            return Err(ParseError::new(self.current_span(), "Expected ':' after OBJ header".to_string()));
        }

        // optional newline then INDENT
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented body after OBJ header".to_string()));
        }

        // parse members until Dedent
        let mut fields = Vec::new();
        let mut mutation_table = Vec::new();
        let mut constructor: Option<Constructor> = None;
        let mut methods = Vec::new();

        while !self.check(TokenType::Dedent) && !self.is_eof() {
            // skip newlines
            if self.check(TokenType::Newline) { self.advance(); continue; }

            // lookahead for FIELD, DEF MUTATION_TABLE, CONSTRUCTOR, DEF (method)
            if self.check(TokenType::Identifier) {
                let look = self.peek().clone();
                if let Some(val) = &look.value {
                    let up = val.to_uppercase();
                    match up.as_str() {
                        "FIELD" => {
                            if let Ok(f) = self.parse_field_decl() { fields.push(f); } else { self.recover_to_next_statement(); }
                            continue;
                        }
                        "DEF" => {
                            // could be MUTATION_TABLE or method
                            // peek next tokens to decide
                            // consume DEF
                            let pos_before = self.pos;
                            let _def_tok = self.advance();
                            if self.check(TokenType::Identifier) {
                                let next = self.peek().clone();
                                if let Some(nv) = &next.value {
                                    if nv.to_uppercase() == "MUTATION_TABLE" {
                                        // consume and parse mutation table
                                        let _ = self.advance();
                                        // expect Colon
                                        if self.match_token(TokenType::Colon) {
                                            if self.check(TokenType::Newline) { self.advance(); }
                                            if self.match_token(TokenType::Indent) {
                                                // parse mut entries until Dedent
                                                while !self.check(TokenType::Dedent) && !self.is_eof() {
                                                    if self.check(TokenType::Newline) { self.advance(); continue; }
                                                    if self.check(TokenType::Identifier) {
                                                        let mut_entry = self.parse_mut_entry()?;
                                                        mutation_table.push(mut_entry);
                                                    } else {
                                                        // skip unknown
                                                        self.advance();
                                                    }
                                                }
                                                // consume Dedent
                                                if !self.match_token(TokenType::Dedent) {
                                                    return Err(ParseError::new(self.current_span(), "Expected DEDENT after MUTATION_TABLE body".to_string()));
                                                }
                                                // expect END
                                                if !self.match_token(TokenType::End) {
                                                    return Err(ParseError::new(self.current_span(), "Expected END after MUTATION_TABLE".to_string()));
                                                }
                                                // optional newline
                                                if self.check(TokenType::Newline) { self.advance(); }
                                                continue;
                                            } else {
                                                return Err(ParseError::new(self.current_span(), "Expected indented MUTATION_TABLE body".to_string()));
                                            }
                                        } else {
                                            return Err(ParseError::new(self.current_span(), "Expected ':' after MUTATION_TABLE".to_string()));
                                        }
                                    }
                                }
                            }
                            // not a MUTATION_TABLE; rewind and parse as method
                            self.pos = pos_before;
                            let method = self.parse_method_decl()?;
                            methods.push(method);
                            continue;
                        }
                        "CONSTRUCTOR" => {
                            let ctor = self.parse_constructor_decl()?;
                            constructor = Some(ctor);
                            continue;
                        }
                        _ => {
                            // unknown member; try to parse as statement and ignore
                            if let Some(_s) = self.parse_statement() { /* ignore */ }
                            continue;
                        }
                    }
                }
            }

            // fallback: parse statement and ignore
            if self.parse_statement().is_some() { /* ignore */ }
        }

        // consume Dedent
        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), "Expected DEDENT after OBJ body".to_string()));
        }

        // expect END
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), "Expected END after OBJ declaration".to_string()));
        }

        // optional newline
        if self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::ObjDecl {
            name,
            family_group,
            params,
            fields,
            mutation_table,
            constructor,
            methods,
            span: start_span,
        })
    }

    fn parse_field_decl(&mut self) -> Result<FieldDecl, ParseError> {
        // FIELD name = expr NEWLINE
        let tok = self.advance(); // FIELD
        let start_span = Span::new(tok.line, tok.col, tok.line, tok.col);
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected identifier after FIELD".to_string()));
        }
        let name_tok = self.advance();
        let name = Identifier::new(name_tok.value.unwrap_or_default(), Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col));
        if !self.match_token(TokenType::Eq) {
            return Err(ParseError::new(self.current_span(), "Expected '=' in FIELD declaration".to_string()));
        }
        let value = self.parse_expression(0);
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(FieldDecl { name, value, span: start_span })
    }

    fn parse_mut_entry(&mut self) -> Result<MutEntry, ParseError> {
        // MUT name = DO ... END  (or MUT name = DO expr)
        let mut_tok = self.advance(); // MUT
        let start_span = Span::new(mut_tok.line, mut_tok.col, mut_tok.line, mut_tok.col);
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected mutation name after MUT".to_string()));
        }
        let name_tok = self.advance();
        let name = Identifier::new(name_tok.value.unwrap_or_default(), Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col));
        if !self.match_token(TokenType::Eq) {
            return Err(ParseError::new(self.current_span(), "Expected '=' after mutation name".to_string()));
        }
        // Expect DO
        if !self.match_token(TokenType::Do) {
            return Err(ParseError::new(self.current_span(), "Expected DO in mutation body".to_string()));
        }
        // If next is Newline + Indent, parse block
        if self.check(TokenType::Newline) { self.advance(); }
        if self.match_token(TokenType::Indent) {
            let mut body = Vec::new();
            while !self.check(TokenType::Dedent) && !self.is_eof() {
                if let Some(s) = self.parse_statement() { body.push(s); }
            }
            if !self.match_token(TokenType::Dedent) {
                return Err(ParseError::new(self.current_span(), "Expected DEDENT after mutation DO body".to_string()));
            }
            // expect END
            if !self.match_token(TokenType::End) {
                return Err(ParseError::new(self.current_span(), "Expected END after mutation DO block".to_string()));
            }
            if self.check(TokenType::Newline) { self.advance(); }
            Ok(MutEntry { name, body: MutBody::Block(body), span: start_span })
        } else {
            // single-line expression until NEWLINE or END token
            let expr = self.parse_expression(0);
            // optional END token
            if self.check(TokenType::End) { self.advance(); }
            if self.check(TokenType::Newline) { self.advance(); }
            Ok(MutEntry { name, body: MutBody::Expr(expr), span: start_span })
        }
    }

    fn parse_constructor_decl(&mut self) -> Result<Constructor, ParseError> {
        // CONSTRUCTOR(params): NEWLINE INDENT { statements } DEDENT END NEWLINE?
        let tok = self.advance(); // CONSTRUCTOR
        let start_span = Span::new(tok.line, tok.col, tok.line, tok.col);
        // expect '(' 'params' ')'
        if !self.match_token(TokenType::LParen) {
            return Err(ParseError::new(self.current_span(), "Expected '(' after CONSTRUCTOR".to_string()));
        }
        // accept any identifier list (commonly "params")
        let mut params = Vec::new();
        while !self.check(TokenType::RParen) && !self.is_eof() {
            if self.check(TokenType::Identifier) {
                let p = self.advance();
                params.push(Identifier::new(p.value.unwrap_or_default(), Span::new(p.line, p.col, p.line, p.col)));
            }
            if self.check(TokenType::Comma) { self.advance(); } else { break; }
        }
        if !self.match_token(TokenType::RParen) {
            return Err(ParseError::new(self.current_span(), "Expected ')' after constructor params".to_string()));
        }
        if !self.match_token(TokenType::Colon) {
            return Err(ParseError::new(self.current_span(), "Expected ':' after CONSTRUCTOR header".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented constructor body".to_string()));
        }
        let mut body = Vec::new();
        while !self.check(TokenType::Dedent) && !self.is_eof() {
            if let Some(s) = self.parse_statement() { body.push(s); }
        }
        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), "Expected DEDENT after constructor body".to_string()));
        }
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), "Expected END after constructor".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(Constructor { params, body, span: start_span })
    }

    fn parse_method_decl(&mut self) -> Result<MethodDecl, ParseError> {
        // DEF name(params): NEWLINE INDENT { statements } DEDENT END
        let def_tok = self.advance(); // DEF
        let start_span = Span::new(def_tok.line, def_tok.col, def_tok.line, def_tok.col);
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected method name after DEF".to_string()));
        }
        let name_tok = self.advance();
        let name = Identifier::new(name_tok.value.unwrap_or_default(), Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col));
        // params
        let mut params = Vec::new();
        if self.match_token(TokenType::LParen) {
            while !self.check(TokenType::RParen) && !self.is_eof() {
                if self.check(TokenType::Identifier) {
                    let p = self.advance();
                    params.push(Identifier::new(p.value.unwrap_or_default(), Span::new(p.line, p.col, p.line, p.col)));
                }
                if self.check(TokenType::Comma) { self.advance(); } else { break; }
            }
            if !self.match_token(TokenType::RParen) {
                return Err(ParseError::new(self.current_span(), "Expected ')' after method params".to_string()));
            }
        }
        if !self.match_token(TokenType::Colon) {
            return Err(ParseError::new(self.current_span(), "Expected ':' after method header".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented method body".to_string()));
        }
        let mut body = Vec::new();
        while !self.check(TokenType::Dedent) && !self.is_eof() {
            if let Some(s) = self.parse_statement() { body.push(s); }
        }
        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), "Expected DEDENT after method body".to_string()));
        }
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), "Expected END after method".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(MethodDecl { name, params, body, span: start_span })
    }

    // ── SPAWN block parsing ──────────────────────────────────────────────────

    fn parse_spawn_block(&mut self) -> Result<Statement, ParseError> {
        // SPAWN : NEWLINE INDENT { SpawnEntry } DEDENT END
        let start_tok = self.advance(); // SPAWN
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
        if !self.match_token(TokenType::Colon) {
            return Err(ParseError::new(self.current_span(), "Expected ':' after SPAWN".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented SPAWN body".to_string()));
        }
        let mut entries = Vec::new();
        while !self.check(TokenType::Dedent) && !self.is_eof() {
            if self.check(TokenType::Newline) { self.advance(); continue; }
            // parse spawn entry: LHS ':' NEWLINE INDENT { statements } DEDENT END
            if let Ok(entry) = self.parse_spawn_entry() {
                entries.push(entry);
            } else {
                self.recover_to_next_statement();
            }
        }
        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), "Expected DEDENT after SPAWN body".to_string()));
        }
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), "Expected END after SPAWN block".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::SpawnBlock { entries, span: start_span })
    }

    fn parse_spawn_entry(&mut self) -> Result<SpawnEntry, ParseError> {
        // parse LHS (expr . Family . MUT [ @ expr . Family . MUT ]) ':' NEWLINE INDENT actions DEDENT END
        let start_span = self.current_span();
        let (father_expr, father_family, mother_expr, mother_family) = self.parse_spawn_lhs()?;
        if !self.match_token(TokenType::Colon) {
            return Err(ParseError::new(self.current_span(), "Expected ':' after SPAWN LHS".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented SPAWN actions".to_string()));
        }
        let mut actions = Vec::new();
        while !self.check(TokenType::Dedent) && !self.is_eof() {
            if let Some(s) = self.parse_statement() { actions.push(s); }
        }
        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), "Expected DEDENT after SPAWN actions".to_string()));
        }
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), "Expected END after SPAWN entry".to_string()));
        }
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(SpawnEntry {
            father_expr,
            father_family,
            mother_expr,
            mother_family,
            actions,
            span: start_span,
        })
    }

    fn parse_spawn_lhs(&mut self) -> Result<(Expr, Option<String>, Expr, Option<String>), ParseError> {
        // parse pattern: expr . Family . MUT [ @ expr . Family . MUT ]
        // parse first expr (could be identifier or call)
        let father_expr = self.parse_expression(0);
        let father_family: Option<String>;
        // expect Dot Family Dot MUT
        if self.match_token(TokenType::Dot) {
            if self.check(TokenType::Identifier) {
                let fg = self.advance();
                father_family = Some(fg.value.unwrap_or_default());
                if !self.match_token(TokenType::Dot) {
                    return Err(ParseError::new(self.current_span(), "Expected '.' after family in spawn LHS".to_string()));
                }
                // expect MUT (Identifier "MUT")
                if self.check(TokenType::Identifier) {
                    let _mut_tok = self.advance();
                    // ignore value; presence is enough
                } else {
                    return Err(ParseError::new(self.current_span(), "Expected MUT after family in spawn LHS".to_string()));
                }
            } else {
                return Err(ParseError::new(self.current_span(), "Expected family identifier after '.' in spawn LHS".to_string()));
            }
        } else {
            return Err(ParseError::new(self.current_span(), "Expected '.' family qualifier in spawn LHS".to_string()));
        }

        // optional mother part prefixed by '@'
        let mut mother_expr = Expr::Raw("".to_string(), Span::dummy());
        let mut mother_family: Option<String> = None;
        if self.match_token(TokenType::At) {
            mother_expr = self.parse_expression(0);
            if self.match_token(TokenType::Dot) {
                if self.check(TokenType::Identifier) {
                    let fg = self.advance();
                    mother_family = Some(fg.value.unwrap_or_default());
                    if !self.match_token(TokenType::Dot) {
                        return Err(ParseError::new(self.current_span(), "Expected '.' after mother family".to_string()));
                    }
                    if self.check(TokenType::Identifier) {
                        let _mut_tok = self.advance(); // MUT
                    } else {
                        return Err(ParseError::new(self.current_span(), "Expected MUT after mother family".to_string()));
                    }
                } else {
                    return Err(ParseError::new(self.current_span(), "Expected mother family identifier".to_string()));
                }
            } else {
                return Err(ParseError::new(self.current_span(), "Expected '.' after mother expression in spawn LHS".to_string()));
            }
        }

        Ok((father_expr, father_family, mother_expr, mother_family))
    }

    // ── DEF / IF / DO / PRINT / ASSIGN / PRIORITY ────────────────────────────

    fn parse_def_statement(&mut self) -> Result<Statement, ParseError> {
        // Support both function defs and DEF DO ... UNTIL (handled separately)
        let start_tok = self.advance(); // DEF
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // If next token is DO -> parse DefDoUntil
        if self.check(TokenType::Do) {
            // rewind one token (we consumed DEF), parse DefDoUntil expecting name after DO
            // Actually grammar: DEF DO name UNTIL cond: ...
            // We already consumed DEF; ensure DO present
            if !self.match_token(TokenType::Do) {
                return Err(ParseError::new(self.current_span(), "Expected DO after DEF".to_string()));
            }
            // expect identifier name
            if !self.check(TokenType::Identifier) {
                return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_IDENTIFIER_AFTER_DEF));
            }
            let name_tok = self.advance();
            let name = Identifier::new(name_tok.value.unwrap_or_default(), Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col));
            // expect UNTIL
            if !self.match_token(TokenType::Identifier) {
                return Err(ParseError::new(self.current_span(), "Expected UNTIL after DEF DO name".to_string()));
            }
            // parse condition expression
            let cond = self.parse_expression(0);
            // expect Colon
            if !self.match_token(TokenType::Colon) {
                return Err(ParseError::new(self.current_span(), "Expected ':' after DEF DO UNTIL header".to_string()));
            }
            if self.check(TokenType::Newline) { self.advance(); }
            if !self.match_token(TokenType::Indent) {
                return Err(ParseError::new(self.current_span(), "Expected indented body after DEF DO header".to_string()));
            }
            let mut body = Vec::new();
            while !self.check(TokenType::Dedent) && !self.is_eof() {
                if let Some(s) = self.parse_statement() { body.push(s); }
            }
            if !self.match_token(TokenType::Dedent) {
                return Err(ParseError::new(self.current_span(), "Expected DEDENT after DEF DO body".to_string()));
            }
            if !self.match_token(TokenType::End) {
                return Err(ParseError::new(self.current_span(), "Expected END after DEF DO body".to_string()));
            }
            // optional LX annotation: ( LX UNTIL cond )
            let mut lx_cond: Option<Expr> = None;
            if self.match_token(TokenType::LParen) {
                // expect Identifier LX
                if self.check(TokenType::Identifier) {
                    let lx_tok = self.advance();
                    if lx_tok.value.unwrap_or_default().to_uppercase() == "LX"
                        && self.match_token(TokenType::Identifier) {
                            // expect UNTIL then condition
                            let cond2 = self.parse_expression(0);
                            lx_cond = Some(cond2);
                        }
                }
                // consume closing paren if present
                if self.check(TokenType::RParen) { self.advance(); }
            }
            if self.check(TokenType::Newline) { self.advance(); }
            let defdo = DefDoUntil { name, until_condition: cond, body, lx_condition: lx_cond, span: start_span };
            return Ok(Statement::DefDoUntil(defdo));
        }

        // More permissive DEF parsing: allow DEF name DO, DEF name = DO, DEF name:, DEF name = :, DEF name =, and any logical combination
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_IDENTIFIER_AFTER_DEF));
        }
        let name_tok = self.advance();
        let mut full_name = name_tok.value.unwrap_or_default();
        let name_span = Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col);
        // Consume dotted segments: e.g. DEF tensor.zeros → full_name = "tensor.zeros"
        while self.check(TokenType::Dot) {
            self.advance(); // consume '.'
            if self.check(TokenType::Identifier) {
                let seg = self.advance();
                full_name.push('.');
                full_name.push_str(&seg.value.unwrap_or_default());
            }
        }
        let name = Identifier::new(full_name, name_span);
        // Optional parameter list: DEF name(param1, param2):
        let mut params: Vec<Identifier> = Vec::new();
        if self.check(TokenType::LParen) {
            self.advance(); // consume '('
            while !self.check(TokenType::RParen) && !self.is_eof() {
                if self.check(TokenType::Identifier) {
                    let p = self.advance();
                    params.push(Identifier::new(p.value.unwrap_or_default(), Span::new(p.line, p.col, p.line, p.col)));
                }
                if self.check(TokenType::Comma) { self.advance(); } else { break; }
            }
            if self.check(TokenType::RParen) { self.advance(); } // consume ')'
        }

        // Accept any combination of =, DO, or : after DEF name/params
        let mut saw_eq = false;
        let mut _saw_do = false;
        let mut saw_colon = false;
        // Accept any order: =, DO, :
        for _ in 0..3 {
            if self.match_token(TokenType::Eq) { saw_eq = true; continue; }
            if self.match_token(TokenType::Do) { _saw_do = true; continue; }
            if self.match_token(TokenType::Colon) { saw_colon = true; continue; }
            break;
        }

        // If = is present but not DO or :, default to DO
        if saw_eq && !_saw_do && !saw_colon {
            _saw_do = true;
        }
        // If only : is present, treat as block body
        // If only DO is present, treat as block body
        // If both are present, treat as block body

        // Single-line body: DEF foo(x): x + 1
        if saw_colon && !self.check(TokenType::Newline) && !self.check(TokenType::Indent) && !self.is_eof() {
            let mut inline_body = Vec::new();
            while !self.check(TokenType::Newline) && !self.check(TokenType::Eof) && !self.is_eof() {
                if self.check(TokenType::End) { self.advance(); break; }
                if let Some(s) = self.parse_statement() { inline_body.push(s); } else { break; }
            }
            if self.check(TokenType::Newline) { self.advance(); }
            self.match_token(TokenType::End);
            while self.check(TokenType::Newline) { self.advance(); }
            return Ok(Statement::FunctionDef { name, params, body: inline_body, span: start_span });
        }

        while self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            // Tolerate missing indent — treat as empty body rather than hard error
            self.match_token(TokenType::End);
            while self.check(TokenType::Newline) { self.advance(); }
            return Ok(Statement::FunctionDef { name, params, body: vec![], span: start_span });
        }
        let mut body = Vec::new();
        while !self.check(TokenType::Dedent) && !self.is_eof() {
            // Consume bare End tokens that close nested blocks (WHILE...END, IF...END)
            // written explicitly inside a DEF body. INDENT/DEDENT already bounds
            // the nested block; these End tokens are no-ops at this level.
            if self.check(TokenType::End) {
                self.advance();
                while self.check(TokenType::Newline) { self.advance(); }
                continue;
            }
            if let Some(stmt) = self.parse_statement() { body.push(stmt); }
        }
        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_DEDENT_AFTER_DEF));
        }
        // The closing END keyword is optional: some scripts write it explicitly,
        // others rely on indentation alone. Consume it if present.
        self.match_token(TokenType::End);
        while self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::FunctionDef { name, params, body, span: start_span })
    }

    fn parse_ret_statement(&mut self) -> Result<Statement, ParseError> {

        // The lexer absorbs "RET.NOW" and "RET.LATE" as a single Identifier token
        // when dot-absorption is active. When emitted as three separate tokens
        // ("RET" "." "NOW"/"LATE"), merge them here before dispatching.
        let (ret_tok, token_val) = {
            let peek_val = self.peek().value.as_deref().unwrap_or("").to_uppercase();
            if peek_val.starts_with("RET.NOW") || peek_val.starts_with("RET.LATE") {
                // Single-token form: Identifier("RET.NOW") or Identifier("RET.LATE")
                let t = self.advance();
                let v = t.value.clone().unwrap_or_default().to_uppercase();
                (t, v)
            } else {
                // Three-token form: Identifier("RET") Dot Identifier("NOW"/"LATE")
                let base = self.advance(); // consume "RET"
                self.advance(); // consume "."
                let suffix = self.advance(); // consume "NOW" or "LATE"
                let suffix_up = suffix.value.unwrap_or_default().to_uppercase();
                let merged = format!("RET.{}", suffix_up);
                (base, merged)
            }
        };
        let span = Span::new(ret_tok.line, ret_tok.col, ret_tok.line, ret_tok.col);

        let variant = if token_val.starts_with("RET.NOW") {
            "NOW"
        } else if token_val.starts_with("RET.LATE") {
            "LATE"
        } else {
            return Err(ParseError::new(span,
                format!("Unknown RET form '{}': expected RET.NOW or RET.LATE", token_val)));
        };

        match variant {
            "NOW" => {
                // consume optional (expr)
                let value = if self.check(TokenType::LParen) {
                    self.advance(); // consume '('
                    // If there's an expression before the closing paren, parse it.
                    
                    if !self.check(TokenType::RParen) {
                        let e = self.parse_expression(0);
                        // allow trailing ')' if present
                        if self.check(TokenType::RParen) { self.advance(); }
                        e
                    } else {
                        // empty parentheses: RET.NOW(): expr  OR  RET.NOW()
                        self.advance(); // consume ')'
                        // If a colon + expression follows, that IS the return value
                        if self.check(TokenType::Colon) {
                            self.advance(); // consume ':'
                            while self.check(TokenType::Newline) { self.advance(); }
                            self.parse_expression(0)
                        } else {
                            // Truly bare RET.NOW() with no value — return None sentinel
                            Expr::Number(0.0, span.clone())
                        }
                    }
                } else {
                    // no paren form; consume optional colon/newlines then parse expression
                    if self.check(TokenType::Colon) { self.advance(); }
                    while self.check(TokenType::Newline) { self.advance(); }
                    self.parse_expression(0)
                };

                // If paren form was used and a trailing colon remains, skip it
                if self.check(TokenType::Colon) { self.advance(); }
                while self.check(TokenType::Newline) { self.advance(); }

                Ok(Statement::RetNow { value, span })

}

            "LATE" => {
                if !self.check(TokenType::LParen) {
                    return Err(ParseError::new(self.current_span(),
                        "RET.LATE requires a condition in parentheses: RET.LATE(500ms) or RET.LATE(WHEN funcname)".to_string()
                    ));
                }
                self.advance(); // consume '('
                let condition = if self.check(TokenType::When) {
                    self.advance(); // consume WHEN
                    let tok = self.peek();
                    let fn_name = tok.value.clone().unwrap_or_default();
                    if fn_name.is_empty() {
                        return Err(ParseError::new(self.current_span(),
                            "RET.LATE(WHEN ...) requires a function name, e.g. RET.LATE(WHEN on_done)".to_string()
                        ));
                    }
                    self.advance(); // consume fn_name
                    RetLateCondition::WhenCalled(fn_name)
                } else if self.check(TokenType::Number) {
                    let cond_expr = self.parse_expression(1); // min_prec=1 suppresses implicit juxtaposition so `500 ms` isn't consumed as `500 + ms`
                    // Require the 'ms' suffix identifier immediately after the number
                    let ms_tok = self.peek();
                    if ms_tok.kind == TokenType::Identifier
                        && ms_tok.value.as_deref().map(|v| v.eq_ignore_ascii_case("ms")).unwrap_or(false)
                    {
                        self.advance(); // consume 'ms'
                        RetLateCondition::AfterMs(cond_expr)
                    } else {
                        return Err(ParseError::new(self.current_span(),
                            "RET.LATE time-based form requires 'ms' suffix, e.g. RET.LATE(500ms): value".to_string()
                        ));
                    }
                } else {
                    return Err(ParseError::new(self.current_span(),
                        "RET.LATE requires 'NUMBERms' or 'WHEN funcname', e.g. RET.LATE(500ms) or RET.LATE(WHEN on_done)".to_string()
                    ));
                };
                if self.check(TokenType::RParen) { self.advance(); }
                if self.check(TokenType::Colon) { self.advance(); }
                while self.check(TokenType::Newline) { self.advance(); }
                let value = self.parse_expression(0);
                Ok(Statement::RetLate { value, condition, span })
            }

            _ => unreachable!(),
        }
    }

    fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // IF
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
        // Parse optional scope modifier: IF(UNBIND_SCOPE) or IF(BIND_SCOPE)
        let scope_modifier = self.parse_scope_modifier();
        let mut conditions = vec![self.parse_expression(6)];
        while self.match_token(TokenType::Or) {
            conditions.push(self.parse_expression(6));
        }
        // Accept: IF cond THEN | IF cond DO | IF cond: | IF cond { ... } | IF cond\n<body>
        let newline_body = !self.check(TokenType::Then)
            && !self.check(TokenType::Do)
            && !self.check(TokenType::Colon)
            && !self.check(TokenType::LBrace)
            && self.check(TokenType::Newline);
        if !self.match_token(TokenType::Then)
            && !self.match_token(TokenType::Do)
            && !self.match_token(TokenType::Colon)
            && !self.check(TokenType::LBrace)
            && !newline_body
        {
            return Err(ParseError::new(self.current_span(), "Expected THEN, DO, :, '{', or newline-indented body after IF condition(s)".to_string()));
        }
        // For newline-style bodies the NEWLINE is left in the stream so that
        // parse_do_body (which starts by consuming it) works correctly.
        let then_body = self.parse_do_body(&start_span)?;
        // Accept OTHERWISE/else with optional DO or :
        let else_body = if self.match_token(TokenType::Otherwise) {
            let _ = self.match_token(TokenType::Do) || self.match_token(TokenType::Colon);
            Some(self.parse_do_body(&start_span)?)
        } else { None };
        while self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::If { conditions, then_body, else_body, scope_modifier, span: start_span })
    }

    fn parse_unless_statement(&mut self) -> Result<Statement, ParseError> {
        let t = self.advance(); // UNLESS
        let span = Span::new(t.line, t.col, t.line, t.col);
        let cond = self.parse_expression(0);
        let not_cond = Expr::Binary {
            op: BinaryOp::Not,
            left: Box::new(Expr::Number(0.0, span.clone())),
            right: Box::new(cond),
            span: span.clone(),
        };
        if !self.match_token(TokenType::Then)
            && !self.match_token(TokenType::Do)
            && !self.match_token(TokenType::Colon)
            && !self.check(TokenType::LBrace)
            && !self.check(TokenType::Newline)
        {
            return Err(ParseError::new(self.current_span(), "Expected THEN, DO, :, '{', or newline-indented body after UNLESS condition".to_string()));
        }
        let then_body = self.parse_do_body(&span)?;
        let else_body = if self.match_token(TokenType::Otherwise) {
            let _ = self.match_token(TokenType::Do) || self.match_token(TokenType::Colon);
            Some(self.parse_do_body(&span)?)
        } else { None };
        while self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::If { conditions: vec![not_cond], then_body, else_body, scope_modifier: None, span })
    }

    fn parse_do_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // DO
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
        let mut targets: Vec<Identifier> = Vec::new();
        
        // Check if we have immediate WHILE (old syntax: DO WHILE condition)
        if self.check(TokenType::While) {
            self.advance(); // consume WHILE
            let condition = self.parse_expression(0);
            let body = self.parse_do_body(&start_span)?;
            return Ok(Statement::WhileBlock { targets, alias: None, condition, body, scope_modifier: None, span: start_span });
        }
        
        // Parse optional targets (identifiers before the body)
        if self.check(TokenType::Identifier) && !self.peek_is_assign() {
            loop {
                let t = self.advance();
                targets.push(Identifier::new(t.value.unwrap_or_default(), Span::new(t.line, t.col, t.line, t.col)));
                if self.check(TokenType::Comma) { self.advance(); continue; } else { break; }
            }
        }
        
        let mut alias: Option<Identifier> = None;
        if self.match_token(TokenType::As) {
            if self.check(TokenType::Identifier) {
                let t = self.advance();
                alias = Some(Identifier::new(t.value.unwrap_or_default(), Span::new(t.line, t.col, t.line, t.col)));
            } else {
                return Err(ParseError::new(self.current_span(), "Expected identifier after AS".to_string()));
            }
        }

        // ── FOR clause (repeats or timed) ─────────────────────────────────────
        // Check for `DO [targets] FOR <expr>[ms]` BEFORE trying to parse a body,
        // so that `DO FOR 500ms` isn't mistakenly parsed as an inline FOR statement.
        if self.check(TokenType::For) {
            self.advance(); // consume FOR
            // Use min_prec=1 to suppress implicit juxtaposition so that `500 ms`
            // doesn't get merged into Add(500, ms) before we can check for the
            // `ms` suffix ourselves.
            let expr = self.parse_expression(1);
            let mut repeats: Option<Vec<Expr>> = None;
            let mut duration_ms: Option<Expr> = None;
            if self.check(TokenType::Identifier)
                && self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("ms")).unwrap_or(false)
            {
                self.advance(); // consume `ms`
                duration_ms = Some(expr);
            } else {
                let mut reps = vec![expr];
                while self.check(TokenType::Comma) {
                    self.advance();
                    reps.push(self.parse_expression(0));
                }
                repeats = Some(reps);
            }
            // For timed loops the body follows as an indented block; for count
            // loops with named targets the body is in the targets themselves.
            let body = self.parse_do_body(&start_span)?;
            return Ok(Statement::DoBlock { targets, alias, repeats, duration_ms, body, span: start_span });
        }

        // ── DO body WHILE condition END  (C-style: body parsed before condition)
        // ── DO targets WHILE condition: body  (original: condition first, body after via colon/newline)
        let pre_body = self.parse_do_while_body(&start_span)?;

        if self.match_token(TokenType::While) {
            let condition = self.parse_expression(0);
            // Determine where the body lives:
            //   - If the pre_body was non-empty: C-style (body was before WHILE) → consume optional END
            //   - If a colon or newline follows: original syntax (body after condition)
            //   - Otherwise: require END (bare `DO body WHILE cond END`)
            let final_body = if !pre_body.is_empty() {
                let _ = self.match_token(TokenType::End);
                pre_body
            } else if self.check(TokenType::Colon) || self.check(TokenType::Newline) {
                self.parse_do_body(&start_span)?
            } else {
                let _ = self.match_token(TokenType::End);
                vec![]
            };
            return Ok(Statement::WhileBlock { targets, alias, condition, body: final_body, scope_modifier: None, span: start_span });
        }

        // Plain DO block (no FOR, no WHILE)
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), "Expected END after DO block".to_string()));
        }
        
        Ok(Statement::DoBlock { targets, alias, repeats: None, duration_ms: None, body: pre_body, span: start_span })
    }

    fn parse_do_while_body(&mut self, _start_span: &Span) -> Result<Vec<Statement>, ParseError> {
        // Parse body that ends with WHILE (instead of starting with colon)
        if self.check(TokenType::Newline) {
            self.advance();
            if self.match_token(TokenType::Indent) {
                let mut body = Vec::new();
                while !self.check(TokenType::Dedent) && !self.check(TokenType::While) && !self.is_eof() {
                    if let Some(s) = self.parse_statement() { body.push(s); }
                }
                if self.match_token(TokenType::Dedent) || self.check(TokenType::While) {
                    return Ok(body);
                }
                return Err(ParseError::new(self.current_span(), "Expected DEDENT or WHILE after DO block".to_string()));
            } else {
                return Ok(Vec::new());
            }
        }
        // Inline body: parse until we hit WHILE
        let mut inline = Vec::new();
        while !self.check(TokenType::While) && !self.check(TokenType::Newline) && !self.check(TokenType::Eof) && !self.is_eof() {
            if let Some(s) = self.parse_statement() { inline.push(s); } else { break; }
        }
        Ok(inline)
    }

    fn parse_do_body(&mut self, _start_span: &Span) -> Result<Vec<Statement>, ParseError> {
        if self.check(TokenType::LBrace) {
            return self.parse_brace_body();
        } else if self.match_token(TokenType::Colon) {
            // Skip any blank lines between the colon and the indented body.
            while self.check(TokenType::Newline) { self.advance(); }
            if self.check(TokenType::LBrace) {
                return self.parse_brace_body();
            }
            if self.match_token(TokenType::Indent) {
                let mut body = Vec::new();
                while !self.check(TokenType::Dedent) && !self.is_eof() {
                    // Consume explicit End tokens used to close nested blocks
                    // (e.g. WHILE...END or IF...END written inside this body).
                    // INDENT/DEDENT already bounded those blocks; these End tokens
                    // are cosmetic closers at this indentation level.
                    if self.check(TokenType::End) {
                        self.advance();
                        while self.check(TokenType::Newline) { self.advance(); }
                        continue;
                    }
                    if let Some(s) = self.parse_statement() { body.push(s); }
                }
                if !self.match_token(TokenType::Dedent) {
                    return Err(ParseError::new(self.current_span(), "Expected DEDENT after block".to_string()));
                }
                return Ok(body);
            } else {
                return Ok(Vec::new());
            }
        } else if self.check(TokenType::Newline) {
            // Skip any blank lines between the header and the indented body.
            while self.check(TokenType::Newline) { self.advance(); }
            if self.check(TokenType::LBrace) {
                return self.parse_brace_body();
            }
            if self.match_token(TokenType::Indent) {
                let mut body = Vec::new();
                while !self.check(TokenType::Dedent) && !self.is_eof() {
                    // Consume explicit End tokens used to close nested blocks.
                    if self.check(TokenType::End) {
                        self.advance();
                        while self.check(TokenType::Newline) { self.advance(); }
                        continue;
                    }
                    if let Some(s) = self.parse_statement() { body.push(s); }
                }
                if !self.match_token(TokenType::Dedent) {
                    return Err(ParseError::new(self.current_span(), "Expected DEDENT after block".to_string()));
                }
                return Ok(body);
            } else {
                return Ok(Vec::new());
            }
        }
        // Inline body: `DO print z` — no colon, no newline yet.
        // Parse statements until end-of-line so `a = DO print z` captures print z.
        let mut inline = Vec::new();
        while !self.check(TokenType::Newline) && !self.check(TokenType::Eof) && !self.is_eof() {
            if self.check(TokenType::End) { self.advance(); break; }
            if let Some(s) = self.parse_statement() { inline.push(s); } else { break; }
        }
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(inline)
    }

    fn parse_for_in_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // FOR
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
        // Parse optional scope modifier: FOR(UNBIND_SCOPE) or FOR(BIND_SCOPE)
        let scope_modifier = self.parse_scope_modifier();

        // Expect loop variable identifier
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(
                self.current_span(),
                "Expected identifier after FOR".to_string(),
            ));
        }
        let var_tok = self.advance();
        let var_span = Span::new(var_tok.line, var_tok.col, var_tok.line, var_tok.col);
        let var = Identifier::new(var_tok.value.unwrap_or_default(), var_span);

        // Expect IN keyword
        if !self.match_token(TokenType::In) {
            return Err(ParseError::new(
                self.current_span(),
                "Expected 'IN' after loop variable in FOR loop".to_string(),
            ));
        }

        // Parse iterable expression
        let iterable = self.parse_expression(0);

        // Expect ':' or newline/indent to open body
        let _ = self.match_token(TokenType::Colon);

        // Parse body
        let body = self.parse_do_body(&start_span)?;

        Ok(Statement::ForIn { var, iterable, body, scope_modifier, span: start_span })
    }

    fn parse_module_decl(&mut self) -> Result<Statement, ParseError> {
        // Expect: MOD Name: NEWLINE INDENT { body } DEDENT END
        let mod_tok = self.advance(); // consume 'MOD' identifier
        let start_span = Span::new(mod_tok.line, mod_tok.col, mod_tok.line, mod_tok.col);

        // Expect module name identifier
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected module name after MOD".to_string()));
        }
        let name_tok = self.advance();
        let name = Identifier::new(name_tok.value.unwrap_or_default(), Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col));

        // Expect ':' or DO
        if !self.match_token(TokenType::Colon) && !self.match_token(TokenType::Do) {
            // tolerate missing colon and continue
        }

        // optional newline then INDENT
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented module body".to_string()));
        }

        let mut exports: Vec<Identifier> = Vec::new();
        let mut body: Vec<Statement> = Vec::new();

        loop {
            if self.is_eof() { break; }
            if self.check(TokenType::Newline) { self.advance(); continue; }
            if self.check(TokenType::Dedent) {
                // A blank line can cause DEDENT followed by Newline then INDENT (returning to body
                // indent level). If we see DEDENT -> (Newline)* -> INDENT, it's a blank line —
                // consume and continue. Otherwise it's the real end of the module body.
                self.advance(); // consume DEDENT
                while self.check(TokenType::Newline) { self.advance(); }
                if self.check(TokenType::Indent) {
                    self.advance(); // consume INDENT — blank-line round-trip, stay in body
                    continue;
                }
                // Real end of module body; DEDENT already consumed.
                break;
            }
            if self.check(TokenType::Export) {
                // parse export list: EXPORT name [, name]* NEWLINE
                let _ = self.advance();
                while !self.check(TokenType::Newline) && !self.is_eof() {
                    if self.check(TokenType::Identifier) {
                        let et = self.advance();
                        exports.push(Identifier::new(et.value.unwrap_or_default(), Span::new(et.line, et.col, et.line, et.col)));
                        if self.check(TokenType::Comma) { self.advance(); continue; } else { break; }
                    } else {
                        break;
                    }
                }
                if self.check(TokenType::Newline) { self.advance(); }
                continue;
            }
            // Otherwise parse statements into module body
            if let Some(s) = self.parse_statement() { body.push(s); }
        }
        // DEDENT was already consumed inside the loop when we broke out naturally.
        // Consume any trailing END if present.
        // expect END
        self.match_token(TokenType::End);
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::ModuleDecl { name, exports, body, span: start_span })
    }

    fn parse_from_block(&mut self) -> Result<Statement, ParseError> {
        // FROM: NEWLINE INDENT { module_group } DEDENT END
        let start_tok = self.advance(); // FROM
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
        // accept ':' and optional newline
        if self.check(TokenType::Colon) { self.advance(); }
        if self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), "Expected indented FROM body".to_string()));
        }

        let mut imports: Vec<ModuleImportGroup> = Vec::new();

        while !self.check(TokenType::Dedent) && !self.is_eof() {
            if self.check(TokenType::Newline) { self.advance(); continue; }
            // module name
            if !self.check(TokenType::Identifier) {
                return Err(ParseError::new(self.current_span(), "Expected module name inside FROM block".to_string()));
            }
            let m = self.advance();
            let module_id = Identifier::new(m.value.unwrap_or_default(), Span::new(m.line, m.col, m.line, m.col));
            // optional colon/newline
            if self.check(TokenType::Colon) { self.advance(); }
            if self.check(TokenType::Newline) { self.advance(); }
            // expect INDENT for USE list
            if !self.match_token(TokenType::Indent) {
                return Err(ParseError::new(self.current_span(), "Expected indented USE block for module".to_string()));
            }
            // parse USE: block(s)
            let mut uses: Vec<UseItem> = Vec::new();
            while !self.check(TokenType::Dedent) && !self.is_eof() {
                if self.check(TokenType::Newline) { self.advance(); continue; }
                if self.check(TokenType::Identifier) {
                    // expect USE token then list
                    if let Some(val) = self.peek().value.as_ref() {
                        if val.eq_ignore_ascii_case("use") {
                            // consume 'use' keyword (identifier — no dedicated TokenType::Use)
                            let _ = self.advance();
                            // optional colon after USE
                            if self.check(TokenType::Colon) { self.advance(); }
                            // skip newline after 'use' keyword
                            if self.check(TokenType::Newline) { self.advance(); }
                            // if the name list lives on an indented sub-block, consume INDENT
                            let had_use_indent = self.match_token(TokenType::Indent);
                            // parse name list: continue over newlines/indents, break on DEDENT/END/EOF
                            loop {
                                if self.check(TokenType::Newline) || self.check(TokenType::Indent) {
                                    self.advance(); continue;
                                }
                                if self.check(TokenType::Dedent) || self.check(TokenType::End) || self.is_eof() {
                                    break;
                                }
                                if self.check(TokenType::Identifier) {
                                    let it = self.advance();
                                    let name_span = Span::new(it.line, it.col, it.line, it.col);
                                    let name = Identifier::new(it.value.unwrap_or_default(), name_span.clone());
                                    let mut alias: Option<Identifier> = None;
                                    if self.match_token(TokenType::As) {
                                        if self.check(TokenType::Identifier) {
                                            let at = self.advance();
                                            alias = Some(Identifier::new(at.value.unwrap_or_default(), Span::new(at.line, at.col, at.line, at.col)));
                                        }
                                    }
                                    uses.push(UseItem { name: name.clone(), alias, span: name_span });
                                    // consume optional trailing comma
                                    if self.check(TokenType::Comma) { self.advance(); }
                                } else {
                                    // unknown token inside use list — consume to avoid infinite loop
                                    self.advance();
                                }
                            }
                            // if we consumed an INDENT for the name block, consume matching DEDENT
                            if had_use_indent { self.match_token(TokenType::Dedent); }
                            // consume optional END closing the USE block
                            if self.check(TokenType::End) { self.advance(); }
                            continue;
                        }
                    }
                }
                // fallback: consume token to avoid infinite loop
                self.advance();
            }
            // consume Dedent for module group
            if !self.match_token(TokenType::Dedent) {
                return Err(ParseError::new(self.current_span(), "Expected DEDENT after module import group".to_string()));
            }
            // optional END token
            if self.check(TokenType::End) { self.advance(); }
            imports.push(ModuleImportGroup { module: module_id, uses, span: start_span.clone() });
        }

        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), "Expected DEDENT after FROM block".to_string()));
        }
        self.match_token(TokenType::End);
        while self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::FromBlock { imports, span: start_span })
    }

    fn parse_while_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // WHILE
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);
        // Parse optional scope modifier: WHILE(UNBIND_SCOPE) or WHILE(BIND_SCOPE)
        let scope_modifier = self.parse_scope_modifier();
        let condition = self.parse_expression(0);
        // Accept brace-delimited blocks: while cond { ... }
        if self.check(TokenType::LBrace) {
            let body = self.parse_brace_body()?;
            return Ok(Statement::WhileBlock { targets: vec![], alias: None, condition, body, scope_modifier, span: start_span });
        }
        if self.match_token(TokenType::Colon) || self.match_token(TokenType::Do) || self.check(TokenType::Newline) {
            let body = self.parse_do_body(&start_span)?;
            Ok(Statement::WhileBlock { targets: vec![], alias: None, condition, body, scope_modifier, span: start_span })
        } else {
            Err(ParseError::new(self.current_span(), "Expected ':' or indented body after WHILE".to_string()))
        }
    }

    /// Parse optional `(UNBIND_SCOPE)` or `(BIND_SCOPE)` after WHILE/FOR/IF.
    fn parse_scope_modifier(&mut self) -> Option<ScopeModifier> {
        if !self.check(TokenType::LParen) { return None; }
        // Peek ahead: is this (UNBIND_SCOPE) or (BIND_SCOPE)?
        let ahead = self.tokens.get(self.pos + 1);
        let kw = ahead.and_then(|t| t.value.as_deref()).unwrap_or("");
        if kw.eq_ignore_ascii_case("UNBIND_SCOPE") {
            self.advance(); // '('
            self.advance(); // 'UNBIND_SCOPE'
            self.match_token(TokenType::RParen);
            Some(ScopeModifier::UnbindScope)
        } else if kw.eq_ignore_ascii_case("BIND_SCOPE") {
            self.advance(); // '('
            self.advance(); // 'BIND_SCOPE'
            self.match_token(TokenType::RParen);
            Some(ScopeModifier::BindScope)
        } else {
            None
        }
    }


    /// Parse a brace-delimited block `{ stmt* }` — C/Rust style.
    /// Indent/dedent blocks still work unchanged.
    fn parse_brace_body(&mut self) -> Result<Vec<Statement>, ParseError> {
        self.advance(); // consume `{`
        while self.check(TokenType::Newline) { self.advance(); }
        let mut body = Vec::new();
        loop {
            // consume all layout/whitespace tokens — brace blocks ignore indentation
            loop {
                if self.check(TokenType::Newline)
                    || self.check(TokenType::Indent)
                    || self.check(TokenType::Dedent)
                {
                    self.advance();
                } else {
                    break;
                }
            }
            if self.check(TokenType::RBrace) || self.is_eof() { break; }
            if self.check(TokenType::End) { self.advance(); continue; }
            let before = self.pos;
            if let Some(s) = self.parse_statement() {
                body.push(s);
            } else if self.pos == before {
                // parse_statement returned None without consuming — skip one token
                // to avoid an infinite loop (e.g. on an unexpected Dedent)
                self.advance();
            }
        }
        if !self.match_token(TokenType::RBrace) {
            return Err(ParseError::new(self.current_span(), "Expected '}' to close block".to_string()));
        }
        while self.check(TokenType::Newline) { self.advance(); }
        Ok(body)
    }

    fn parse_print_statement(&mut self) -> Result<Statement, ParseError> {
        let t = self.advance(); // PRINT
        let start_span = Span::new(t.line, t.col, t.line, t.col);
        let first = self.parse_expression(0);
        let expr = if self.check(TokenType::Comma) {
            let mut items = vec![first];
            while self.check(TokenType::Comma) {
                self.advance();
                items.push(self.parse_expression(0));
            }
            let end = items.last().unwrap().span();
            let span = Span::new(start_span.start_line, start_span.start_col, end.end_line, end.end_col);
            Expr::List { items, span }
        } else {
            first
        };
        let print_span = expr.span();
        // Extract identifier before expr is moved into print_stmt,
        // so we can auto-append an increment for `print VAR WHILE cond`.
        let loop_var_id: Option<Identifier> = if let Expr::Identifier(ref id) = expr {
            Some(id.clone())
        } else {
            None
        };
        let print_stmt = Statement::Print { expr, span: print_span.clone() };
        if self.match_token(TokenType::While) {
            let condition = self.parse_expression(0);
            if self.check(TokenType::Newline) { self.advance(); }
            let end_span = self.current_span();
            let while_span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
            let mut body = vec![print_stmt];
            // Auto-append `var = var + 1` when printing a bare identifier.
            if let Some(id) = loop_var_id {
                let var_span = id.span.clone();
                let inc_expr = Expr::Binary {
                    op: BinaryOp::Add,
                    left: Box::new(Expr::Identifier(id.clone())),
                    right: Box::new(Expr::Number(1.0, var_span.clone())),
                    span: var_span.clone(),
                };
                body.push(Statement::Assignment { target: id, value: inc_expr, span: var_span });
            }
            return Ok(Statement::WhileBlock { targets: vec![], alias: None, condition, body, scope_modifier: None, span: while_span });
        }
        if self.match_token(TokenType::For) {
            let count_expr = self.parse_expression(0);
            if self.check(TokenType::Newline) { self.advance(); }
            let end_span = self.current_span();
            let for_span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
            let dummy_target = Identifier::new("_print_for_".to_string(), for_span.clone());
            return Ok(Statement::DoBlock { targets: vec![dummy_target], alias: None, repeats: Some(vec![count_expr]), duration_ms: None, body: vec![print_stmt], span: for_span });
        }
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(print_stmt)
    }

    fn parse_assignment_statement(&mut self) -> Result<Statement, ParseError> {
        let id_tok = self.advance();
        let mut name = id_tok.value.clone().unwrap_or_default();
        let start_span = Span::new(id_tok.line, id_tok.col, id_tok.line, id_tok.col);
        let mut end_span = start_span.clone();
        
        // Support dotted names on the left side: math.pi = 3.14
        while self.check(TokenType::Dot) {
            self.advance(); // consume the dot
            if self.check(TokenType::Identifier) {
                let next_tok = self.advance();
                name.push('.');
                name.push_str(&next_tok.value.clone().unwrap_or_default());
                end_span = Span::new(next_tok.line, next_tok.col, next_tok.line, next_tok.col);
            } else {
                return Err(ParseError::new(self.current_span(), "Expected identifier after '.' in assignment target".to_string()));
            }
        }
        
        let target_span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
        let target = Identifier::new(name, target_span.clone());

        // Multi-label assignment: `a, b, c = expr`
        if self.check(TokenType::Comma) {
            let mut targets = vec![target];
            while self.check(TokenType::Comma) {
                self.advance(); // consume ','
                if !self.check(TokenType::Identifier) {
                    return Err(ParseError::new(self.current_span(), "Expected identifier after ',' in multi-label assignment".to_string()));
                }
                let t = self.advance();
                let ts = Span::new(t.line, t.col, t.line, t.col);
                targets.push(Identifier::new(t.value.unwrap_or_default(), ts));
            }
            if !self.match_token(TokenType::Eq) {
                return Err(ParseError::new(self.current_span(), "Expected '=' in multi-label assignment".to_string()));
            }
            let value = self.parse_expression(0);
            if self.check(TokenType::Newline) { self.advance(); }
            let span = Span::new(start_span.start_line, start_span.start_col, value.span().end_line, value.span().end_col);
            return Ok(Statement::MultiAssignment { targets, value, span });
        }

        if !self.match_token(TokenType::Eq) {
            return Err(ParseError::new(self.current_span(), "Expected '=' in assignment".to_string()));
        }
        // `name = LOOP ... END` — named loop block
        if self.check(TokenType::Loop) {
            let loop_tok = self.advance(); // consume LOOP
            let loop_span = Span::new(loop_tok.line, loop_tok.col, loop_tok.line, loop_tok.col);
            // Do NOT consume the newline — parse_do_body needs it to detect the indented block.
            let body = self.parse_do_body(&loop_span)?;
            return Ok(Statement::LoopBlock { name: target.name.clone(), body, span: target_span });
        }
        if self.check(TokenType::Print) {
            let print_stmt = self.parse_print_statement()?;
            let span = match &print_stmt { Statement::Print { span, .. } => span.clone(), _ => Span::dummy() };
            let lam = Expr::Lambda(vec![print_stmt], span);
            return Ok(Statement::Assignment { target: target.clone(), value: lam, span: target_span });
        } else if self.check(TokenType::Do) {
            let do_stmt = self.parse_do_statement()?;
            let span = match &do_stmt { Statement::WhileBlock { span, .. } => span.clone(), Statement::DoBlock { span, .. } => span.clone(), _ => Span::dummy() };
            let lam = match do_stmt {
                Statement::WhileBlock { .. } => Expr::Lambda(vec![do_stmt.clone()], span),
                Statement::DoBlock { targets, alias: _, repeats: _, duration_ms: _, body, span: _ } if targets.is_empty() => Expr::Lambda(body.clone(), span),
                other => Expr::Lambda(vec![other.clone()], span),
            };
            return Ok(Statement::Assignment { target: target.clone(), value: lam, span: target_span });
        }
        let first = self.parse_expression(0);
        let expr = if self.check(TokenType::Comma) {
            let mut items = vec![first];
            while self.check(TokenType::Comma) {
                self.advance();
                items.push(self.parse_expression(0));
            }
            let start = items.first().unwrap().span();
            let end = items.last().unwrap().span();
            let span = Span::new(start.start_line, start.start_col, end.end_line, end.end_col);
            Expr::List { items, span }
        } else {
            first
        };
        if self.check(TokenType::Semicolon) { self.advance(); } // gfx_patch_assign
        if self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::Assignment { target: target.clone(), value: expr, span: target_span })
    }

    fn parse_priority_override(&mut self) -> Result<Statement, ParseError> {
        let a = self.advance();
        let higher = Identifier::new(a.value.clone().unwrap_or_default(), Span::new(a.line, a.col, a.line, a.col));
        if !self.match_token(TokenType::Over) {
            return Err(ParseError::new(self.current_span(), "Expected OVER in priority override".to_string()));
        }
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected identifier after OVER".to_string()));
        }
        let b = self.advance();
        let lower = Identifier::new(b.value.clone().unwrap_or_default(), Span::new(b.line, b.col, b.line, b.col));
        if self.check(TokenType::Newline) { self.advance(); }
        let span = Span::new(a.line, a.col, b.line, b.col);
        Ok(Statement::PriorityOverride { higher, lower, span })
    }

    /// Parse an ATTEMPT(err_var) / TRY(err_var) block — pasta try/except.
    /// TRY/OTHERWISE exception handling - flexible syntax:
    ///
    /// Block style:
    ///   TRY:
    ///       statements...
    ///   OTHERWISE:
    ///       fallback...
    ///
    /// Inline style:
    ///   TRY: DO risky_func() OTHERWISE: DO safe_fallback()
    ///
    /// With error capture (ATTEMPT form):
    ///   ATTEMPT(err):
    ///       statements...
    ///   OTHERWISE:
    ///       PRINT("Error: " err)
    ///
    /// Nested:
    ///   TRY: DO outer() OTHERWISE: TRY: DO inner() OTHERWISE: DO fallback()
    ///
    fn parse_try_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // consume TRY / ATTEMPT
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Check for ATTEMPT(err_var) form
        let err_var = if self.check(TokenType::LParen) {
            self.advance(); // consume `(`
            let err_tok = self.peek().clone();
            let var = if err_tok.kind == TokenType::Identifier {
                self.advance();
                Some(Identifier::new(
                    err_tok.value.clone().unwrap_or_default(),
                    start_span.clone(),
                ))
            } else {
                return Err(ParseError::new(
                    start_span.clone(),
                    format!("ATTEMPT expects identifier, got {:?}", err_tok.kind),
                ));
            };
            if !self.match_token(TokenType::RParen) {
                return Err(ParseError::new(start_span.clone(), "Expected `)` after ATTEMPT variable"));
            }
            var
        } else {
            None
        };

        // Optional colon after TRY or ATTEMPT(var)
        let _ = self.match_token(TokenType::Colon);

        // Parse the try body - handles both inline and block styles
        let try_body = self.parse_flexible_body(&start_span)?;

        // Skip newlines before looking for OTHERWISE/ELSE
        while self.check(TokenType::Newline) { self.advance(); }

        // Parse OTHERWISE/ELSE clause if present
        let else_body = if self.match_token(TokenType::Otherwise) {
            let _ = self.match_token(TokenType::Colon);
            let body = self.parse_flexible_body(&start_span)?;
            while self.check(TokenType::Newline) { self.advance(); }
            body
        } else {
            vec![]
        };

        // Consume the final END for the whole ATTEMPT/TRY block
        let _ = self.match_token(TokenType::End);
        while self.check(TokenType::Newline) { self.advance(); }

        // Use AttemptBlock if we have an error variable, otherwise emit TryBlock
        if let Some(ev) = err_var {
            Ok(Statement::AttemptBlock {
                err_var: ev,
                try_body,
                else_body,
                span: start_span,
            })
        } else {
            Ok(Statement::TryBlock {
                try_body,
                else_body,
                span: start_span,
            })
        }
    }

    /// Parse a flexible body that works for both inline and block styles:
    /// - Inline: `DO func()` or just `func()` on same line
    /// - Block: newline + indent + statements + dedent
    fn parse_flexible_body(&mut self, _span: &Span) -> Result<Vec<Statement>, ParseError> {
        let mut body = Vec::new();

        // Skip optional DO keyword
        let _ = self.match_token(TokenType::Do);

        // Check what follows
        if self.check(TokenType::LBrace) {
            return self.parse_brace_body();
        } else if self.check(TokenType::Newline) {
            // Block style - skip newlines and look for indent
            while self.check(TokenType::Newline) { self.advance(); }
            if self.check(TokenType::LBrace) {
                return self.parse_brace_body();
            }
            
            if self.match_token(TokenType::Indent) {
                // Parse statements until dedent or OTHERWISE (not END - let nested blocks handle their own ENDs)
                while !self.check(TokenType::Dedent) && !self.is_eof() {
                    // Stop only at OTHERWISE/ELSE - this marks the end of try body
                    if self.check(TokenType::Otherwise) {
                        break;
                    }
                    if let Some(s) = self.parse_statement() {
                        body.push(s);
                    }
                }
                let _ = self.match_token(TokenType::Dedent);
            } else {
                // No indent - parse statements until OTHERWISE or END
                while !self.is_eof() {
                    if self.check(TokenType::Otherwise) || self.check(TokenType::End) {
                        break;
                    }
                    if let Some(s) = self.parse_statement() {
                        body.push(s);
                    }
                    while self.check(TokenType::Newline) { self.advance(); }
                }
            }
        } else if !self.check(TokenType::Otherwise) && !self.check(TokenType::End) && !self.is_eof() {
            // Inline style - parse single statement/expression on same line
            if let Some(s) = self.parse_statement() {
                body.push(s);
            }
        }

        Ok(body)
    }

    fn parse_expr_statement(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expression(0);
        if self.check(TokenType::Semicolon) { self.advance(); }
        if self.check(TokenType::Newline) { self.advance(); }
        let span = expr.span();
        Ok(Statement::ExprStmt { expr, span })
    }

    fn peek_is_assign(&self) -> bool {
        // lookahead for '=' after identifier
        if self.pos + 1 < self.tokens.len() {
            return self.tokens[self.pos + 1].kind == TokenType::Eq;
        }
        false
    }

    /// Returns true if the current position starts a multi-label assignment:
    /// `ident , ident [, ident]* =`
    fn peek_is_multi_assign(&self) -> bool {
        let mut i = self.pos + 1;
        while i < self.tokens.len() {
            match self.tokens[i].kind {
                TokenType::Comma => {
                    i += 1;
                    // skip the next identifier
                    if i < self.tokens.len() && self.tokens[i].kind == TokenType::Identifier {
                        i += 1;
                    } else {
                        return false;
                    }
                }
                TokenType::Eq => return true,
                _ => return false,
            }
        }
        false
    }

    fn peek_is_priority_override(&self) -> bool {
        // lookahead for OVER after identifier
        if self.pos + 1 < self.tokens.len() {
            return self.tokens[self.pos + 1].kind == TokenType::Over;
        }
        false
    }

    // ── Expression parsing (precedence climbing) ──────────────────────────────

    /// Returns true if `kind` can start a primary expression (and thus
    /// participate in implicit juxtaposition / space-concat).
    fn can_start_primary(kind: &TokenType) -> bool {
        matches!(kind,
            TokenType::String
            | TokenType::Number
            | TokenType::Bool
            | TokenType::NoneVal
            | TokenType::Identifier
            | TokenType::LParen
            | TokenType::LBracket
        )
    }

    fn parse_expression(&mut self, min_prec: i32) -> Expr {
        let mut left = self.parse_unary();
        left = self.parse_postfix(left);
        loop {
            let tok = self.peek().clone();
            let prec = self.get_prec(&tok.kind);

            // Implicit juxtaposition / space-concat:
            // "Hello " name  =>  "Hello " + name
            // Only applies at the top level (min_prec == 0) to avoid ambiguity
            // inside higher-precedence subexpressions, and only when the next
            // token can start a primary and is NOT a statement-level keyword.
            if prec == 0 && min_prec == 0 && Self::can_start_primary(&tok.kind)
                && !matches!(tok.kind, TokenType::Newline | TokenType::Eof
                    | TokenType::End | TokenType::Otherwise | TokenType::If
                    | TokenType::While | TokenType::Do | TokenType::Def
                    | TokenType::Print | TokenType::Set)
            {
                let right = self.parse_unary();
                let right = self.parse_postfix(right);
                let span = Span::new(left.span().start_line, left.span().start_col,
                                     right.span().end_line, right.span().end_col);
                left = Expr::Binary { op: BinaryOp::Add, left: Box::new(left), right: Box::new(right), span };
                continue;
            }

            if prec < min_prec || prec == 0 { break; }
            let op_tok = self.advance();
            let op = self.token_to_binop(&op_tok.kind);
            // ^ is right-associative: use same prec as min so right side can include same op
            let next_min = if op_tok.kind == TokenType::Caret { prec } else { prec + 1 };
            let right = self.parse_expression(next_min);
            let span = Span::new(left.span().start_line, left.span().start_col, right.span().end_line, right.span().end_col);
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }
        left
    }

    fn parse_postfix(&mut self, mut expr: Expr) -> Expr {
        loop {
            if self.check(TokenType::LBracket) {
                let bracket_tok = self.advance();
                let mut indices = Vec::new();
                while !self.check(TokenType::RBracket) && !self.is_eof() {
                    indices.push(self.parse_expression(0));
                    if self.check(TokenType::Comma) { self.advance(); } else { break; }
                }
                let end_tok = if self.check(TokenType::RBracket) { self.advance() } else { Token::new(TokenType::RBracket, None, bracket_tok.line, bracket_tok.col) };
                let span = Span::new(expr.span().start_line, expr.span().start_col, end_tok.line, end_tok.col);
                expr = Expr::Index { base: Box::new(expr), indices, span };
            } else {
                break;
            }
        }
        expr
    }

    fn skip_layout_tokens(&mut self) {
        while matches!(self.peek().kind, TokenType::Newline | TokenType::Indent | TokenType::Dedent) {
            self.advance();
        }
    }

    fn parse_unary(&mut self) -> Expr {
        if self.match_token(TokenType::Minus) {
            let rhs = self.parse_unary();
            let zero = Expr::Number(0.0, rhs.span());
            let span = rhs.span();
            Expr::Binary { op: BinaryOp::Sub, left: Box::new(zero), right: Box::new(rhs), span }
        } else if self.match_token(TokenType::Not) {
            let rhs = self.parse_unary();
            let zero = Expr::Number(0.0, rhs.span());
            let span = rhs.span();
            Expr::Binary { op: BinaryOp::Not, left: Box::new(zero), right: Box::new(rhs), span }
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Expr {
        let tok = self.peek().clone();
        match tok.kind {
            TokenType::Number => {
                let t = self.advance();
                let mut text = t.value.clone().unwrap_or_default();
                text.retain(|c| c != '_');
                let n = text.parse::<f64>().unwrap_or(0.0);
                Expr::Number(n, Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::String => {
                let t = self.advance();
                Expr::String(t.value.clone().unwrap_or_default(), Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::Bool => {
                let t = self.advance();
                let b = t.value.as_ref().map(|s| s.eq_ignore_ascii_case("true")).unwrap_or(false);
                Expr::Bool(b, Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::NoneVal => {
                let t = self.advance();
                Expr::None(Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::Identifier | TokenType::Class => {
                // ── lambda expression: lambda param1, param2: expr ────────────
                if self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("lambda")).unwrap_or(false) {
                    let lam_tok = self.advance(); // consume "lambda"
                    let lam_span = Span::new(lam_tok.line, lam_tok.col, lam_tok.line, lam_tok.col);
                    // collect param names until ':'
                    let mut params: Vec<Identifier> = Vec::new();
                    while !self.check(TokenType::Colon) && !self.is_eof() && !self.check(TokenType::Newline) {
                        if self.check(TokenType::Identifier) {
                            let p = self.advance();
                            params.push(Identifier::new(p.value.unwrap_or_default(), Span::new(p.line, p.col, p.line, p.col)));
                        } else if self.check(TokenType::Comma) {
                            self.advance(); // skip comma between params
                        } else {
                            break;
                        }
                    }
                    if self.check(TokenType::Colon) { self.advance(); } // consume ':'
                    // parse body expression
                    let body_expr = self.parse_expression(0);
                    // Build an assignment for each param so the lambda body can reference them,
                    // then return the expression. We store the lambda as a DEF-style body that
                    // first binds param names from a magic __args__ list injected at call time.
                    // For now: build a minimal lambda that uses the param names as globals
                    // (same pattern as DEF functions — caller sets globals before invoking).
                    // The body is: [ExprStmt(body_expr)] wrapped in a Lambda.
                    let body_stmt = Statement::ExprStmt { expr: body_expr.clone(), span: lam_span.clone() };
                    // Wrap param assignments: each param = __arg_N__ (resolved at call site)
                    let mut stmts: Vec<Statement> = params.iter().enumerate().map(|(i, p): (usize, &Identifier)| {
                        Statement::Assignment {
                            target: p.clone(),
                            value: Expr::Identifier(Identifier::new(format!("__arg_{}__", i), lam_span.clone())),
                            span: lam_span.clone(),
                        }
                    }).collect();
                    stmts.push(body_stmt);
                    return Expr::Lambda(stmts, lam_span);
                }
                // ─────────────────────────────────────────────────────────────
                let id_tok = self.advance();
                let mut id_name = id_tok.value.clone().unwrap_or_default();
                let id_span = Span::new(id_tok.line, id_tok.col, id_tok.line, id_tok.col);
                // Consume dotted segments: tensor.zeros(...) -> id_name = "tensor.zeros"
                while self.check(TokenType::Dot) {
                    self.advance(); // consume '.'
                    if self.check(TokenType::Identifier) {
                        let seg = self.advance();
                        id_name.push('.');
                        id_name.push_str(&seg.value.unwrap_or_default());
                    }
                }
                let id = Identifier::new(id_name.clone(), id_span);
                // tensor{[r1,c1,...], [r2,...]} — brace tensor literal
                if id_name.eq_ignore_ascii_case("tensor") && self.check(TokenType::LBrace) {
                    let brace_tok = self.advance(); // consume '{'
                    let brace_span = Span::new(brace_tok.line, brace_tok.col, brace_tok.line, brace_tok.col);
                    let mut rows = Vec::new();
                    while !self.check(TokenType::RBrace) && !self.is_eof() {
                        if self.check(TokenType::Newline) { self.advance(); continue; }
                        rows.push(self.parse_expression(0));
                        if self.check(TokenType::Comma) { self.advance(); }
                    }
                    let close = self.advance(); // consume '}'
                    let span = Span::new(brace_span.start_line, brace_span.start_col, close.line, close.col);
                    let inner = Expr::List { items: rows, span: span.clone() };
                    return Expr::TensorBuilder { expr: Box::new(inner), span };
                }
                if self.check(TokenType::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    self.skip_layout_tokens();
                    while !self.check(TokenType::RParen) && !self.is_eof() {
                        self.skip_layout_tokens();
                        args.push(self.parse_expression(0));
                        self.skip_layout_tokens();
                        if self.check(TokenType::Comma) {
                            self.advance();
                            self.skip_layout_tokens();
                        } else {
                            break;
                        }
                    }
                    if self.check(TokenType::RParen) {
                        let r = self.advance();
                        let span = Span::new(id.span.start_line, id.span.start_col, r.line, r.col);
                        // emit as a plain Call — executor dispatches to builtin or user fn
                        return Expr::Call { callee: Box::new(Expr::Identifier(id)), args, span };
                    }
                }
                Expr::Identifier(id)
            }
            TokenType::Obj => {
                // OBJ.GROUP[.MUT](parentA, parentB) — family node expression
                let obj_tok = self.advance(); // OBJ
                let span = Span::new(obj_tok.line, obj_tok.col, obj_tok.line, obj_tok.col);
                if !self.match_token(TokenType::Dot) {
                    return Expr::Raw("OBJ missing '.'".to_string(), span);
                }
                if !self.check(TokenType::Identifier) {
                    return Expr::Raw("OBJ missing group name".to_string(), span);
                }
                let grp_tok = self.advance();
                let group = grp_tok.value.unwrap_or_default().to_uppercase();
                // Check for .MUT or direct (
                let mutable = if self.check(TokenType::Dot) {
                    let saved = self.pos;
                    self.advance(); // consume dot
                    if self.check(TokenType::Identifier)
                        && self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("mut")).unwrap_or(false)
                    {
                        self.advance(); // consume MUT
                        true
                    } else {
                        self.pos = saved; // backtrack
                        false
                    }
                } else { false };
                if !self.match_token(TokenType::LParen) {
                    return Expr::Raw("OBJ missing '('".to_string(), span);
                }
                let parent_a = self.parse_expression(0);
                if self.check(TokenType::Comma) { self.advance(); }
                let parent_b = self.parse_expression(0);
                if self.check(TokenType::RParen) { self.advance(); }
                Expr::ObjFamNew { group, mutable, parent_a: Box::new(parent_a), parent_b: Box::new(parent_b), span }
            }
            TokenType::Typeof => {
                // TYPEOF x  or  TYPEOF(x)  → call to built-in "type"
                let t = self.advance();
                let span = Span::new(t.line, t.col, t.line, t.col);
                let target = if self.check(TokenType::LParen) {
                    self.advance();
                    let e = self.parse_expression(0);
                    if self.check(TokenType::RParen) { self.advance(); }
                    e
                } else {
                    self.parse_unary()
                };
                let func_id = Identifier::new("type".to_string(), span.clone());
                Expr::Call { callee: Box::new(Expr::Identifier(func_id)), args: vec![target], span }
            }
            TokenType::DoesParentExist => {
                let dpe_tok = self.advance(); // DOES_PARENT_EXIST
                let span = Span::new(dpe_tok.line, dpe_tok.col, dpe_tok.line, dpe_tok.col);
                let target = self.parse_unary();
                Expr::DoesParentExist { target: Box::new(target), span }
            }
            TokenType::LParen => {
                self.advance();
                let e = self.parse_expression(0);
                if self.check(TokenType::RParen) { self.advance(); }
                e
            }
            TokenType::Ref => {
                // REF.<KIND>(target) WITH { metadata }
                let ref_tok = self.advance(); // REF
                let ref_span = Span::new(ref_tok.line, ref_tok.col, ref_tok.line, ref_tok.col);

                // Expect '.' then KIND
                if !self.match_token(TokenType::Dot) {
                    return Expr::Raw("REF missing '.'".to_string(), ref_span);
                }

                if !self.check(TokenType::Identifier) {
                    return Expr::Raw("REF missing kind".to_string(), ref_span);
                }
                let kind_tok = self.advance();
                let kind = kind_tok.value.unwrap_or_default().to_uppercase();

                // Expect '(' target ')'
                if !self.match_token(TokenType::LParen) {
                    return Expr::Raw("REF missing '('".to_string(), ref_span);
                }

                let target = Box::new(self.parse_expression(0));

                if !self.match_token(TokenType::RParen) {
                    return Expr::Raw("REF missing ')'".to_string(), ref_span);
                }

                // Optional WITH { metadata }
                let mut metadata = Vec::new();
                if self.check(TokenType::Identifier) && self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("with")).unwrap_or(false) {
                    self.advance(); // WITH
                    if self.match_token(TokenType::LBrace) {
                        while !self.check(TokenType::RBrace) && !self.is_eof() {
                            if self.check(TokenType::Newline) { self.advance(); continue; }
                            if !self.check(TokenType::Identifier) { break; }
                            let key_tok = self.advance();
                            let key = key_tok.value.unwrap_or_default();
                            if !self.match_token(TokenType::Colon) && !self.match_token(TokenType::Eq) {
                                break;
                            }
                            let value = self.parse_expression(0);
                            metadata.push((key, value));
                            if self.check(TokenType::Comma) { self.advance(); }
                        }
                        self.match_token(TokenType::RBrace);
                    }
                }

                Expr::Ref { kind, target, metadata, span: ref_span }
            }
            TokenType::LBracket => {
                // list literal
                self.advance();
                let mut items = Vec::new();
                self.skip_layout_tokens();
                while !self.check(TokenType::RBracket) && !self.is_eof() {
                    self.skip_layout_tokens();
                    items.push(self.parse_expression(0));
                    self.skip_layout_tokens();
                    if self.check(TokenType::Comma) {
                        self.advance();
                        self.skip_layout_tokens();
                    } else {
                        break;
                    }
                }
                if self.check(TokenType::RBracket) { let r = self.advance(); let span = Span::new(items.first().map(|i: &Expr| i.span().start_line).unwrap_or(0), 0, r.line, r.col); return Expr::List { items, span }; }
                Expr::List { items, span: Span::dummy() }
            }
            TokenType::LBrace => {
                // dict literal: {"key": value, ...}
                let open = self.advance(); // consume '{'
                let span = Span::new(open.line, open.col, open.line, open.col);
                let mut pairs = Vec::new();
                self.skip_layout_tokens();
                while !self.check(TokenType::RBrace) && !self.is_eof() {
                    self.skip_layout_tokens();
                    let key = self.parse_expression(0);
                    self.skip_layout_tokens();
                    if !self.match_token(TokenType::Colon) { break; }
                    self.skip_layout_tokens();
                    let val = self.parse_expression(0);
                    pairs.push((key, val));
                    self.skip_layout_tokens();
                    if self.check(TokenType::Comma) {
                        self.advance();
                        self.skip_layout_tokens();
                    }
                }
                self.match_token(TokenType::RBrace);
                Expr::Dict { pairs, span }
            }
            _ => {
                // fallback: consume token as raw
                let t = self.advance();
                Expr::Raw(t.value.unwrap_or_default(), Span::new(t.line, t.col, t.line, t.col))
            }
        }
    }

    // ═══════════════════════════════════════════════════════════════════════════
    // v1.4.4 POINTER STATEMENT PARSING
    // ═══════════════════════════════════════════════════════════════════════════

    /// Parse `GOTO <label>` or `GOTO <ptr>: ... END` (pointer context block).
    fn parse_goto_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // consume GOTO
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Expect an identifier for the loop label or pointer name
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected label or pointer name after GOTO".to_string()));
        }
        let label_tok = self.advance();
        let name = label_tok.value.clone().unwrap_or_default();

        // If followed by ':', this is a pointer-context block: GOTO ptr: ... END
        if self.check(TokenType::Colon) {
            let body = self.parse_do_body(&start_span)?;
            return Ok(Statement::GotoBlock { name, body, span: start_span });
        }

        while self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::GotoLabel { label: name, span: start_span })
    }

    /// Parse PULL.<TYPE> [ptr] -> target
    fn parse_pull_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // PULL
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Expect '.' then TYPE
        if !self.match_token(TokenType::Dot) {
            return Err(ParseError::new(self.current_span(), "Expected '.' after PULL".to_string()));
        }

        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected data type after PULL.".to_string()));
        }
        let dtype_tok = self.advance();
        let dtype = dtype_tok.value.unwrap_or_default().to_uppercase();

        // Optional explicit pointer: `PULL.BYTE ptr -> x` or `PULL.BYTE ptr` (no target)
        // Heuristic: if current token is an identifier AND next is `->` or end-of-line,
        // treat the identifier as the explicit pointer (not part of the value expression).
        let explicit_ptr: Option<Box<Expr>> = if self.check(TokenType::Identifier) {
            let next_is_ptr_sentinel = self.tokens.get(self.pos + 1).map(|t| {
                t.kind == TokenType::Arrow || t.kind == TokenType::Newline || t.kind == TokenType::Eof
            }).unwrap_or(true);
            if next_is_ptr_sentinel {
                Some(Box::new(self.parse_expression(0)))
            } else {
                None
            }
        } else {
            None
        };

        // Optional args in parentheses (kept for backward compat; currently unused)
        let mut args = Vec::new();
        if self.check(TokenType::LParen) {
            self.advance(); // '('
            while !self.check(TokenType::RParen) && !self.is_eof() {
                args.push(self.parse_expression(0));
                if self.check(TokenType::Comma) { self.advance(); } else { break; }
            }
            if !self.match_token(TokenType::RParen) {
                return Err(ParseError::new(self.current_span(), "Expected ')' after PULL arguments".to_string()));
            }
        }

        // Optional -> target
        let target = if self.match_token(TokenType::Arrow) {
            if !self.check(TokenType::Identifier) {
                return Err(ParseError::new(self.current_span(), "Expected identifier after '->'".to_string()));
            }
            let tgt_tok = self.advance();
            let tgt_span = Span::new(tgt_tok.line, tgt_tok.col, tgt_tok.line, tgt_tok.col);
            Some(Identifier::new(tgt_tok.value.unwrap_or_default(), tgt_span))
        } else {
            None
        };

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Pull { dtype, explicit_ptr, args, target, span: start_span })
    }

    /// Parse PUSH.<TYPE> [ptr,] <value>
    fn parse_push_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // PUSH
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Expect '.' then TYPE
        if !self.match_token(TokenType::Dot) {
            return Err(ParseError::new(self.current_span(), "Expected '.' after PUSH".to_string()));
        }

        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected data type after PUSH.".to_string()));
        }
        let dtype_tok = self.advance();
        let dtype = dtype_tok.value.unwrap_or_default().to_uppercase();

        // Optional args in parentheses (kept for backward compat; currently unused)
        let mut args = Vec::new();
        if self.check(TokenType::LParen) {
            self.advance(); // '('
            while !self.check(TokenType::RParen) && !self.is_eof() {
                args.push(self.parse_expression(0));
                if self.check(TokenType::Comma) { self.advance(); } else { break; }
            }
            if !self.match_token(TokenType::RParen) {
                return Err(ParseError::new(self.current_span(), "Expected ')' after PUSH arguments".to_string()));
            }
        }

        // Parse the first expression (may be `ptr` if followed by `,`, or the value)
        let first = self.parse_expression(0);

        // If followed by `,`, the first expr was the explicit pointer; parse the actual value
        let (explicit_ptr, value) = if self.check(TokenType::Comma) {
            self.advance(); // consume ','
            let val = self.parse_expression(0);
            (Some(Box::new(first)), val)
        } else {
            (None, first)
        };

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Push { dtype, explicit_ptr, value, args, span: start_span })
    }

    /// Parse <var> = ALLOC.<KIND>(args) WITH { metadata }
    /// Note: This is called after recognizing ALLOC token at statement level
    fn parse_alloc_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // ALLOC
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Expect '.' then KIND
        if !self.match_token(TokenType::Dot) {
            return Err(ParseError::new(self.current_span(), "Expected '.' after ALLOC".to_string()));
        }

        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected pointer kind after ALLOC.".to_string()));
        }
        let kind_tok = self.advance();
        let kind = kind_tok.value.unwrap_or_default().to_uppercase();

        // Optional args in parentheses
        let mut args = Vec::new();
        if self.check(TokenType::LParen) {
            self.advance(); // '('
            while !self.check(TokenType::RParen) && !self.is_eof() {
                args.push(self.parse_expression(0));
                if self.check(TokenType::Comma) { self.advance(); } else { break; }
            }
            if !self.match_token(TokenType::RParen) {
                return Err(ParseError::new(self.current_span(), "Expected ')' after ALLOC arguments".to_string()));
            }
        }

        // Optional WITH { metadata }
        let mut metadata = Vec::new();
        if self.check(TokenType::Identifier) && self.peek().value.as_deref().map(|s| s.eq_ignore_ascii_case("with")).unwrap_or(false) {
            self.advance(); // WITH
            if self.match_token(TokenType::LBrace) {
                while !self.check(TokenType::RBrace) && !self.is_eof() {
                    if self.check(TokenType::Newline) { self.advance(); continue; }
                    if !self.check(TokenType::Identifier) { break; }
                    let key_tok = self.advance();
                    let key = key_tok.value.unwrap_or_default();
                    if !self.match_token(TokenType::Colon) && !self.match_token(TokenType::Eq) {
                        return Err(ParseError::new(self.current_span(), "Expected ':' or '=' after metadata key".to_string()));
                    }
                    let value = self.parse_expression(0);
                    metadata.push((key, value));
                    if self.check(TokenType::Comma) { self.advance(); }
                }
                self.match_token(TokenType::RBrace);
            }
        }

        // Expect -> target
        if !self.match_token(TokenType::Arrow) {
            return Err(ParseError::new(self.current_span(), "Expected '->' after ALLOC to specify target variable".to_string()));
        }

        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected identifier after '->'".to_string()));
        }
        let target_tok = self.advance();
        let target_span = Span::new(target_tok.line, target_tok.col, target_tok.line, target_tok.col);
        let target = Identifier::new(target_tok.value.unwrap_or_default(), target_span);

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Alloc { target, kind, args, metadata, span: start_span })
    }

    /// Parse FREE <pointer_expr>
    fn parse_free_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // FREE
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Parse pointer expression
        let pointer_expr = self.parse_expression(0);

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Free { pointer_expr, span: start_span })
    }

    /// Parse INFO <pointer_expr> -> target
    fn parse_info_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // INFO
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Parse pointer expression
        let pointer_expr = self.parse_expression(0);

        // Optional -> target
        let target = if self.match_token(TokenType::Arrow) {
            if !self.check(TokenType::Identifier) {
                return Err(ParseError::new(self.current_span(), "Expected identifier after '->'".to_string()));
            }
            let tgt_tok = self.advance();
            let tgt_span = Span::new(tgt_tok.line, tgt_tok.col, tgt_tok.line, tgt_tok.col);
            Some(Identifier::new(tgt_tok.value.unwrap_or_default(), tgt_span))
        } else {
            None
        };

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Info { pointer_expr, target, span: start_span })
    }

    /// Parse SEEK <pointer_expr>, <offset_expr>
    fn parse_seek_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // SEEK
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Parse pointer expression
        let pointer_expr = self.parse_expression(0);

        // Expect comma
        if !self.match_token(TokenType::Comma) {
            return Err(ParseError::new(self.current_span(), "Expected ',' after pointer in SEEK".to_string()));
        }

        // Parse offset expression
        let offset_expr = self.parse_expression(0);

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Seek { pointer_expr, offset_expr, span: start_span })
    }

    /// Parse SWAP <var1>, <var2>
    fn parse_swap_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // SWAP
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Parse first variable name
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected identifier after SWAP".to_string()));
        }
        let var1_tok = self.advance();
        let var1_span = Span::new(var1_tok.line, var1_tok.col, var1_tok.line, var1_tok.col);
        let var1 = Identifier::new(var1_tok.value.unwrap_or_default(), var1_span);

        // Expect comma
        if !self.match_token(TokenType::Comma) {
            return Err(ParseError::new(self.current_span(), "Expected ',' between variables in SWAP".to_string()));
        }

        // Parse second variable name
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected identifier after ',' in SWAP".to_string()));
        }
        let var2_tok = self.advance();
        let var2_span = Span::new(var2_tok.line, var2_tok.col, var2_tok.line, var2_tok.col);
        let var2 = Identifier::new(var2_tok.value.unwrap_or_default(), var2_span);

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::Swap { var1, var2, span: start_span })
    }
}
