//! src/interpreter/repl.rs
//!
//! Interactive REPL for the PASTA interpreter.
//! Drop-in replacement: clearer structure, robust input accumulation, and
//! integrated meta commands (including :shell which calls Executor::enter_shell).

use std::io::{self, Write};
use anyhow::{anyhow, Result};

use crate::lexer::lexer::Lexer;
use crate::parser::parser::Parser;
use crate::interpreter::executor::Executor;
use crate::interpreter::environment::Value;
use crate::lexer::TokenType;

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


        // Inline pipeline syntax: allow `a.ps | b.ps | c.ps` typed directly at the REPL.
        // Up to 8 stages separated by '|'. Each token must be a non-empty path.
        if trimmed.contains('|') && !trimmed.starts_with(':') && buffer.trim().is_empty() {
            let parts: Vec<&str> = trimmed.split('|').map(|p| p.trim()).collect();
            let valid = parts.len() >= 2 && parts.len() <= 8
                && parts.iter().all(|p| !p.is_empty());
            if valid {
                let display = parts.join(" | ");
                let stage_paths: Vec<&str> = parts.clone();
                match executor.spawn_pipeline_from_files(&stage_paths) {
                    Ok(_) => println!("spawned pipeline ({}): {}", parts.len(), display),
                    Err(e) => println!("failed to spawn pipeline: {}", e),
                }
                buffer.clear();
                indent_depth = 0;
                continue;
            }
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
    println!("PASTA v1.5 — :help for commands, exit to quit");
}

