// Access the VERBOSE_DEBUG flag from main
pub fn is_verbose_debug() -> bool {
    use std::sync::atomic::{AtomicBool, Ordering};
    extern "Rust" {
        static VERBOSE_DEBUG: AtomicBool;
    }
    unsafe { VERBOSE_DEBUG.load(Ordering::Relaxed) }
}
use std::sync::atomic::Ordering;
// use crate::bin::pasta::VERBOSE_DEBUG; // Disabled to fix import error
// src/parser/parser.rs
//
// Robust recursive-descent / precedence-climbing parser for PASTA.
//
// Features:
// - Handles `DO` statements with multiple targets: `DO a, b FOR 3, n:`
// - Handles `DO` statements with WHILE condition: `DO x WHILE <expr>:`
// - Assignment RHS that begins with `PRINT` or `DO` is captured as a `Lambda`
// - Assignment RHS that is a comma-separated list is captured as `Expr::List`
// - Precedence-climbing expression parser for binary operators.
// - Minimal recovery: on parse error we skip to next NEWLINE or DEDENT.
// - ≈ ≠ ≡ operators registered in precedence table and token_to_binop.
// - @ (matmul) operator registered in precedence table and token_to_binop.
// - Subscript indexing: `expr[i]` and `expr[i, j]` → Expr::Index.

use std::collections::HashMap;

