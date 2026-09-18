//! PASTA Exception Handling Module
//!
//! This module provides robust exception handling for the PASTA interpreter.
//! It supports:
//! - TRY/OTHERWISE blocks (simple exception handling)
//! - ATTEMPT(err)/OTHERWISE blocks (with error capture)
//! - Nested exception handling
//! - Both block and inline syntax styles
//!
//! # Syntax Examples
//!
//! ## Block Style
//! ```pasta
//! TRY:
//!     risky_operation()
//! OTHERWISE:
//!     fallback_operation()
//! END
//! ```
//!
//! ## Inline Style  
//! ```pasta
//! TRY: DO risky() OTHERWISE: DO fallback()
//! ```
//!
//! ## With Error Capture
//! ```pasta
//! ATTEMPT(err):
//!     risky_operation()
//! OTHERWISE:
//!     PRINT("Error: " err)
//! END
//! ```

use crate::interpreter::executor::Executor;
use crate::interpreter::environment::{ScopeKind, Value};
use crate::parser::ast::{Statement, Identifier};
use anyhow::Result;

/// Result of executing a try block
#[derive(Debug)]
pub enum TryResult {
    /// Block executed successfully
    Success,
    /// Block failed with error message
    Error(String),
}

/// Execute a TRY block without error capture
///
/// # Arguments
/// * `exec` - The executor context
/// * `try_body` - Statements to try executing
/// * `else_body` - Statements to execute on error
///
/// # Returns
/// * `Ok(None)` - Always returns None, errors are handled internally
pub fn execute_try_block(
    exec: &mut Executor,
    try_body: &[Statement],
    else_body: &[Statement],
) -> Result<Option<Value>> {
    // Push new scope for TRY block to prevent variable leaks
    crate::interpreter::ex_frame::push_scope(&mut exec.env, ScopeKind::Block); // TRY body
    
    let try_result = execute_body_catching_errors(exec, try_body);

    // Pop TRY block scope, handling any errors gracefully
    crate::interpreter::ex_frame::pop_scope(&mut exec.env, "TRY block cleanup", &mut exec.diagnostics);

    match try_result {
        TryResult::Success => {
            // Try block succeeded, no fallback needed
        }
        TryResult::Error(_) => {
            // Push new scope for TRY ELSE block to prevent variable leaks
            crate::interpreter::ex_frame::push_scope(&mut exec.env, ScopeKind::Block); // TRY ELSE body
            
            // Execute fallback - error message not captured
            let else_result = execute_body_ignoring_control_flow(exec, else_body);
            
            // Pop TRY ELSE block scope, handling any errors gracefully
            crate::interpreter::ex_frame::pop_scope(&mut exec.env, "TRY ELSE block cleanup", &mut exec.diagnostics);
            
            // Propagate any error from ELSE block
            else_result?;
        }
    }

    Ok(None)
}

/// Execute an ATTEMPT block with error capture
///
/// # Arguments
/// * `exec` - The executor context
/// * `err_var` - Variable name to bind error message to
/// * `try_body` - Statements to try executing
/// * `else_body` - Statements to execute on error
///
/// # Returns
/// * `Ok(None)` - Always returns None, errors are handled internally
pub fn execute_attempt_block(
    exec: &mut Executor,
    err_var: &Identifier,
    try_body: &[Statement],
    else_body: &[Statement],
) -> Result<Option<Value>> {
    // Push new scope for ATTEMPT block to prevent variable leaks
    crate::interpreter::ex_frame::push_scope(&mut exec.env, ScopeKind::Block); // TRY body
    
    let try_result = execute_body_catching_errors(exec, try_body);

    // Pop ATTEMPT block scope, handling any errors gracefully
    crate::interpreter::ex_frame::pop_scope(&mut exec.env, "ATTEMPT block cleanup", &mut exec.diagnostics);

    match try_result {
        TryResult::Success => {
            // Try block succeeded, no fallback needed
        }
        TryResult::Error(err_msg) => {
            // Bind error message to variable in current (outer) scope
            exec.env.set_local(err_var.name.clone(), Value::String(err_msg));
            // Execute fallback
            execute_body_ignoring_control_flow(exec, else_body)?;
        }
    }

    Ok(None)
}

/// Execute a list of statements, catching any errors
fn execute_body_catching_errors(exec: &mut Executor, body: &[Statement]) -> TryResult {
    let result: Result<(), anyhow::Error> = (|| {
        for stmt in body {
            exec.execute_statement(stmt)?;
            if exec.control_flow.is_some() {
                break;
            }
            let _ = exec.collect_garbage();
        }
        Ok(())
    })();

    match result {
        Ok(_) => TryResult::Success,
        Err(e) => TryResult::Error(e.to_string()),
    }
}

/// Execute a list of statements, handling control flow
fn execute_body_ignoring_control_flow(exec: &mut Executor, body: &[Statement]) -> Result<()> {
    for stmt in body {
        exec.execute_statement(stmt)?;
        if exec.control_flow.is_some() {
            break;
        }
        let _ = exec.collect_garbage();
    }
    Ok(())
}

/// Check if an error should be re-raised (for fatal errors)
#[allow(dead_code)]
pub fn is_fatal_error(error: &str) -> bool {
    // Some errors should not be caught
    error.contains("stack overflow") ||
    error.contains("out of memory") ||
    error.contains("fatal")
}

/// Format an error message for display
#[allow(dead_code)]
pub fn format_error(error: &str) -> String {
    // Strip common prefixes for cleaner display
    let cleaned = error
        .trim_start_matches("error: ")
        .trim_start_matches("Error: ");
    cleaned.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_try_result_success() {
        let result = TryResult::Success;
        assert!(matches!(result, TryResult::Success));
    }

    #[test]
    fn test_try_result_error() {
        let result = TryResult::Error("test error".to_string());
        if let TryResult::Error(msg) = result {
            assert_eq!(msg, "test error");
        } else {
            panic!("Expected Error variant");
        }
    }

    #[test]
    fn test_is_fatal_error() {
        assert!(is_fatal_error("stack overflow detected"));
        assert!(is_fatal_error("out of memory"));
        assert!(is_fatal_error("fatal error occurred"));
        assert!(!is_fatal_error("undefined variable 'x'"));
    }

    #[test]
    fn test_format_error() {
        assert_eq!(format_error("error: something bad"), "something bad");
        assert_eq!(format_error("Error: something bad"), "something bad");
        assert_eq!(format_error("something bad"), "something bad");
    }
}
