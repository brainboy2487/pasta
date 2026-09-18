// src/runtime/asm.rs
//! Simple ASM block runtime for PASTA
//!
//! Responsibilities:
//! - Represent inline ASM blocks parsed by the parser as `AsmBlock`.
//! - Provide a small, safe "executor" that can run a very limited subset of
//!   ASM-like commands in a sandboxed manner against the interpreter `Environment`.
//! - Store and retrieve named ASM blocks for later invocation.
//!
//! Notes:
//! - This is intentionally conservative: ASM blocks are treated as sequences of
//!   raw lines. The executor implements only a tiny, well-documented subset of
//!   commands (e.g., `set`, `print`) so ASM can be useful for quick scripting
//!   without exposing arbitrary host execution. Extend carefully if you need
//!   more capabilities (e.g., a WASM sandbox).
//!
//! Example supported lines:
//! - `set x = 42`      -> sets variable `x` in the current environment scope
//! - `print x`         -> prints the value of `x` (for debugging)
//! - `print "hello"`   -> prints a string literal
//!
//! Unsupported lines will return an error rather than executing arbitrary code.

use anyhow::{anyhow, Result};
use std::collections::HashMap;

use crate::interpreter::environment::{Environment, Value};

/// Representation of an ASM block captured by the parser.
#[derive(Debug, Clone)]
pub struct AsmBlock {
    /// Optional name given to the ASM block (e.g., `ASM foo:`).
    pub name: Option<String>,
    /// Raw lines inside the ASM block (trimmed).
    pub lines: Vec<String>,
}

impl AsmBlock {
    /// Create a new unnamed ASM block.
    pub fn new(lines: Vec<String>) -> Self {
        Self { name: None, lines }
    }

    /// Create a named ASM block.
    pub fn with_name(name: impl Into<String>, lines: Vec<String>) -> Self {
        Self {
            name: Some(name.into()),
            lines,
        }
    }
}

/// A tiny ASM runtime that stores ASM blocks and can execute them in a sandbox.
///
/// The runtime does not spawn processes or run arbitrary shell commands. It
/// supports a minimal command set that manipulates the provided `Environment`.
pub struct AsmRuntime {
    /// Named ASM blocks (name -> block)
    blocks: HashMap<String, AsmBlock>,
}

impl AsmRuntime {
    /// Create a new, empty runtime.
    pub fn new() -> Self {
        Self {
            blocks: HashMap::new(),
        }
    }

    /// Register an ASM block. If a block with the same name exists, it is replaced.
    pub fn register_block(&mut self, block: AsmBlock) -> Result<()> {
        if let Some(name) = &block.name {
            self.blocks.insert(name.clone(), block);
            Ok(())
        } else {
            Err(anyhow!("Cannot register unnamed ASM block"))
        }
    }

    /// Retrieve a registered block by name.
    pub fn get_block(&self, name: &str) -> Option<&AsmBlock> {
        self.blocks.get(name)
    }

    /// Execute an ASM block by name against the provided environment.
    ///
    /// Returns Ok(()) on success or an error describing the first unsupported
    /// or failing instruction.
    pub fn execute_block_by_name(&self, name: &str, env: &mut Environment) -> Result<()> {
        let block = self
            .blocks
            .get(name)
            .ok_or_else(|| anyhow!("ASM block '{}' not found", name))?;
        self.execute_block(block, env)
    }

    /// Execute an ASM block (anonymous or named) against the provided environment.
    ///
    /// Supported instructions (line-based):
    /// - `set <ident> = <number|string>`  -> sets a variable in the current scope
    /// - `print <ident|\"literal\">`       -> prints a value to stdout
    ///
    /// The function is conservative: any line that does not match the supported
    /// patterns will return an error.
    pub fn execute_block(&self, block: &AsmBlock, env: &mut Environment) -> Result<()> {
        for (idx, raw) in block.lines.iter().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }

            // Simple `set` command: set <ident> = <literal>
            if let Some(rest) = line.strip_prefix("set ") {
                // Expect format: <ident> = <value>
                let parts: Vec<&str> = rest.splitn(2, '=').map(|s| s.trim()).collect();
                if parts.len() != 2 {
                    return Err(anyhow!("ASM parse error on line {}: invalid set syntax", idx + 1));
                }
                let name = parts[0];
                if !is_valid_ident(name) {
                    return Err(anyhow!("ASM parse error on line {}: invalid identifier '{}'", idx + 1, name));
                }
                let val_text = parts[1];
                let val = parse_literal(val_text)?;
                // Set in current (innermost) scope
                env.set_local(name.to_string(), val);
                continue;
            }

