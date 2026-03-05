// src/parser/grammar.rs
//! Grammar reference for the PASTA language (EBNF-style).
//!
//! This file is a canonical, human-readable grammar used by the parser
//! implementation and for documentation. It is not machine-generated but
//! is kept in sync with the parser implementation. Use this as the source
//! of truth when extending syntax.
//!
//! Notation:
//!  - `::=` defines a production
//!  - `|` separates alternatives
//!  - `{ ... }` zero-or-more repetition
//!  - `[ ... ]` optional
//!  - `(...)` grouping
//!  - terminals are lowercase or punctuation; nonterminals are Capitalized
//!  - `INDENT` / `DEDENT` / `NEWLINE` are produced by the lexer
//!
//! High-level goals:
//!  - Keep statements line-oriented with optional indented blocks
//!  - Support plain-English aliases normalized by the lexer
//!  - Provide explicit constructs for DO blocks, LIMIT OVER constraints,
//!    priority overrides, LEARN model blocks, and ASM blocks
//!  - Allow expression nesting, function calls, and lists

// Program
// -------
// A program is a sequence of statements terminated by EOF.
pub const GRAMMAR: &str = r#"
Program ::= { Statement } EOF

// Statements
// ----------
Statement ::=
      Assignment
    | DoStatement
    | PriorityOverride
    | ConstraintStatement
    | IfStatement
    | TryStatement
    | ClassDecl
    | GroupDecl
    | LearnDecl
    | AsmBlock
    | ExprStatement
    | EndStatement

// Assignment
// ----------
Assignment ::=
    ( "set" Identifier "=" Expr NEWLINE )
  | ( Identifier "=" Expr NEWLINE )

// DO block
// --------
// DO may appear with optional alias and optional repeat count.
// The body is an indented block of statements.
DoStatement ::=
    "DO" Identifier [ "AS" Identifier ] [ "FOR" Expr ] ":" NEWLINE INDENT { Statement } DEDENT

// Priority override
// -----------------
// Can appear as a standalone statement or inside a DO line.
// Examples:
//   DO A OVER B
//   A OVER B
PriorityOverride ::=
    Identifier "OVER" Identifier NEWLINE

// Constraint statement (LIMIT OVER)
// ---------------------------------
// Forms:
//   <expr> [ Relation ] <expr> "LIMIT OVER" <expr>
// Examples:
//   velocity distance LIMIT OVER time
//   x equals y LIMIT OVER z
ConstraintStatement ::=
    Expr [ Relation ] Expr "LIMIT OVER" Expr NEWLINE

Relation ::=
    "approaches" | "equals" | "is" | "is not" | ">" | "<" | ">=" | "<=" | "minimize" | "maximize" | "in"

// If statement
// ------------
IfStatement ::=
    "IF" Expr ":" NEWLINE INDENT { Statement } DEDENT [ "OTHERWISE" ":" NEWLINE INDENT { Statement } DEDENT ]

// Try / Otherwise (catch-like)
// ----------------------------
TryStatement ::=
    "TRY" ":" NEWLINE INDENT { Statement } DEDENT [ "OTHERWISE" ":" NEWLINE INDENT { Statement } DEDENT ]

// Class and Group declarations
// ----------------------------
ClassDecl ::=
    "CLASS" Identifier ":" NEWLINE INDENT { Statement } DEDENT

GroupDecl ::=
    "GROUP" Identifier ":" NEWLINE INDENT { Statement } DEDENT

// LEARN block (model builder)
// ---------------------------
// Example:
//   LEARN model_name AS ModelType (params) :
//       ... indented training block ...
LearnDecl ::=
    "LEARN" Identifier [ "AS" Identifier ] [ "(" ParamList ")" ] ":" NEWLINE INDENT { Statement } DEDENT

ParamList ::= [ Param { "," Param } ]
Param ::= Identifier [ "=" Expr ]

// ASM block
// ---------
// Inline assembly sandbox; content treated as raw lines until END or DEDENT.
AsmBlock ::=
    "ASM" [ Identifier ] ":" NEWLINE INDENT { RawLine } DEDENT

RawLine ::= { any characters except EOF }

// Expression statements
// ---------------------
ExprStatement ::=
    Expr NEWLINE

EndStatement ::=
    "END" NEWLINE

// Expressions
// -----------
Expr ::=
      BinaryExpr
    | CallExpr
    | ListExpr
    | Primary

BinaryExpr ::=
    Expr BinaryOp Expr

BinaryOp ::=
    "+" | "-" | "*" | "/" | "==" | "!=" | "<" | ">" | "<=" | ">=" | "and" | "or"

CallExpr ::=
    Primary "(" [ ArgList ] ")"

ArgList ::= Expr { "," Expr }

ListExpr ::=
    "[" [ Expr { "," Expr } ] "]"

Primary ::=
      Number
    | String
    | Bool
    | Identifier
    | "(" Expr ")"

// Identifiers and literals
// ------------------------
Identifier ::= /[A-Za-z_][A-Za-z0-9_]*/
Number ::= /[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?/
String ::= /"([^"\\]|\\.)*"/
Bool   ::= "true" | "false"

// Whitespace and layout
// ---------------------
// INDENT / DEDENT tokens are produced by the lexer using leading spaces.
// NEWLINE terminates most statements; semicolon-style inline separators are not used.
"#;