use crate::lexer::{Token, TokenType};
use crate::parser::ast::*;
// bring in a few common error message templates; the parser will supply
// specific context when emitting diagnostics.
use crate::interpreter::errors::messages as err_msg;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl ParseError {
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
            pub fn debug_trace(&self, msg: &str) {
                // VERBOSE_DEBUG removed for build compatibility
            }
        pub fn debug_print(&self) {
            if is_verbose_debug() {
                println!("[DEBUG] Parser state:");
                println!("  Current token: {:?}", self.current_span());
                println!("  Diagnostics: {:?}", self.diagnostics);
            }
        }
    /// Create a new parser from a token stream.
    pub fn new(tokens: Vec<Token>) -> Self {
        let mut prec = HashMap::new();
        // precedence: higher number => binds tighter
        prec.insert(TokenType::At,       45); // matmul: tightest binary, above * /
        prec.insert(TokenType::Star,     40);
        prec.insert(TokenType::Slash,    40);
        prec.insert(TokenType::Plus,     30);
        prec.insert(TokenType::Minus,    30);
        prec.insert(TokenType::EqEq,     20);
        prec.insert(TokenType::Neq,      20);
        prec.insert(TokenType::Lt,       20);
        prec.insert(TokenType::Gt,       20);
        prec.insert(TokenType::Lte,      20);
        prec.insert(TokenType::Gte,      20);
        prec.insert(TokenType::Approx,   20); // ≈ same level as ==
        prec.insert(TokenType::NotEq,    20); // ≠ same level as !=
        prec.insert(TokenType::StrictEq, 20); // ≡ same level as ==
        prec.insert(TokenType::And,      10);
        prec.insert(TokenType::Or,        5); // Or is lower than And

        Parser {
            tokens,
            pos: 0,
            prec,
            diagnostics: Vec::new(),
            eof: Token::new(TokenType::Eof, None, 0, 0),
        }
    }


    /// Parse and return a Program. Diagnostics are recorded internally.
    pub fn parse(&mut self) -> Program {
        let (p, _diags) = self.parse_with_diagnostics();
        p
    }

    /// Parse and return Program plus diagnostics.
    pub fn parse_with_diagnostics(&mut self) -> (Program, Vec<ParseError>) {
        let mut stmts = Vec::new();
        stmts.reserve(16);
        while !self.is_eof() {
            if let Some(s) = self.parse_statement() {
                stmts.push(s);
            }
        }
        let diags = std::mem::take(&mut self.diagnostics);
        (Program::new(stmts), diags)
    }

    // ── Statement parsing ────────────────────────────────────────────────────

    fn parse_statement(&mut self) -> Option<Statement> {
        // Skip any leading newlines
        while self.check(TokenType::Newline) {
            self.advance();
        }
        if self.is_eof() {
            return None;
        }

        let tok = self.peek();
        match tok.kind {
            TokenType::Def => {
                match self.parse_def_statement() {
                    Ok(s) => Some(s),
                    Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                }
            }
            TokenType::If => {
                match self.parse_if_statement() {
                    Ok(s) => Some(s),
                    Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                }
            }
            TokenType::Do => {
                match self.parse_do_statement() {
                    Ok(s) => Some(s),
                    Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                }
            }
            TokenType::Print => {
                match self.parse_print_statement() {
                    Ok(s) => Some(s),
                    Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                }
            }
            TokenType::End => {
                // END without a matching block is a standalone token - skip it
                self.advance();
                while self.check(TokenType::Newline) { self.advance(); }
                None
            }
            TokenType::Identifier => {
                if self.peek_is_assign() {
                    match self.parse_assignment_statement() {
                        Ok(s) => Some(s),
                        Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                    }
                } else if self.peek_is_priority_override() {
                    match self.parse_priority_override() {
                        Ok(s) => Some(s),
                        Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                    }
                } else {
                    match self.parse_expr_statement() {
                        Ok(s) => Some(s),
                        Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                    }
                }
            }
            TokenType::Newline => {
                self.advance();
                None
            }
            _ => {
                match self.parse_expr_statement() {
                    Ok(s) => Some(s),
                    Err(e) => { self.diagnostics.push(e); self.recover_to_next_statement(); None }
                }
            }
        }
    }

    // ── DEF / Function Definition parsing ────────────────────────────────────

    fn parse_def_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance();
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_IDENTIFIER_AFTER_DEF));
        }
        let name_tok = self.advance();
        let name = Identifier::new(
            name_tok.value.unwrap_or_else(|| "unknown".to_string()),
            Span::new(name_tok.line, name_tok.col, name_tok.line, name_tok.col),
        );

        if !self.match_token(TokenType::Do) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_DO_AFTER_DEF));
        }

        while self.check(TokenType::Newline) { self.advance(); }
        if !self.match_token(TokenType::Indent) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_INDENT_AFTER_DEF));
        }

        let mut body = Vec::new();
        body.reserve(8);
        while !self.check(TokenType::Dedent) && !self.is_eof() {
            if let Some(stmt) = self.parse_statement() { body.push(stmt); }
        }

        if !self.match_token(TokenType::Dedent) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_DEDENT_AFTER_DEF));
        }
        if !self.match_token(TokenType::End) {
            return Err(ParseError::new(self.current_span(), err_msg::EXPECTED_END_AFTER_DEF));
        }

        while self.check(TokenType::Newline) { self.advance(); }

        Ok(Statement::FunctionDef { name, body, span: start_span })
    }

    // ── IF / ELSE parsing ────────────────────────────────────────────────────

    /// Parse an `IF` statement. Supports OR-chained conditions and optional ELSE.
    ///
    /// Syntax: IF <expr> OR <expr> OR ... DO: <body> [OTHERWISE DO: <body>]
    /// NOTE: We parse conditions at precedence 6 to stop at OR (which has precedence 5),
    ///       allowing OR to act as a statement-level separator, not just a binary op.
    fn parse_if_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance();
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        // Parse the first condition at precedence 6 to exclude OR (precedence 5)
        let mut conditions = vec![self.parse_expression(6)];
        conditions.reserve(4);

        // Parse additional OR-chained conditions
        while self.match_token(TokenType::Or) {
            conditions.push(self.parse_expression(6));
        }

        // Expect DO keyword
        if !self.match_token(TokenType::Do) {
            return Err(ParseError::new(
                self.current_span(),
                "Expected DO after IF condition(s)".to_string(),
            ));
        }

        let then_body = self.parse_do_body(&start_span)?;

        // Optional OTHERWISE (ELSE) clause
        let else_body = if self.match_token(TokenType::Otherwise) {
            if !self.match_token(TokenType::Do) {
                return Err(ParseError::new(
                    self.current_span(),
                    "Expected DO after OTHERWISE/ELSE".to_string(),
                ));
            }
            Some(self.parse_do_body(&start_span)?)
        } else {
            None
        };

        while self.check(TokenType::Newline) { self.advance(); }

        let end_span = self.current_span();
        let span = Span::new(
            start_span.start_line, start_span.start_col,
            end_span.end_line, end_span.end_col,
        );

        Ok(Statement::If { conditions, then_body, else_body, span })
    }

    // ── DO / WHILE parsing ───────────────────────────────────────────────────

    fn parse_do_statement(&mut self) -> Result<Statement, ParseError> {
        let start_tok = self.advance(); // consume DO
        let start_span = Span::new(start_tok.line, start_tok.col, start_tok.line, start_tok.col);

        let mut targets: Vec<Identifier> = Vec::new();
        targets.reserve(4);
        if self.check(TokenType::Identifier) {
            loop {
                let t = self.advance();
                let id = Identifier::new(
                    t.value.clone().unwrap_or_default(),
                    Span::new(t.line, t.col, t.line, t.col),
                );
                targets.push(id);
                if self.check(TokenType::Comma) {
                    self.advance();
                    continue;
                } else {
                    break;
                }
            }
        }

        let mut alias: Option<Identifier> = None;
        if self.match_token(TokenType::As) {
            if self.check(TokenType::Identifier) {
                let t = self.advance();
                alias = Some(Identifier::new(
                    t.value.clone().unwrap_or_default(),
                    Span::new(t.line, t.col, t.line, t.col),
                ));
            } else {
                return Err(ParseError::new(self.current_span(), "Expected identifier after AS".to_string()));
            }
        }

        let body_stmt_opt: Option<Statement> = if targets.is_empty() && self.check(TokenType::Print) {
            Some(self.parse_print_statement()?)
        } else {
            None
        };

        if self.match_token(TokenType::While) {
            let condition = self.parse_expression(0);
            let body = if let Some(print_stmt) = body_stmt_opt {
                vec![print_stmt]
            } else {
                self.parse_do_body(&start_span)?
            };
            let end_span = self.current_span();
            let span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
            return Ok(Statement::WhileBlock { targets, alias, condition, body, span });
        }

        if let Some(print_stmt) = body_stmt_opt {
            let end_span = self.current_span();
            let full_span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
            return Ok(Statement::DoBlock {
                targets: vec![], alias: None, repeats: None,
                body: vec![print_stmt], span: full_span,
            });
        }

        let mut repeats: Option<Vec<Expr>> = None;
        if self.match_token(TokenType::For) {
            let mut reps: Vec<Expr> = Vec::new();
            reps.reserve(4);
            loop {
                reps.push(self.parse_expression(0));
                if self.check(TokenType::Comma) { self.advance(); continue; } else { break; }
            }
            repeats = Some(reps);
        }

        let body = self.parse_do_body(&start_span)?;
        let end_span = self.current_span();
        let span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
        Ok(Statement::DoBlock { targets, alias, repeats, body, span })
    }

    fn parse_do_body(&mut self, _start_span: &Span) -> Result<Vec<Statement>, ParseError> {
        if self.match_token(TokenType::Colon) {
            if self.check(TokenType::Newline) { self.advance(); }
            if self.match_token(TokenType::Indent) {
                let mut body: Vec<Statement> = Vec::new();
                body.reserve(8);
                while !self.check(TokenType::Dedent) && !self.is_eof() {
                    if let Some(s) = self.parse_statement() { body.push(s); }
                }
                if self.match_token(TokenType::Dedent) {
                    return Ok(body);
                } else {
                    return Err(ParseError::new(self.current_span(), "Expected DEDENT after DO/WHILE body".to_string()));
                }
            } else {
                return Ok(Vec::new());
            }
        }
        Ok(Vec::new())
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
        let print_stmt = Statement::Print { expr, span: print_span.clone() };

        if self.match_token(TokenType::While) {
            let condition = self.parse_expression(0);
            if self.check(TokenType::Newline) { self.advance(); }
            let end_span = self.current_span();
            let while_span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
            return Ok(Statement::WhileBlock {
                targets: vec![], alias: None, condition,
                body: vec![print_stmt], span: while_span,
            });
        }

        if self.match_token(TokenType::For) {
            let count_expr = self.parse_expression(0);
            if self.check(TokenType::Newline) { self.advance(); }
            let end_span = self.current_span();
            let for_span = Span::new(start_span.start_line, start_span.start_col, end_span.end_line, end_span.end_col);
            let dummy_target = Identifier::new("_print_for_".to_string(), for_span.clone());
            return Ok(Statement::DoBlock {
                targets: vec![dummy_target], alias: None,
                repeats: Some(vec![count_expr]),
                body: vec![print_stmt], span: for_span,
            });
        }

        if self.check(TokenType::Newline) { self.advance(); }
        Ok(print_stmt)
    }

    fn parse_assignment_statement(&mut self) -> Result<Statement, ParseError> {
                self.debug_trace("Parsing assignment statement");
                self.debug_trace("Parsing DO statement");
                self.debug_trace("Parsing IF statement");
            if is_verbose_debug() {
                println!("[DEBUG] Parsing assignment statement");
                println!("[DEBUG] Parsing DO statement");
                println!("[DEBUG] Parsing IF statement");
            }
        let id_tok = self.advance();
        let target = Identifier::new(
            id_tok.value.clone().unwrap_or_default(),
            Span::new(id_tok.line, id_tok.col, id_tok.line, id_tok.col),
        );
        if !self.match_token(TokenType::Eq) {
            return Err(ParseError::new(self.current_span(), "Expected '=' in assignment".to_string()));
        }

        if self.check(TokenType::Print) {
            let print_stmt = self.parse_print_statement()?;
            let span = print_stmt.match_span();
            let lam = Expr::Lambda(vec![print_stmt], span);
            if is_verbose_debug() {
                println!("[DIAGNOSTIC] Assignment AST: target={:?}, value={:?}", target, lam);
            }
            return Ok(Statement::Assignment { target: target.clone(), value: lam, span: target.span.clone() });
        } else if self.check(TokenType::Do) {
            let do_stmt = self.parse_do_statement()?;
            let span = do_stmt.match_span();
            let lam = match do_stmt {
                Statement::WhileBlock { .. } => {
                    // Wrap the WhileBlock in a lambda for assignment
                    Expr::Lambda(vec![do_stmt.clone()], span)
                }
                Statement::DoBlock { targets, alias: _, repeats: _, body, span: _ }
                    if targets.is_empty() =>
                {
                    Expr::Lambda(body.clone(), span)
                }
                other => Expr::Lambda(vec![other.clone()], span),
            };
            println!("[DIAGNOSTIC] Assignment AST: target={:?}, value={:?}", target, lam);
            return Ok(Statement::Assignment { target: target.clone(), value: lam, span: target.span.clone() });
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

        if self.check(TokenType::Newline) { self.advance(); }
        Ok(Statement::Assignment { target: target.clone(), value: expr, span: target.span.clone() })
    }

    fn parse_priority_override(&mut self) -> Result<Statement, ParseError> {
        let a = self.advance();
        let higher = Identifier::new(
            a.value.clone().unwrap_or_default(),
            Span::new(a.line, a.col, a.line, a.col),
        );
        if !self.match_token(TokenType::Over) {
            return Err(ParseError::new(self.current_span(), "Expected OVER in priority override".to_string()));
        }
        if !self.check(TokenType::Identifier) {
            return Err(ParseError::new(self.current_span(), "Expected identifier after OVER".to_string()));
        }
        let b = self.advance();
        let lower = Identifier::new(
            b.value.clone().unwrap_or_default(),
            Span::new(b.line, b.col, b.line, b.col),
        );
        if self.check(TokenType::Newline) { self.advance(); }
        let span = Span::new(a.line, a.col, b.line, b.col);
        Ok(Statement::PriorityOverride { higher, lower, span })
    }

    fn parse_expr_statement(&mut self) -> Result<Statement, ParseError> {
        let expr = self.parse_expression(0);
        if self.check(TokenType::Newline) { self.advance(); }
        let span = expr.span();
        Ok(Statement::ExprStmt { expr, span })
    }

    // ── Expression parsing (precedence climbing) ──────────────────────────────

    fn parse_expression(&mut self, min_prec: i32) -> Expr {
        let mut left = self.parse_unary();

        // After parsing primary, check for subscript indexing: expr[...]
        left = self.parse_postfix(left);

        loop {
            let tok = self.peek();
            let prec = self.get_prec(&tok.kind);
            if prec < min_prec || prec == 0 {
                break;
            }
            let op_tok = self.advance();
            let op = self.token_to_binop(&op_tok.kind);
            let next_min = prec + 1;
            let right = self.parse_expression(next_min);
            let span = Span::new(
                left.span().start_line, left.span().start_col,
                right.span().end_line, right.span().end_col,
            );
            left = Expr::Binary { op, left: Box::new(left), right: Box::new(right), span };
        }

        left
    }

    /// Parse postfix operators: subscript indexing `base[i]` or `base[i, j]`.
    fn parse_postfix(&mut self, mut expr: Expr) -> Expr {
        loop {
            if self.check(TokenType::LBracket) {
                let bracket_tok = self.advance(); // consume '['
                let mut indices = Vec::new();
                indices.reserve(4);
                while !self.check(TokenType::RBracket) && !self.is_eof() {
                    indices.push(self.parse_expression(0));
                    if self.check(TokenType::Comma) {
                        self.advance();
                    } else {
                        break;
                    }
                }
                let end_tok = if self.check(TokenType::RBracket) {
                    self.advance()
                } else {
                    // Missing ']' — synthesise a span
                    Token::new(TokenType::RBracket, None, bracket_tok.line, bracket_tok.col)
                };
                let span = Span::new(
                    expr.span().start_line, expr.span().start_col,
                    end_tok.line, end_tok.col,
                );
                expr = Expr::Index { base: Box::new(expr), indices, span };
            } else {
                break;
            }
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        if self.match_token(TokenType::Minus) {
            let rhs = self.parse_unary();
            let zero = Expr::Number(0.0, rhs.span());
            let span = rhs.span();
            Expr::Binary { op: BinaryOp::Sub, left: Box::new(zero), right: Box::new(rhs), span }
        } else if self.match_token(TokenType::Not) {
            // NOT expr — represent as Binary(0, Not, rhs)
            let rhs = self.parse_unary();
            let zero = Expr::Number(0.0, rhs.span());
            let span = rhs.span();
            Expr::Binary { op: BinaryOp::Not, left: Box::new(zero), right: Box::new(rhs), span }
        } else {
            self.parse_primary()
        }
    }



    fn parse_primary(&mut self) -> Expr {
        let tok = self.peek();
        match tok.kind {
            TokenType::Number => {
                let t = self.advance();
                let mut text = t.value.clone().unwrap_or_default();
                // remove underscores which are allowed for readability
                text.retain(|c| c != '_');

                // try decimal/float first
                let n = if let Ok(v) = text.parse::<f64>() {
                    v
                } else {
                    // handle integer prefixes 0x,0b,0o explicitly
                    let lower = text.to_ascii_lowercase();
                    if let Some(stripped) = lower.strip_prefix("0x") {
                        i64::from_str_radix(stripped, 16).unwrap_or(0) as f64
                    } else if let Some(stripped) = lower.strip_prefix("0b") {
                        i64::from_str_radix(stripped, 2).unwrap_or(0) as f64
                    } else if let Some(stripped) = lower.strip_prefix("0o") {
                        i64::from_str_radix(stripped, 8).unwrap_or(0) as f64
                    } else {
                        0.0
                    }
                };
                Expr::Number(n, Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::String => {
                let t = self.advance();
                Expr::String(t.value.clone().unwrap_or_default(), Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::Bool => {
                let t = self.advance();
                let b = t.value.as_ref().map(|s: &String| s.eq_ignore_ascii_case("true")).unwrap_or(false);
                Expr::Bool(b, Span::new(t.line, t.col, t.line, t.col))
            }
            TokenType::Identifier | TokenType::Class => {
                let id_tok = self.advance();
                let id = Identifier::new(
                    id_tok.value.clone().unwrap_or_default(),
                    Span::new(id_tok.line, id_tok.col, id_tok.line, id_tok.col),
                );
                if self.check(TokenType::LParen) {
                    self.advance(); // consume '('
                    let mut args = Vec::new();
                    args.reserve(4);
                    while !self.check(TokenType::RParen) && !self.is_eof() {
                        args.push(self.parse_expression(0));
                        if self.check(TokenType::Comma) { self.advance(); } else { break; }
                    }
                    if self.check(TokenType::RParen) {
                        let r = self.advance();
                        let span = Span::new(id.span.start_line, id.span.start_col, r.line, r.col);
                        Expr::Call { callee: Box::new(Expr::Identifier(id)), args, span }
                    } else {
                        let span = id.span.clone();
                        Expr::Call { callee: Box::new(Expr::Identifier(id)), args, span }
                    }
                } else {
                    Expr::Identifier(id)
                }
            }
            TokenType::LParen => {
                self.advance();
                let e = self.parse_expression(0);
                if self.check(TokenType::RParen) { self.advance(); }
                e
            }
            TokenType::LBracket => {
                // list literal — NOTE: subscript indexing is handled by parse_postfix,
                // so a bare '[' here is always a list constructor.
                let start = self.advance();
                let mut items = Vec::new();
                items.reserve(4);
                while !self.check(TokenType::RBracket) && !self.is_eof() {
                    items.push(self.parse_expression(0));
                    if self.check(TokenType::Comma) { self.advance(); } else { break; }
                }
                if self.check(TokenType::RBracket) {
                    let end = self.advance();
                    let span = Span::new(start.line, start.col, end.line, end.col);
                    Expr::List { items, span }
                } else {
                    let span = Span::new(start.line, start.col, start.line, start.col);
                    Expr::List { items, span }
                }
            }
            TokenType::Build => {
                // BUILD TENSOR: <expression> where expression should be a (possibly nested) list
                let start = self.advance(); // consume BUILD
                if self.match_token(TokenType::Tensor) {
                    if self.match_token(TokenType::Colon) {
                        // after the colon the user may start the tensor on the next line
                        // so we want to ignore any stray newline/indent/dedent tokens
                        // before we begin parsing the real expression.  otherwise a
                        // leading newline would be interpreted as a `Raw("Newline")`
                        // expression and end up in the tensor data (see
                        // `exec_build_tensor_multiline` failure).
                        while self.check(TokenType::Newline)
                            || self.check(TokenType::Indent)
                            || self.check(TokenType::Dedent)
                        {
                            self.advance();
                        }

                        // parse a general expression; prefer list-like syntax
                        let mut expr = self.parse_expression(0);

                        // if the expression is followed by comma-separated items we
                        // should combine them into a single `List` node.  this
                        // handles the common multiline tensor syntax:
                        //
                        //   BUILD TENSOR:
                        //       [1, 2],
                        //       [3, 4]
                        //
                        // we also tolerate a trailing comma by dropping any
                        // extraneous `Raw("Newline")`, `Raw("Indent")` or
                        // `Raw("Dedent")` nodes that may be produced.
                        if self.check(TokenType::Comma) {
                            // we will need the original expression later (for span),
                            // so clone it rather than move.
                            let mut items = vec![expr.clone()];
                            while self.check(TokenType::Comma) {
                                self.advance();
                                // skip any whitespace tokens that appear before
                                // the next expression
                                while self.check(TokenType::Newline)
                                    || self.check(TokenType::Indent)
                                    || self.check(TokenType::Dedent)
                                {
                                    self.advance();
                                }
                                // if the comma was trailing, we may have eaten all
                                // of the meaningful input; break in that case.
                                if self.check(TokenType::RBracket)
                                    || self.check(TokenType::Eof)
                                {
                                    break;
                                }
                                items.push(self.parse_expression(0));
                            }
                            // strip any bogus newline/indent/dedent raws inserted
                            items.retain(|e| {
                                match e {
                                    Expr::Raw(s, _) if s == "Newline" || s == "Indent" || s == "Dedent" => false,
                                    _ => true,
                                }
                            });
                            if !items.is_empty() {
                                let start_span = items.first().unwrap().span();
                                let end_span = items.last().unwrap().span();
                                expr = Expr::List {
                                    items,
                                    span: Span::new(
                                        start_span.start_line,
                                        start_span.start_col,
                                        end_span.end_line,
                                        end_span.end_col,
                                    ),
                                };
                            }
                        }
                        // try to compute span from the expr
                        let end_span = expr.span();
                        Expr::TensorBuilder {
                            expr: Box::new(expr),
                            span: Span::new(start.line, start.col, end_span.end_line, end_span.end_col),
                        }
                    } else {
                        Expr::Raw("BUILD TENSOR (missing colon)".to_string(), Span::new(start.line, start.col, start.line, start.col))
                    }
                } else {
                    Expr::Raw("BUILD (expected TENSOR)".to_string(), Span::new(start.line, start.col, start.line, start.col))
                }
            }
            _ => {
                let t = self.advance();
                Expr::Raw(
                    t.value.clone().unwrap_or_else(|| t.kind.to_string()),
                    Span::new(t.line, t.col, t.line, t.col),
                )
            }
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    #[inline]
    fn get_prec(&self, kind: &TokenType) -> i32 {
        *self.prec.get(kind).unwrap_or(&0)
    }

    #[inline]
    fn token_to_binop(&self, kind: &TokenType) -> BinaryOp {
        match kind {
            TokenType::Plus     => BinaryOp::Add,
            TokenType::Minus    => BinaryOp::Sub,
            TokenType::Star     => BinaryOp::Mul,
            TokenType::Slash    => BinaryOp::Div,
            TokenType::At       => BinaryOp::MatMul,
            TokenType::EqEq     => BinaryOp::Eq,
            TokenType::Neq      => BinaryOp::Neq,
            TokenType::Lt       => BinaryOp::Lt,
            TokenType::Gt       => BinaryOp::Gt,
            TokenType::Lte      => BinaryOp::Lte,
            TokenType::Gte      => BinaryOp::Gte,
            TokenType::Approx   => BinaryOp::Approx,
            TokenType::NotEq    => BinaryOp::NotEq,
            TokenType::StrictEq => BinaryOp::StrictEq,
            TokenType::And      => BinaryOp::And,
            TokenType::Or       => BinaryOp::Or,
            _ => panic!("Unknown binary operator token: {:?}", kind),
        }
    }

    #[inline]
    fn peek(&self) -> &Token {
        if self.pos < self.tokens.len() {
            &self.tokens[self.pos]
        } else {
            &self.eof
        }
    }

    #[inline]
    fn advance(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            t
        } else {
            Token::new(TokenType::Eof, None, 0, 0)
        }
    }

    #[inline]
    fn check(&self, kind: TokenType) -> bool {
        self.peek().kind == kind
    }

    #[inline]
    fn match_token(&mut self, kind: TokenType) -> bool {
        if self.check(kind.clone()) { self.advance(); true } else { false }
    }

    #[inline]
    fn is_eof(&self) -> bool {
        self.peek().kind == TokenType::Eof
    }

    #[inline]
    fn peek_is_assign(&self) -> bool {
        self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].kind == TokenType::Eq
    }

    #[inline]
    fn peek_is_priority_override(&self) -> bool {
        self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1].kind == TokenType::Over
    }

    #[inline]
    fn current_span(&self) -> Span {
        let t = self.peek();
        Span::new(t.line, t.col, t.line, t.col)
    }

    fn recover_to_next_statement(&mut self) {
        while !self.is_eof() && !self.check(TokenType::Newline) && !self.check(TokenType::Dedent) {
            self.advance();
        }
        if self.check(TokenType::Newline) { self.advance(); }
    }
}

// Small helper to extract span from Statement variants
trait StatementSpan {
    fn match_span(&self) -> Span;
}

impl StatementSpan for Statement {
    fn match_span(&self) -> Span {
        match self {
            Statement::Assignment { span, .. } => span.clone(),
            Statement::FunctionDef { span, .. } => span.clone(),
            Statement::DoBlock { span, .. } => span.clone(),
            Statement::WhileBlock { span, .. } => span.clone(),
            Statement::PriorityOverride { span, .. } => span.clone(),
            Statement::Constraint { span, .. } => span.clone(),
            Statement::ExprStmt { span, .. } => span.clone(),
            Statement::Print { span, .. } => span.clone(),
            Statement::If { span, .. } => span.clone(),
            Statement::End { span } => span.clone(),
            Statement::Other { span, .. } => span.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lexer::Lexer;

    fn parse(src: &str) -> Program {
        let tokens = Lexer::new(src).lex();
        Parser::new(tokens).parse()
    }

    #[test]
    fn parse_simple_assignment() {
        let prog = parse("x = 1\n");
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::Assignment { target, value, .. } => {
                assert_eq!(target.name, "x");
                assert!(matches!(value, Expr::Number(_, _)));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_prefixed_integer_literals() {
        let prog = parse("a = 0xFF\nb = 0b1010_0011\nc = 0o755\n");
        assert_eq!(prog.statements.len(), 3);
        let vals: Vec<f64> = prog.statements.iter().map(|stmt| {
            if let Statement::Assignment { value, .. } = stmt {
                if let Expr::Number(n, _) = value {
                    *n
                } else { 0.0 }
            } else { 0.0 }
        }).collect();
        assert_eq!(vals, vec![255.0, 0b1010_0011 as f64, 0o755 as f64]);
    }

    #[test]
    fn parse_do_multiple_targets_and_for() {
        let prog = parse("DO a, b FOR 3, 4:\n    x = 1\nEND\n");
        assert_eq!(prog.statements.len(), 1);
        match &prog.statements[0] {
            Statement::DoBlock { targets, repeats, body, .. } => {
                assert_eq!(targets.len(), 2);
                assert!(repeats.is_some());
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected do block"),
        }
    }

    #[test]
    fn parse_do_while_single_target() {
        let prog = parse("DO worker WHILE running:\n    x = x + 1\n");
        match &prog.statements[0] {
            Statement::WhileBlock { targets, condition, body, alias, .. } => {
                assert_eq!(targets.len(), 1);
                assert_eq!(targets[0].name, "worker");
                assert!(alias.is_none());
                assert!(matches!(condition, Expr::Identifier(_)));
                assert_eq!(body.len(), 1);
            }
            _ => panic!("expected while block"),
        }
    }

    #[test]
    fn parse_if_simple() {
        let prog = parse("IF x < 10 DO:\n    y = 1\n");
        match &prog.statements[0] {
            Statement::If { conditions, then_body, else_body, .. } => {
                assert_eq!(conditions.len(), 1);
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_none());
            }
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn parse_if_with_or_conditions() {
        let prog = parse("IF x < 10 OR y > 5 DO:\n    z = 1\n");
        match &prog.statements[0] {
            Statement::If { conditions, .. } => {
                assert_eq!(conditions.len(), 2);
            }
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn parse_if_with_else() {
        let prog = parse("IF x < 10 DO:\n    y = 1\nOTHERWISE DO:\n    y = 2\n");
        match &prog.statements[0] {
            Statement::If { then_body, else_body, .. } => {
                assert_eq!(then_body.len(), 1);
                assert!(else_body.is_some());
                assert_eq!(else_body.as_ref().unwrap().len(), 1);
            }
            _ => panic!("expected if statement"),
        }
    }

    #[test]
    fn parse_approx_operator() {
        let prog = parse("x = a ≈ b\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                assert!(matches!(value, Expr::Binary { op: BinaryOp::Approx, .. }));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_stricteq_operator() {
        let prog = parse("x = a ≡ b\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                assert!(matches!(value, Expr::Binary { op: BinaryOp::StrictEq, .. }));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_noteq_operator() {
        let prog = parse("x = a ≠ b\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                assert!(matches!(value, Expr::Binary { op: BinaryOp::NotEq, .. }));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_matmul_operator() {
        let prog = parse("x = A @ B\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                assert!(matches!(value, Expr::Binary { op: BinaryOp::MatMul, .. }));
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_build_tensor_simple() {
        let prog = parse("x = BUILD TENSOR: [[1,2],[3,4]]\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                match value {
                    Expr::TensorBuilder { expr, .. } => {
                        // should be a list of two lists
                        if let Expr::List { items, .. } = &**expr {
                            assert_eq!(items.len(), 2);
                        } else { panic!("expected list"); }
                    }
                    other => panic!("expected TensorBuilder, got {:?}", other),
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_build_tensor_multiline() {
        let src = "x = BUILD TENSOR:\n    [1, 2],\n    [3, 4]\n";
        let prog = parse(src);
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                match value {
                    Expr::TensorBuilder { expr, .. } => {
                        if let Expr::List { items, .. } = &**expr {
                            assert_eq!(items.len(), 2);
                        } else { panic!("expected list"); }
                    }
                    _ => panic!("expected TensorBuilder"),
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_build_tensor_trailing_comma_inline() {
        let prog = parse("x = BUILD TENSOR: [[1,2],[3,4],]\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                match value {
                    Expr::TensorBuilder { expr, .. } => {
                        if let Expr::List { items, .. } = &**expr {
                            assert_eq!(items.len(), 2);
                        } else { panic!("expected list"); }
                    }
                    other => panic!("expected TensorBuilder, got {:?}", other),
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_build_tensor_trailing_comma_multiline() {
        let src = "x = BUILD TENSOR:\n    [1, 2],\n    [3, 4],\n";
        let prog = parse(src);
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                match value {
                    Expr::TensorBuilder { expr, .. } => {
                        if let Expr::List { items, .. } = &**expr {
                            assert_eq!(items.len(), 2);
                        } else { panic!("expected list"); }
                    }
                    _ => panic!("expected TensorBuilder"),
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_tensor_higher_dim() {
        let prog = parse("x = BUILD TENSOR: [[[1,2],[3,4]],[[5,6],[7,8]]]\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                match value {
                    Expr::TensorBuilder { expr, .. } => {
                        // expect top-level list of two items
                        if let Expr::List { items, .. } = &**expr {
                            assert_eq!(items.len(), 2);
                        } else { panic!("expected list"); }
                    }
                    _ => panic!("expected TensorBuilder"),
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_tensor_index_1d() {
        let prog = parse("x = T[0]\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                assert!(matches!(value, Expr::Index { .. }));
                if let Expr::Index { base, indices, .. } = value {
                    assert!(matches!(base.as_ref(), Expr::Identifier(id) if id.name == "T"));
                    assert_eq!(indices.len(), 1);
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_tensor_index_2d() {
        let prog = parse("x = T[1, 2]\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                if let Expr::Index { indices, .. } = value {
                    assert_eq!(indices.len(), 2);
                } else {
                    panic!("expected Index expr");
                }
            }
            _ => panic!("expected assignment"),
        }
    }

    #[test]
    fn parse_comma_list_rhs_produces_list_expr() {
        let prog = parse("b = a, a\n");
        match &prog.statements[0] {
            Statement::Assignment { value, .. } => {
                assert!(matches!(value, Expr::List { .. }));
            }
            _ => panic!("expected assignment"),
        }
    }
}
