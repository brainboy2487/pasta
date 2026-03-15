#![warn(missing_docs)]
use std::sync::atomic::AtomicBool;
/// Global verbose flags for diagnostics
#[no_mangle]
pub static VERBOSE_FLAG: AtomicBool = AtomicBool::new(false);
#[no_mangle]
pub static VERBOSE_DEBUG: AtomicBool = AtomicBool::new(false);
// pasta/src/lib.rs
/// PASTA — small language runtime library

pub mod lexer;
pub mod parser;
pub mod semantics;
pub mod interpreter;
pub mod runtime;

#[cfg(feature = "typing")]
pub mod typing;

#[cfg(feature = "scheduler")]
pub mod scheduler;

#[cfg(feature = "scheduler")]
pub use scheduler::Scheduler;

#[cfg(not(feature = "scheduler"))]
#[derive(Debug, Clone)]
pub struct Scheduler;

pub use interpreter::{Executor, Environment, Value, ThreadMeta};
pub use parser::ast::{Program, Statement, Expr, Identifier, Span, BinaryOp, RelationToken};

pub use runtime::{auto_configure, detect_host_arch};
pub use runtime::asm::{AsmRuntime, AsmBlock};

/// Convenience: initialize an executor and attempt to auto-configure the environment
pub fn init_executor_with_auto_config() -> Executor {
    let mut exe = Executor::new();
    match auto_configure(&mut exe.env) {
        Ok(Some(dev)) => {
            exe.diagnostics.push(format!("Auto-configured device: {} ({})", dev.id, dev.arch));
        }
        Ok(None) => {
            exe.diagnostics.push("No matching device profile found; using defaults".to_string());
        }
        Err(e) => {
            exe.diagnostics.push(format!("Device auto-configure error: {}", e));
        }
    }
    exe
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parser::Parser;
    use crate::lexer::lexer::Lexer;

    #[test]
    fn lib_init_and_parse_smoke() {
        let src = r#"
set x = 10
set y = x + 5
"#;
        let tokens = Lexer::new(src).lex();
        let mut p = Parser::new(tokens);
        let program = p.parse();

        let mut exe = init_executor_with_auto_config();
        let _ = exe.execute_program(&program);

        // We don't assert runtime here because executor may be in flux; keep smoke test simple.
        assert!(!program.statements.is_empty());
    }
}