fn read_line(prompt: &str) -> Result<Option<String>> {
    let result = crate::readline::read_line_with_history(prompt)
        .map_err(|e| anyhow!("readline: {}", e))?;
    if let Some(ref line) = result {
        crate::readline::history_push(line);
    }
    // Preserve trailing newline behaviour expected by indent tracking
    Ok(result.map(|s| s + "\n"))
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
    let (program, parse_diags): (_, Vec<crate::parser::parser::ParseError>) = {
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
        let msg = e.to_string();
        // Undefined identifier used as a bare expression (e.g. typing `help`)
        // is demoted to a warning so the REPL keeps running.
        if msg.contains("E2001") || msg.contains("undefined variable") {
            eprintln!("Warning: {}", msg);
        } else {
            eprintln!("Runtime error: {}", msg);
        }
    }

    // ── Drain executor diagnostics ────────────────────────────────────────────
    let drained: Vec<String> = executor.diagnostics.drain(..).collect();
    for d in drained.iter() {
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
            println!("  exit / quit           exit the REPL");
            println!("  :env                  show all variables in scope");
            println!("  :threads              show active threads (THID, name, status, elapsed)");
            println!("  :threads:kill:N       kill thread by THID");
            println!("  :thread-details <N>   show full details for a thread by THID");
            println!("  :keywords             show all available keywords and commands");
            println!("  :reset                reset the interpreter state");
            println!("  :diag                 show and clear pending diagnostics");
            println!("  :clear                clear the screen (ANSI)");
            println!("  :help                 show this help");
            println!("  :pointers             show all live pointers (MEM/FILE/DEV/NET)");
            println!();
            println!("Pipeline syntax (REPL): a.ps | b.ps | c.ps   (up to 8 stages)");
            println!("  First stage: every RETURN sends a value downstream.");
            println!("  Other stages: run once per incoming value; PIPE_IN holds it.");
        }

        ":env" => {
            let vars = executor.env.list_vars();
            if vars.is_empty() {
                println!("(no variables)");
            } else {
                let mut pairs: Vec<(&String, &Value)> = vars.iter().collect();
                pairs.sort_by_key(|(k, _): &(&String, &Value)| k.as_str());
                for (k, v) in pairs {
                    println!("  {} = {}", k, fmt_value(v));
                }
            }
        }

        ":threads" => {
            let rows = crate::threading::thread_api::threads_snapshot();
            if rows.is_empty() {
                println!("(no active threads)");
            } else {
                println!("  {:<8}  {:<32}  {:<12}  {}", "THID", "NAME", "STATUS", "ELAPSED");
                println!("  {}", "-".repeat(70));
                for row in &rows {
                    println!("{}", row);
                }
                println!();
                println!("  use :threads:kill:N to kill a thread by THID");
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
            match executor.enter_shell() {
                Ok(_) => println!("Exited shell."),
                Err(e) => eprintln!("shell error: {}", e),
            }
        }

        ":pointers" => {
            use crate::runtime::pointer::pointer::PointerTarget;
            let ids = executor.pointer_gc_tracker.all_allocations();
            let reg = executor.pointer_registry.read().unwrap();
            let live_n = reg.live_count();
            let total_n = reg.total_count();
            println!("Pointers: {} live, {} total", live_n, total_n);
            let mut printed = 0;
            for id in &ids {
                if let Some(ptr) = reg.lookup(*id) {
                    if !ptr.alive { continue; }
                    let detail = match &ptr.target {
                        PointerTarget::Memory { data, offset } =>
                            format!("MEM  size={} offset={}", data.len(), offset),
                        PointerTarget::File { path, offset, mode } =>
                            format!("FILE path={:?} offset={} mode={}", path, offset, mode),
                        PointerTarget::Device { device_id, device_type } =>
                            format!("DEV  id={} type={}", device_id, device_type),
                        PointerTarget::Network { host, port, stream } =>
                            format!("NET  {}:{} connected={}", host, port, stream.is_some()),
                    };
                    println!("  ptr:{:<4}  {}", ptr.id, detail);
                    printed += 1;
                }
            }
            if printed == 0 {
                println!("(no live pointers)");
            }
        }

        other if other.starts_with(":threads:kill:") => {
            let id_str = &other[":threads:kill:".len()..];
            match id_str.trim().parse::<u64>() {
                Ok(id) => {
                    if crate::threading::thread_api::kill_thread_by_id(id) {
                        println!("Kill signal sent to THID:{}", id);
                    } else {
                        println!("No thread with THID:{} found", id);
                    }
                }
                Err(_) => eprintln!("Usage: :threads:kill:N  where N is a thread ID"),
            }
        }

        other if other.starts_with(":thread-details ") || other.starts_with(":thread-details:") => {
            let id_str = other.trim_start_matches(":thread-details").trim_start_matches(' ').trim_start_matches(':');
            match id_str.trim().parse::<u64>() {
                Ok(id) => {
                    let reg = crate::threading::threads::global_registry();
                    let lock = reg.lock().unwrap();
                    match lock.threads.get(&id) {
                        None => println!("No thread with THID:{} found", id),
                        Some(t) => {
                            println!("Thread THID:{}", t.id);
                            println!("  name:    {}", t.name);
                            println!("  status:  {}", t.status);
                            let elapsed = t.elapsed_ms();
                            if t.ended_at_ms > 0 {
                                println!("  elapsed: {}ms (finished)", elapsed);
                            } else {
                                println!("  elapsed: {}ms (still running)", elapsed);
                            }
                            if let Some(pid) = t.pipeline_id {
                                println!("  pipeline id:    {}", pid);
                                println!("  pipeline stage: {}/{}",
                                    t.pipeline_stage.map(|s| s + 1).unwrap_or(0),
                                    t.pipeline_total.unwrap_or(0));
                            }
                            if let Some(cpid) = t.child_pid {
                                println!("  child PID: {}", cpid);
                            }
                        }
                    }
                }
                Err(_) => eprintln!("Usage: :thread-details <THID>"),
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
    println!("\n╔══════════════════════════════════════════════════════════════════════╗");
    println!("║               PASTA Keywords & Commands (v1.5)                      ║");
    println!("╚══════════════════════════════════════════════════════════════════════╝");
    println!();
    println!("┌─── Control Flow ─────────────────────────────────────────────────────┐");
    println!("│  IF [THEN]            Conditional branch                            │");
    println!("│  OTHERWISE / ELSE     Else branch (also: catch)                     │");
    println!("│  UNLESS               If-not shorthand                               │");
    println!("│  WHILE ... END        Condition loop                                 │");
    println!("│  UNTIL ... END        Loop until condition is true                   │");
    println!("│  FOR x IN list END    Iterate over list                              │");
    println!("│  FOR n TIMES END      Repeat n times                                 │");
    println!("│  BREAK                Exit innermost loop                            │");
    println!("│  CONTINUE             Skip to next iteration                         │");
    println!("│  PASS / noop          No-operation placeholder                       │");
    println!("│  MATCH val WHEN ...   Pattern match                                  │");
    println!("│  GOTO label           Jump to named LOOP label                       │");
    println!("│  LOOP name: ... END   Named loop block (GOTO target)                 │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Variables & Functions ────────────────────────────────────────────┐");
    println!("│  x = expr             Assign variable                                │");
    println!("│  SET / LET / MAKE     Assign aliases                                 │");
    println!("│  CONST name = val     Immutable constant                             │");
    println!("│  DEF name(params) END Define function                                │");
    println!("│  RETURN / RET.NOW()   Return value from function                     │");
    println!("│  RET.LATE(Nms)        Return after delay                             │");
    println!("│  RET.LATE(WHEN fn)    Return when function is called                 │");
    println!("│  YIELD / emit         Yield value (iterator/pipeline)                │");
    println!("│  TYPEOF(x)            Type string of value                           │");
    println!("│  ASSERT cond          Raise if condition is false                    │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── DO Blocks & Threading ────────────────────────────────────────────┐");
    println!("│  DO: ... END          Spawn async task block                         │");
    println!("│  DO AS name END       Named task                                     │");
    println!("│  DO OVER Nms END      Run every N ms                                 │");
    println!("│  DO LIMIT N END       Run at most N times                            │");
    println!("│  PAUSE name           Pause a named task                             │");
    println!("│  UNPAUSE name         Resume a paused task                           │");
    println!("│  RESTART name         Restart a task                                 │");
    println!("│  WAIT name            Wait for task to finish                        │");
    println!("│  GROUP: ... END       Bundle tasks in a group                        │");
    println!("│  SPAWN name(args)     Spawn an object instance                       │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Pipelines ────────────────────────────────────────────────────────┐");
    println!("│  val |> fn            Pipe value into function (expression)          │");
    println!("│  a.ps | b.ps          Script pipeline: RETURN sends downstream       │");
    println!("│  a.ps | b.ps | c.ps   Multi-stage (up to 8); PIPE_IN = incoming val  │");
    println!("│  PIPE_IN              Variable: value received from upstream stage   │");
    println!("│  |   ||   |&|   |:|   Pipe operator variants                        │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Error Handling ───────────────────────────────────────────────────┐");
    println!("│  TRY / attempt        Begin error-guarded block                      │");
    println!("│  OTHERWISE / catch    Handle error                                   │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Modules & Imports ────────────────────────────────────────────────┐");
    println!("│  FROM module: USE name1, name2 END   Import from module              │");
    println!("│  IMPORT / use / require / include    Import aliases                  │");
    println!("│  EXPORT name          Mark symbol as exported                        │");
    println!("│  WITH name AS alias   Alias import                                   │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Classes & Objects ────────────────────────────────────────────────┐");
    println!("│  CLASS name: ... END  Define class                                   │");
    println!("│  OBJ name             Object reference                               │");
    println!("│  SPAWN name(args)     Spawn instance                                 │");
    println!("│  MUT (reserved)       Mutable binding modifier                       │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Pointer System ───────────────────────────────────────────────────┐");
    println!("│  ALLOC.MEM(size)      Allocate memory buffer                         │");
    println!("│  ALLOC.FILE(path)     Open file as pointer                           │");
    println!("│  GOTO ptr: ... END    Set active pointer context                     │");
    println!("│  PUSH.BYTE val        Write byte to active pointer                   │");
    println!("│  PULL.BYTE -> var     Read byte from active pointer                  │");
    println!("│  SEEK ptr, offset     Set pointer offset                             │");
    println!("│  INFO ptr -> var      Get pointer metadata                           │");
    println!("│  FREE ptr             Release pointer                                │");
    println!("│  REF ptr              Create reference to pointer                    │");
    println!("│  SWAP var1, var2      Swap two variables                             │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Operators ────────────────────────────────────────────────────────┐");
    println!("│  Arithmetic : + - * / // % ** ^                                     │");
    println!("│  Comparison : == != < > <= >= ≈ ≠ ≡                                 │");
    println!("│  Logical    : AND OR NOT && ||                                       │");
    println!("│  Assignment : = += -= *= /= %=                                       │");
    println!("│  Other      : -> => .. @ ? :: |> |                                  │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Built-in Functions ───────────────────────────────────────────────┐");
    println!("│  Output    : PRINT, echo, log_info, log_warn, log_error, log_debug  │");
    println!("│  Type conv : str(x), num(x), int(x), bool(x), float(x)             │");
    println!("│  Type check: typeof(x), type_of(x), is_inf(x), is_nan(x)           │");
    println!("│  Strings   : len, upper, lower, trim, split, join, replace          │");
    println!("│              starts_with, ends_with, contains, string_repeat        │");
    println!("│              string_reverse, string_pad_left, string_pad_right      │");
    println!("│              chars_to_string, substr, string_len, string_concat     │");
    println!("│              format_number, format_currency, format_percentage      │");
    println!("│              format_bytes                                            │");
    println!("│  Lists     : list, len, range(n), first, last, idx(lst,i)          │");
    println!("│              list_push, list_pop, list_sort, list_reverse           │");
    println!("│              list_contains, list_flatten, list_slice, list_sum      │");
    println!("│              list_min, list_max, list_avg, list_concat, list_drop   │");
    println!("│              list_take, list_len, distinct_values, zip              │");
    println!("│              collection_empty, collection_fill, collection_merge    │");
    println!("│              collection_pair, collection_single, collection_triple  │");
    println!("│  Math      : abs, sqrt, pow, floor, ceil, round, sign, clamp       │");
    println!("│              min, max, sin, cos, tan, asin, acos, atan, atan2      │");
    println!("│              exp, ln, log, log2, log10, hypot, factorial, gcd, lcm │");
    println!("│              degrees, radians                                        │");
    println!("│  Random    : rand(), rand.int(lo,hi), rand.float, rand.choice      │");
    println!("│              rand.seed, rand.shuffle, rand.sample                   │");
    println!("│  Time      : time_ms(), sleep(ms), time.now, time.now_ns           │");
    println!("│              time.format, time.delta, time.sleep                    │");
    println!("│  Files     : file_read, file_write, file_exists, file_delete        │");
    println!("│              file_size, fs.read, fs.write, fs.append, fs.exists     │");
    println!("│              fs.delete, fs.list, fs.mkdir, fs.rmdir, fs.copy       │");
    println!("│              fs.move, fs.size, fs.touch, fs.basename, fs.dirname   │");
    println!("│              fs.ext, fs.is_file, fs.is_dir, fs.getcwd              │");
    println!("│              dir_create, dir_delete, dir_exists, dir_list          │");
    println!("│  System    : sys.args, sys.env, sys.platform, sys.exit, sys.getcwd │");
    println!("│              stdin_readline, input, exit, env, device.name         │");
    println!("│              device.arch, device.cores, device.ram, device.gpu     │");
    println!("│  Threads   : thread.id, thread.count, thread.join, thread.sleep    │");
    println!("│  Tensor    : tensor, tensor_create_zeros, tensor_create_ones       │");
    println!("│              tensor_rand, tensor_shape, tensor_reshape, tensor_get │");
    println!("│              tensor_add, tensor_sub, tensor_mul, tensor_div        │");
    println!("│              tensor_matmul, tensor_mean, tensor_sum, tensor_flatten│");
    println!("│              tensor_transpose, tensor_to_list, tensor_from_list    │");
    println!("│  Memory    : memory.alloc, memory.free, memory.set, memory.copy    │");
    println!("│              memcpy, memset, memory.size                            │");
    println!("│  Misc      : assert, debug.print, debug.vars, debug.type           │");
    println!("│              debug.len, debug.dump, debug.trace, debug.backtrace   │");
    println!("│              validate_not_empty, validate_range, validate_length   │");
    println!("│              gc.collect, gc.count, gc.stats, gc.resume             │");
    println!("│              import(path), partial(fn, args)                        │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── Graphics (opt-in: FROM graphics: USE ... END) ────────────────────┐");
    println!("│  window_create, window_is_open, window_close, window_poll           │");
    println!("│  window_key, canvas_create, canvas_set_pixel, canvas_get_pixel      │");
    println!("│  canvas_present, canvas_clear, canvas_fill_rect, canvas_draw_rect   │");
    println!("│  canvas_draw_line, canvas_draw_circle, canvas_fill_circle           │");
    println!("│  canvas_draw_ellipse, canvas_fill_ellipse, canvas_draw_triangle     │");
    println!("│  canvas_fill_triangle, canvas_draw_polygon, canvas_fill_polygon     │");
    println!("│  canvas_draw_arc, canvas_blit, canvas_width, canvas_height          │");
    println!("│  canvas_save_ppm, color_rgb, color_rgba, color_hsv, color_lerp      │");
    println!("│  graphics_cleanup                                                    │");
    println!("│  Colors: RED GREEN BLUE WHITE BLACK YELLOW CYAN MAGENTA ORANGE      │");
    println!("│          PINK PURPLE BROWN GRAY LIGHT_GRAY DARK_GRAY LIME TEAL     │");
    println!("│          NAVY MAROON OLIVE SILVER GOLD INDIGO VIOLET CORAL         │");
    println!("│          SALMON KHAKI TURQUOISE LAVENDER BEIGE TRANSPARENT         │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── ML/AI (reserved) ─────────────────────────────────────────────────┐");
    println!("│  LEARN, BUILD, TENSOR                                               │");
    println!("│  ai_relu, ai_softmax, ai_loss_mse, ai_loss_crossentropy            │");
    println!("│  ai_list_to_tensor, ai_tensor_to_list, batch_split                 │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("┌─── REPL Meta Commands ───────────────────────────────────────────────┐");
    println!("│  :help                Show this help                                 │");
    println!("│  :env                 Show all variables in scope                    │");
    println!("│  :threads             Show active threads                            │");
    println!("│  :threads:kill:N      Kill thread by THID                            │");
    println!("│  :thread-details N    Full details for a thread                      │");
    println!("│  :keywords            Show this keyword list                         │");
    println!("│  :reset               Reset interpreter state                        │");
    println!("│  :diag                Show and clear diagnostics                     │");
    println!("│  :clear               Clear screen                                   │");
    println!("│  :shell               Enter integrated shell                         │");
    println!("│  :quit / exit         Exit REPL                                      │");
    println!("└──────────────────────────────────────────────────────────────────────┘");
    println!();
    println!("For full syntax examples, see: docs/pasta_syntax.txt");
}
