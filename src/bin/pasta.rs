// src/bin/pasta.rs
//! CLI runner for the PASTA interpreter.
//!
//! Modelled after python/perl: clean output, REPL on no args, -e/-c take the
//! entire remaining command-line as the program string (no quoting required for
//! simple one-liners), and file execution never prints spurious status messages.

use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::process;

use pasta::{init_executor_with_auto_config, lexer::lexer::Lexer, parser::parser::Parser};

// ──────────────────────────────────────────────────────────────────────────────
// Help
// ──────────────────────────────────────────────────────────────────────────────

fn print_usage(prog: &str) {
    eprintln!("Usage: {prog} [OPTIONS] [FILE] [ARGS...]");
    eprintln!("       {prog} -e|-c <CODE>  [ARGS...]");
    eprintln!("       {prog}               (interactive REPL)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  -e, -c <CODE>           Evaluate CODE (all remaining tokens joined)");
    eprintln!("  -i, --repl              Force interactive REPL");
    eprintln!("  -t, --tokens            Print token stream (debug)");
    eprintln!("  -a, --ast               Print parsed AST (debug)");
    eprintln!("  -q, --quiet             Suppress all output except program prints");
    eprintln!("  -v, --verbose           Verbose diagnostics");
    eprintln!("      --verbose-debug      Full diagnostic traceback (super-verbose)");
    eprintln!("  -h, --help              Show this help");
}

fn print_usage_and_exit(prog: &str) -> ! {
    print_usage(prog);
    process::exit(2);
}

// ──────────────────────────────────────────────────────────────────────────────
// Argument parsing
// ──────────────────────────────────────────────────────────────────────────────

struct Args {
    verbose_debug: bool,
    /// Source to evaluate directly (from -e/-c).
    eval_source: Option<String>,
    /// File to run.
    filename: Option<String>,
    /// Extra positional args after the file (available to the program as argv).
    _script_args: Vec<String>,
    show_tokens: bool,
    show_ast: bool,
    quiet: bool,
    force_repl: bool,
    verbose: bool,
    // verbose_debug: bool, // duplicate removed
}

