// src/bin/pasta.rs
//! CLI runner for the PASTA interpreter.
//!
//! Modelled after python/perl: clean output, REPL on no args, -e/-c take the
//! entire remaining command-line as the program string (no quoting required for
//! simple one-liners), and file execution never prints spurious status messages.

use std::env;
use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::Path;
use std::process;

use pasta::{
    init_executor_with_auto_config, lexer::lexer::Lexer, mod_loader, parser::parser::Parser,
};

// ──────────────────────────────────────────────────────────────────────────────
// Stdlib Compilation
// ──────────────────────────────────────────────────────────────────────────────

fn compile_stdlib_to_binary() {
    println!("=== Compiling PASTA stdlib to binary format ===");

    let stdlib_dir = Path::new("src/stdlib");
    if !stdlib_dir.exists() {
        eprintln!("Error: stdlib directory not found at src/stdlib");
        return;
    }

    let mut compiled = 0;
    let mut errors = 0;

    for entry in fs::read_dir(stdlib_dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();

        if path.extension().and_then(|e| e.to_str()) == Some("ph") {
            let _stem = path.file_stem().unwrap().to_string_lossy();
            let output_path = path.with_extension("phb");

            print!("  Compiling {}... ", path.display());

            match mod_loader::compile_file(&path, &output_path) {
                Ok(()) => {
                    println!("OK");
                    compiled += 1;
                }
                Err(e) => {
                    println!("FAILED: {}", e);
                    errors += 1;
                }
            }
        }
    }

    println!();
    println!("=== Summary: {} compiled, {} errors ===", compiled, errors);
}

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
    eprintln!("      --compile-stdlib     Compile stdlib .ph files to binary .phb format");
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
    compile_stdlib: bool,
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
    let mut compile_stdlib = false;

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
            "--compile-stdlib" => {
                compile_stdlib = true;
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
        compile_stdlib,
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
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            parser.parse_with_diagnostics().0
        })) {
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
            // If the error contains a RuntimeError, print its pretty() formatted
            // diagnostic (with source line and traceback) — otherwise fall back
            // to the default anyhow formatting.
            if let Some(re) = e.downcast_ref::<pasta::interpreter::errors::RuntimeError>() {
                eprintln!("{}", re.pretty());
            } else {
                eprintln!("Error: {e}");
            }
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
    // CLI pipeline handling: support quoted pipeline or --spawn-pipeline flag.
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() == 4 && args[1] == "--spawn-pipeline" {
            let left = &args[2];
            let right = &args[3];
            let mut exe = pasta::init_executor_with_auto_config();
            let stages: Vec<&str> = vec![left.as_str(), right.as_str()];
            match exe.spawn_pipeline_from_files(&stages) {
                Ok(_) => {
                    println!("spawned pipeline: {} | {}", left, right);
                    if let Err(e) = exe.enter_shell() {
                        eprintln!("REPL error: {}", e);
                        std::process::exit(1);
                    }
                    std::process::exit(0);
                }
                Err(e) => {
                    eprintln!("failed to spawn pipeline: {}", e);
                    std::process::exit(1);
                }
            }
        }
        if args.len() == 2 && args[1].contains('|') {
            let s = args[1].clone();
            let parts: Vec<&str> = s
                .split('|')
                .map(|p| p.trim())
                .filter(|p| !p.is_empty())
                .collect();
            if parts.len() >= 2 && parts.len() <= 8 {
                let mut exe = pasta::init_executor_with_auto_config();
                match exe.spawn_pipeline_from_files(&parts) {
                    Ok(_) => {
                        println!("spawned pipeline ({}): {}", parts.len(), parts.join(" | "));
                        if let Err(e) = exe.enter_shell() {
                            eprintln!("REPL error: {}", e);
                            std::process::exit(1);
                        }
                        std::process::exit(0);
                    }
                    Err(e) => {
                        eprintln!("failed to spawn pipeline: {}", e);
                        std::process::exit(1);
                    }
                }
            }
        }
    }

    let raw_args: Vec<String> = env::args().collect();
    let args = parse_args(&raw_args);

    // Set a global flag for verbose mode

    if args.verbose {
        pasta::VERBOSE_FLAG.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    if args.verbose_debug {
        pasta::VERBOSE_DEBUG.store(true, std::sync::atomic::Ordering::Relaxed);
    }

    // ── Compile stdlib if requested ─────────────────────────────────────────
    if args.compile_stdlib {
        compile_stdlib_to_binary();
        process::exit(0);
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
