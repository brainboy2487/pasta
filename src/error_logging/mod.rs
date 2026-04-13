// src/error_logging/mod.rs
//! Error logging and catalog-backed diagnostic helpers.
//!
//! This module complements the interpreter's native error types with:
//! - a JSON-backed catalog of longer explanations and suggestions
//! - helpers for formatting catalog entries into expanded diagnostics
//! - persistence of structured diagnostic bundles with traceback data
//!
//! The main runtime integration points are recursion/stack-guard failures and
//! panic-hook diagnostics.

pub mod error_handler;
pub mod error_messages;