fn parse_args(raw: &[String]) -> Args {
    let prog = raw.get(0).map(|s| s.as_str()).unwrap_or("pasta");

    let mut eval_source: Option<String> = None;
    let mut filename: Option<String> = None;
    let mut script_args: Vec<String> = Vec::new();
    let mut show_tokens = false;
    let mut show_ast = false;
    let mut quiet = false;
    let mut force_repl = false;
    let mut verbose = false;

    let mut verbose_debug = false;
    let mut i = 1usize;

    while i < raw.len() {
        match raw[i].as_str() {
            // -e / -c: join ALL remaining tokens into one source string.
            // This mirrors `perl -e 'stmt1' 'stmt2'` and lets users write:
            //   pasta -e  X=1  PRINT X
            // without any shell quoting.
            "-e" | "-c" | "--eval" => {
                i += 1;
                if i >= raw.len() {
                    eprintln!("Missing argument for {}", raw[i - 1]);
                    print_usage_and_exit(prog);
                }
                // Collect everything remaining; stop at the first flag-like token
                // so that  pasta -e PRINT X -q  still respects -q.
                let mut parts: Vec<&str> = Vec::new();
                while i < raw.len() && !raw[i].starts_with('-') {
                    parts.push(&raw[i]);
                    i += 1;
                }
                eval_source = Some(parts.join(" "));
                // Don't increment i again at the bottom of the loop.
                continue;
            }
            "-i" | "--repl" => {
                force_repl = true;
            }
            "-t" | "--tokens" => {
                show_tokens = true;
            }
            "-a" | "--ast" => {
                show_ast = true;
            }
            "-q" | "--quiet" => {
                quiet = true;
            }
            "-v" | "--verbose" => {
                verbose = true;
            }
            "--verbose-debug" => {
                verbose = true;
                verbose_debug = true;
            }
            "-h" | "--help" => {
                print_usage(prog);
                process::exit(0);
            }
            s if s.starts_with('-') => {
                eprintln!("Unknown option: {s}");
                print_usage_and_exit(prog);
            }
            s => {
                if filename.is_none() {
                    filename = Some(s.to_string());
                } else {
                    // Everything after the filename is passed to the script.
                    script_args.push(s.to_string());
                }
            }
        }
        i += 1;
    }

    Args {
        eval_source,
        filename,
        _script_args: script_args,
        show_tokens,
        show_ast,
        quiet,
        force_repl,
        verbose,
        verbose_debug,
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Lex → parse → execute pipeline
// ──────────────────────────────────────────────────────────────────────────────

fn run_source(source: &str, show_tokens: bool, show_ast: bool, quiet: bool, verbose: bool) -> i32 {
    if verbose {
        eprintln!("[DEBUG] Starting interpreter in verbose mode");
    }
    // Lex
    let tokens =
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| Lexer::new(source).lex())) {
            Ok(t) => t,
            Err(_) => {
                eprintln!("Internal error: lexer panicked.");
                return 5;
            }
        };

    if show_tokens && !quiet {
        eprintln!("--- tokens ---");
        for (idx, tok) in tokens.iter().enumerate() {
            eprintln!("{:04}: {:?}", idx, tok);
        }
        eprintln!("--------------");
    }

    // Parse
    let program = {
        let mut parser = Parser::new(tokens.clone());
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| parser.parse())) {
            Ok(p) => p,
            Err(_) => {
                eprintln!("Internal error: parser panicked.");
                return 6;
            }
        }
    };
    if verbose {
        eprintln!("[DEBUG] AST after parsing:");
        eprintln!("{:#?}", program);
    }

    if show_ast && !quiet {
        eprintln!("--- ast ---");
        eprintln!("{:#?}", program);
        eprintln!("-----------");
    }

    // Execute
    let mut exe = init_executor_with_auto_config();
    if verbose {
        exe.verbose = true;
    }
    if verbose {
        exe.env.debug_print();
    }

    let result = exe.execute_program(&program);

    // Print diagnostics (always to stderr, never to stdout).
    if !exe.diagnostics.is_empty() && !quiet {
        for d in &exe.diagnostics {
            // Skip the auto-configure line in normal operation — it's noise.
            if d.starts_with("Auto-configured device:") {
                continue;
            }
            eprintln!("note: {d}");
        }
    }

    match result {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("Error: {e}");
            // Print constraint / semantic diagnostics that explain the error.
            for d in &exe.diagnostics {
                if d.contains("Constraint") || d.contains("validation") {
                    eprintln!("  {d}");
                }
            }
            7
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Entry point
// ──────────────────────────────────────────────────────────────────────────────

fn main() {
    let raw_args: Vec<String> = env::args().collect();
    let args = parse_args(&raw_args);

    // Set a global flag for verbose mode
    use std::sync::atomic::{AtomicBool, Ordering};
    if args.verbose {
        pasta::VERBOSE_FLAG.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if args.verbose_debug {
        pasta::VERBOSE_DEBUG.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    // ── Determine mode ──────────────────────────────────────────────────────

    // 1. -i / --repl: always open interactive session.
    if args.force_repl {
        if let Err(e) = pasta::interpreter::repl::run_repl() {
            eprintln!("REPL error: {e}");
            process::exit(1);
        }
        process::exit(0);
    }

    // 2. -e / -c: evaluate inline source.
    if let Some(src) = args.eval_source {
        let code = run_source(
            &src,
            args.show_tokens,
            args.show_ast,
            args.quiet,
            args.verbose,
        );
        process::exit(code);
    }

    // 3. FILE argument: run a script file.
    if let Some(fname) = args.filename {
        let src = match fs::read_to_string(&fname) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{fname}: {e}");
                process::exit(3);
            }
        };
        let code = run_source(
            &src,
            args.show_tokens,
            args.show_ast,
            args.quiet,
            args.verbose,
        );
        process::exit(code);
    }

    // 4. No args and stdin is a terminal → interactive REPL (like python / perl -de1).
    if io::stdin().is_terminal() {
        if let Err(e) = pasta::interpreter::repl::run_repl() {
            eprintln!("REPL error: {e}");
            process::exit(1);
        }
        process::exit(0);
    }

    // 5. Stdin is a pipe / redirect → read and execute.
    let mut src = String::new();
    if let Err(e) = io::stdin().read_to_string(&mut src) {
        eprintln!("Failed to read stdin: {e}");
        process::exit(4);
    }
    let code = run_source(
        &src,
        args.show_tokens,
        args.show_ast,
        args.quiet,
        args.verbose,
    );
    process::exit(code);
}
