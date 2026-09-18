use std::fmt;

use crate::parser::ast::Span;

/// Compiler error for the bootstrap native compiler path.
#[derive(Debug, Clone)]
pub struct CompilerError {
    message: String,
    span: Option<Span>,
}

impl CompilerError {
    pub(crate) fn unsupported(message: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub(crate) fn tool(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            span: None,
        }
    }
}

impl fmt::Display for CompilerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(span) = &self.span {
            write!(
                f,
                "compiler error at {}:{}: {}",
                span.start_line, span.start_col, self.message
            )
        } else {
            write!(f, "compiler error: {}", self.message)
        }
    }
}

impl std::error::Error for CompilerError {}
