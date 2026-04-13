# PASTA — Programming And Scripting Tool for Automation

> **Version 1.6.1** · Scripting Language Interpreter written in Rust

[![Build](https://img.shields.io/badge/build-passing-brightgreen)](#)
[![Tests](https://img.shields.io/badge/tests-layered-brightgreen)](#building--testing)
[![Language](https://img.shields.io/badge/language-Rust-orange)](https://www.rust-lang.org/)
[![Version](https://img.shields.io/badge/version-1.6.1-blue)](#)
[![License](https://img.shields.io/badge/license-MIT-green)](#)

**PASTA** is a domain-specific scripting language and interpreter written in Rust — clean Python-like syntax with a Rust-backed execution model, native 2D graphics, threading, pipelines, and a full interactive shell.

## Quick Start

```bash
git clone https://github.com/brainboy2487/pasta.git
cd pasta
cargo build --release
./target/release/pasta          # interactive REPL
./target/release/pasta my_script.ps
```

## Example

```pasta
DEF greet(name):
    PRINT "Hello, " + name + "!"
END

greet("World")

FOR item IN ["apple", "banana", "cherry"]:
    PRINT item
END

TRY:
    x = 1 / 0
OTHERWISE:
    PRINT "Caught division by zero"
END
```

**Graphics (opt-in):**

```pasta
FROM:
    graphics
        use
            canvas_create, canvas_fill_rect, canvas_save_ppm, RED
        END
    END
END

c = canvas_create(320, 240)
canvas_fill_rect(c, 40, 40, 120, 80, RED)
canvas_save_ppm(c, "out.ppm")
```

**Pipelines:**

```pasta
# producer.ps — sends values downstream
FOR i IN range(1, 6):
    RETURN i
END

# double.ps — receives each value, doubles it
PRINT PIPE_IN * 2
```
```bash
pasta> producer.ps | double.ps
```

## Features

- **Clean syntax** — colon/indent or brace blocks, `DEF`/`DO`, `IF`/`OTHERWISE`, `WHILE`, `FOR IN`
- **Script pipelines** — `a.ps | b.ps | c.ps` up to 8 stages; `PIPE_IN` receives upstream values; `RETURN` sends downstream
- **`|>` pipe operator** — `value |> function` expression-level piping
- **Opt-in 2D graphics** — block `FROM graphics: use ... END`; lines, rects, circles, ellipses, triangles, polygons, arcs; 32 named color constants + `color(r,g,b)` function; `COLOR_GREY`; PPM/X11 output; live game loop via `WINDOW`, `window_poll`, `SWAP_BUFFER`, `fps_init/begin/end/tick`
- **Threading** — `DO: ... END` async blocks; `:threads`, `:threads:kill:N`, `:thread-details N` in REPL
- **Integrated shell** — full VFS shell with quote parsing, IO redirect (`>`, `>>`, `<`), env var expansion (`$VAR`), glob (`*`, `?`, `[...]`), pipes, and builtins (`echo`, `grep`, `wc`, `sort`, `uniq`, `head`, `tail`, `find`)
- **Module imports** — block `FROM: module use name END END`; stdlib module registry; `PASTA_MODULE_PATH` support
- **Pointer system** — `ALLOC.MEM/FILE/DEV/NET`, `GOTO`/`PULL`/`PUSH`, `SEEK`, `INFO`, `FREE`, `REF`
- **Family Object System** — `OBJ.FAM` dual-parent lineage objects
- **Error handling** — `TRY`/`OTHERWISE` with error binding; 60+ structured error codes E0xxx–E9xxx
- **Rich builtins** — 100+ built-in functions: math, strings, lists, files, time, random, tensors, system; `range(n)` / `range(s,e)` generate inclusive integer ranges; `LIST_CONTAINS(list, val)` membership test
- **Comprehensive File I/O** — `READ_FILE`, `WRITE_FILE`, `APPEND_FILE`, `READ_LINES`, `WRITE_LINES`, `DELETE_FILE`, `RENAME_FILE`, `COPY_FILE`, `FILE_EXISTS`, `FILE_SIZE`; handle-based streaming: `OPEN`/`CLOSE`/`READLINE`/`WRITELINE`/`READ_ALL`
- **Dictionaries** — `DICT(k,v,...)`, dict literal `{"k": v}`, `dict_get`, `dict_set`, `dict_has`, `dict_keys`, `dict_values`, `dict_items`, `dict_delete`, `dict_len`; bracket read access `d["key"]`
- **User I/O & env** — `INPUT("prompt")` reads stdin; `GETENV`/`SETENV` for environment variables; `ARGV()` returns command-line arguments
- **JSON** — `JSON_PARSE(str)` → dict/list/number/bool/none; `JSON_STRINGIFY(val)` and `JSON_STRINGIFY(val, true)` (pretty)
- **Extended String Utilities** — `contains`, `trim_left`, `trim_right`, `replace.all`, `replace.first`, `replace.last`, `index_of` (1-based, 0=not found), `count_occurrences`, `pad_left(str, width[, char])`, `pad_right`, `substr(str, start[, len])`; **Regex**: `regex_match`, `regex_find`, `regex_replace`, `regex_captures` (use `{{n}}` for literal `{n}` quantifiers in patterns)

## Keywords (v1.6.1)

**Control:** `IF` `OTHERWISE` `UNLESS` `WHILE` `UNTIL` `FOR IN` `MATCH` `WHEN` `BREAK` `CONTINUE` `PASS` `GOTO` `LOOP`  
**Scope modifiers:** `UNBIND_SCOPE` `BIND_SCOPE` *(on `WHILE`/`FOR`/`IF` blocks)*  
**Functions:** `DEF` `RETURN` `RET.NOW` `RET.LATE` `YIELD` `LAMBDA` `CONST` `TYPEOF` `ASSERT`  
**Blocks:** `DO` `END` `TRY` `ATTEMPT` `GROUP` `THEN`  
**Threading:** `PAUSE` `UNPAUSE` `RESTART` `WAIT` `AS` `OVER` `LIMIT`  
**Modules:** `FROM` `USE` `EXPORT` `WITH` `IMPORT`  
**Pointers:** `ALLOC` `FREE` `GOTO` `PULL` `PUSH` `SEEK` `INFO` `REF` `SWAP`  
**Objects:** `CLASS` `OBJ` `SPAWN` `MUT`  
**Operators:** `AND` `OR` `NOT` `|>` `|` `->` `=>` `::` `..`

### v1.6.1 Release Highlights

| Feature | Description |
|---------|-------------|
| `color(r, g, b)` | Pack RGB bytes into a color value (`0xFFRRGGBB`) |
| `COLOR_GREY` | New named color constant |
| `range(n)` | Returns `[1..n]` inclusive integer list |
| `range(s, e)` | Returns `[s..e]` inclusive integer list |
| `LIST_CONTAINS(list, val)` | Returns `true` if `val` is in `list` |
| `{"k": v, ...}` | Dict literal syntax |
| `WHILE(UNBIND_SCOPE) ... END` | Block-scoped WHILE — vars die at end |
| `WHILE(BIND_SCOPE) ... END` | Block-scoped WHILE — vars hoisted to enclosing function/global |
| `FOR(UNBIND_SCOPE)` / `IF(UNBIND_SCOPE)` | Same scope modifiers for FOR and IF |

## Building & Testing

```bash
cargo build                        # debug build
cargo build --release              # release build
cargo build --release --features x11   # with live X11 window support
cargo test --workspace --all       # full Rust suite
python3 tools/devkit/tasks.py smoke
python3 tools/devkit/tasks.py golden-check
sudo cp target/release/pasta /usr/local/bin/pasta
```

## REPL Commands

| Command | Description |
|---------|-------------|
| `:help` | Show all commands |
| `:env` | Dump current variable scope |
| `:threads` | List active threads |
| `:threads:kill:N` | Kill thread by ID |
| `:thread-details N` | Full thread info (pipeline stage, elapsed, etc.) |
| `:keywords` | Full keyword and builtin reference |
| `:shell` | Enter the integrated VFS shell |
| `:reset` | Reset interpreter state |
| `:diag` | Show and clear diagnostics |
| `exit` / `quit` | Exit |

## Full Documentation

See [`docs/README.md`](docs/README.md) for the complete language reference, built-in functions, graphics subsystem, shell architecture, pipeline system, and full changelog.

---

*PASTA v1.6.1 — Built with ❤️ in Rust*
