//! src/interpreter/repl.rs
//!
//! Robust interactive REPL for the PASTA interpreter.
//! Drop‑in replacement: calls `executor.enter_shell()` as a method.

use std::io::{self, Write};
use anyhow::{anyhow, Result};

use crate::lexer::lexer::Lexer;
use crate::lexer::TokenType;
use crate::parser::parser::Parser;
use crate::interpreter::executor::Executor;
use crate::interpreter::environment::Value;

/// Run the interactive REPL loop. Returns only on EOF or `exit`.
pub fn run_repl() -> Result<()> {
    let mut executor = Executor::new();
    let mut buffer = String::new();
    let mut indent_depth: i32 = 0;

    print_banner();

    loop {
        let prompt = if indent_depth > 0 || !buffer.trim().is_empty() {
            "....> "
        } else {
            "pasta> "
        };

        let line = match read_line(prompt)? {
            Some(l) => l,
            None => {
                // EOF: flush any pending block
                if !buffer.trim().is_empty() {
                    run_block(&mut executor, &buffer);
                    buffer.clear();
                }
                println!();
                break;
            }
        };

        let trimmed = line.trim();

        // Exit commands
        match trimmed {
            "exit" | "quit" | "exit()" | "quit()" | ":exit" | ":quit" => {
                println!("Goodbye.");
                break;
            }
            _ => {}
        }

        // Meta commands (start with ':'), only at top level
        if trimmed.starts_with(':') && indent_depth == 0 && buffer.trim().is_empty() {
            if let Err(e) = handle_meta(trimmed, &mut executor) {
                eprintln!("Meta command failed: {}", e);
            }
            continue;
        }

        // Blank line: if buffer has content and we're at top level, execute it
        if trimmed.is_empty() {
            if !buffer.trim().is_empty() && indent_depth == 0 {
                run_block(&mut executor, &buffer);
                buffer.clear();
            }
            continue;
        }

        // Accumulate input
        buffer.push_str(&line);
        if !line.ends_with('\n') {
            buffer.push('\n');
        }

        // Update indent depth via token-based counting
        indent_depth = compute_indent_depth(&buffer);

        // Still inside an indented block — keep reading
        if indent_depth > 0 {
            continue;
        }

        // Top-level complete — execute
        run_block(&mut executor, &buffer);
        buffer.clear();
        indent_depth = 0;
    }

    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Internal helpers
// ─────────────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!("PASTA interpreter — :help for commands, exit to quit");
}

fn read_line(prompt: &str) -> Result<Option<String>> {
    print!("{}", prompt);
    io::stdout().flush().map_err(|e| anyhow!("flush: {}", e))?;

    let mut line = String::new();
    let n = io::stdin().read_line(&mut line)?;
    if n == 0 {
        Ok(None)
    } else {
        Ok(Some(line))
    }
}

/// Compute the current indentation depth for the given source fragment.
/// Counts INDENT / DEDENT tokens emitted by the lexer. Returns 0 on lex error.
fn compute_indent_depth(src: &str) -> i32 {
    match Lexer::new(src).lex_result() {
        Ok(tokens) => {
            let mut depth = 0i32;
            for tok in &tokens {
                match tok.kind {
                    TokenType::Indent => depth += 1,
                    TokenType::Dedent => depth -= 1,
                    _ => {}
                }
            }
            depth
        }
        Err(_) => 0,
    }
}