            // Simple `print` command: print <expr>
            if let Some(rest) = line.strip_prefix("print ") {
                let arg = rest.trim();
                if arg.starts_with('"') && arg.ends_with('"') && arg.len() >= 2 {
                    // string literal
                    let inner = &arg[1..arg.len() - 1];
                    println!("{}", inner);
                } else if is_valid_ident(arg) {
                    if let Some(v) = env.get(arg) {
                        println!("{:?}", v);
                    } else {
                        println!("(undefined {})", arg);
                    }
                } else {
                    // Try to parse as number literal
                    match arg.parse::<f64>() {
                        Ok(n) => println!("{}", n),
                        Err(_) => {
                            return Err(anyhow!("ASM parse error on line {}: unsupported print argument '{}'", idx + 1, arg));
                        }
                    }
                }
                continue;
            }

            // No supported instruction matched
            return Err(anyhow!("Unsupported ASM instruction on line {}: '{}'", idx + 1, line));
        }

        Ok(())
    }
}

/// Very small identifier validator (matches parser's identifier rules loosely).
fn is_valid_ident(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for ch in chars {
        if !(ch.is_ascii_alphanumeric() || ch == '_') {
            return false;
        }
    }
    true
}

/// Parse a literal token into a runtime `Value`.
///
/// Supports:
/// - numeric literals (floating point)
/// - string literals wrapped in double quotes
/// - boolean `true` / `false`
///
/// Returns an error for unsupported forms.
fn parse_literal(s: &str) -> Result<Value> {
    let s = s.trim();
    if s.starts_with('"') && s.ends_with('"') && s.len() >= 2 {
        let inner = &s[1..s.len() - 1];
        return Ok(Value::String(inner.to_string()));
    }
    if s.eq_ignore_ascii_case("true") {
        return Ok(Value::Bool(true));
    }
    if s.eq_ignore_ascii_case("false") {
        return Ok(Value::Bool(false));
    }
    if let Ok(n) = s.parse::<f64>() {
        return Ok(Value::Number(n));
    }
    Err(anyhow!("Unsupported literal: {}", s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::environment::{Environment, Value};

    #[test]
    fn asm_set_and_print() {
        let mut env = Environment::new();
        let block = AsmBlock::new(vec![
            "set x = 10".into(),
            "set msg = \"hello\"".into(),
            "print x".into(),
            "print msg".into(),
        ]);
        let rt = AsmRuntime::new();
        // Execute should succeed and set variables
        rt.execute_block(&block, &mut env).unwrap();
        assert_eq!(env.get("x"), Some(Value::Number(10.0)));
        assert_eq!(env.get("msg"), Some(Value::String("hello".into())));
    }

    #[test]
    fn asm_register_and_invoke_named_block() {
        let mut env = Environment::new();
        let mut rt = AsmRuntime::new();
        let block = AsmBlock::with_name("init", vec!["set a = 1".into(), "set b = 2".into()]);
        rt.register_block(block.clone()).unwrap();
        assert!(rt.get_block("init").is_some());
        rt.execute_block_by_name("init", &mut env).unwrap();
        assert_eq!(env.get("a"), Some(Value::Number(1.0)));
        assert_eq!(env.get("b"), Some(Value::Number(2.0)));
    }

    #[test]
    fn asm_unsupported_instruction_errors() {
        let mut env = Environment::new();
        let block = AsmBlock::new(vec!["exec rm -rf /".into()]);
        let rt = AsmRuntime::new();
        let res = rt.execute_block(&block, &mut env);
        assert!(res.is_err());
    }

    #[test]
    fn parse_literal_variants() {
        assert_eq!(parse_literal("42").unwrap(), Value::Number(42.0));
        assert_eq!(parse_literal("3.14").unwrap(), Value::Number(3.14));
        assert_eq!(parse_literal("\"hi\"").unwrap(), Value::String("hi".into()));
        assert_eq!(parse_literal("true").unwrap(), Value::Bool(true));
        assert!(parse_literal("unknown").is_err());
    }
}