/// Lex, parse, and execute a block, printing structured diagnostics on error.
fn run_block(executor: &mut Executor, src: &str) {
    // ── Lex ──────────────────────────────────────────────────────────────────
    let tokens = match Lexer::new(src).lex_result() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("Lex error at {}:{}: {}", e.line, e.col, e.message);
            return;
        }
    };

    // ── Parse ─────────────────────────────────────────────────────────────────
    let (program, parse_diags) = {
        let mut parser = Parser::new(tokens);
        parser.parse_with_diagnostics()
    };

    for d in &parse_diags {
        eprintln!(
            "Parse error at {}:{}: {}",
            d.span.start_line, d.span.start_col, d.message
        );
    }
    if program.statements.is_empty() && !parse_diags.is_empty() {
        return;
    }

    // ── Execute ───────────────────────────────────────────────────────────────
    if let Err(e) = executor.execute_repl(&program) {
        eprintln!("Runtime error: {}", e);
    }

    // ── Drain executor diagnostics ────────────────────────────────────────────
    let drained: Vec<String> = executor.diagnostics.drain(..).collect();
    for d in drained {
        if !d.starts_with("Auto-configured device:") {
            eprintln!("note: {}", d);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Meta commands
// ─────────────────────────────────────────────────────────────────────────────

fn handle_meta(cmd: &str, executor: &mut Executor) -> Result<()> {
    match cmd {
        ":help" => {
            println!("Commands:");
            println!("  exit / quit        exit the REPL");
            println!("  :env               show all variables in scope");
            println!("  :threads           show active DO threads");
            println!("  :keywords          show all available keywords and commands");
            println!("  :reset             reset the interpreter state");
            println!("  :diag              show and clear pending diagnostics");
            println!("  :clear             clear the screen (ANSI)");
            println!("  :help              show this help");
            println!("  :shell             enter the integrated shell");
        }

        ":env" => {
            let vars = executor.env.list_vars();
            if vars.is_empty() {
                println!("(no variables)");
            } else {
                let mut pairs: Vec<(&String, &Value)> = vars.iter().collect();
                pairs.sort_by_key(|(k, _)| k.as_str());
                for (k, v) in pairs {
                    println!("  {} = {}", k, fmt_value(v));
                }
            }
        }

        ":threads" => {
            use std::collections::HashMap;
            use crate::interpreter::environment::ThreadMeta;

            let (_vars, threads): (HashMap<String, Value>, HashMap<u64, ThreadMeta>) =
                executor.env.snapshot();

            if threads.is_empty() {
                println!("(no active threads)");
            } else {
                let mut ids: Vec<u64> = threads.keys().cloned().collect();
                ids.sort_unstable();
                for id in ids {
                    if let Some(meta) = threads.get(&id) {
                        println!(
                            "  id={} name={:?} weight={}",
                            id, meta.name, meta.priority_weight
                        );
                    }
                }
            }
        }

        ":keywords" => {
            print_keywords();
        }

        ":reset" => {
            *executor = Executor::new();
            println!("Interpreter state reset.");
        }

        ":diag" => {
            if executor.diagnostics.is_empty() {
                println!("(no diagnostics)");
            } else {
                let diags: Vec<String> = executor.diagnostics.drain(..).collect();
                for d in diags {
                    println!("diag: {}", d);
                }
            }
        }

        ":clear" => {
            print!("\x1B[2J\x1B[1;1H");
            io::stdout().flush().ok();
        }

        // New meta command: enter integrated shell
        ":shell" => {
            // Call the shell via the Executor method; ensure Executor defines `pub fn enter_shell(&mut self)`.
            match executor.enter_shell() {
                Ok(_) => println!("Exited shell."),
                Err(e) => eprintln!("shell error: {}", e),
            }
        }

        other => {
            eprintln!("Unknown command: {}", other);
            eprintln!("Type :help for available commands.");
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Value formatting helper
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_value(v: &Value) -> String {
    match v {
        Value::Number(n) => {
            if n.fract() == 0.0 && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Value::String(s) => format!("{:?}", s),
        Value::Bool(b) => b.to_string(),
        Value::List(items) => {
            let inner: Vec<String> = items.iter().map(fmt_value).collect();
            format!("[{}]", inner.join(", "))
        }
        Value::None => "none".to_string(),
        other => format!("{:?}", other),
    }
}

/// Print all available PASTA keywords and commands organized by category.
fn print_keywords() {
    println!("\nPASTA Keywords & Commands:");
    // (content unchanged)
    println!("  :shell             enter the integrated shell");
}
