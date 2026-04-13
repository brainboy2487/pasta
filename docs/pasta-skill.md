# Pasta language skill

Use this document as a model-agnostic guide for writing valid Pasta programs against the current runtime in this repository.

## Primary rules

1. Prefer `name = expr` for assignment. `SET`, `LET`, and `MAKE` are aliases, but plain `=` is the safest default.
2. Define functions with `DEF name(args): ... END`.
3. Prefer `RET.NOW(): value` or `RETURN value` for returns. Treat `RET.LATE` as experimental and avoid it unless explicitly requested.
4. Prefer colon-and-indent blocks for functions and general readability.
5. Brace blocks are valid for `IF`, `UNLESS`, `FOR`, `WHILE`, and `TRY` / `OTHERWISE`. `TRY` brace form still ends with `END`.
6. Use uppercase control keywords (`DEF`, `IF`, `FOR`, `TRY`, `PRINT`) to match the project examples.
7. Lists use `[]`, dictionaries use `{"key": value}`, and indexing uses `value[index]`.
8. Many builtins are snake_case (`list_append`, `dict_get`, `canvas_get_pixel`) even when statement keywords are uppercase.
9. Use headless `CANVAS(...)` for non-interactive graphics scripts. Use `WINDOW(...)` only for interactive X11 flows.
10. Do not generate `GLOB.DEF`; it is planned but not implemented.

## Verified syntax patterns

### Functions

```pasta
DEF add(a, b):
    RET.NOW(): a + b
END

PRINT add(2, 3)
```

### Conditionals and loops

```pasta
total = 0
FOR n IN [1, 2, 3]:
    IF n == 2:
        total = total + 10
    OTHERWISE:
        total = total + n
    END
END
PRINT total
```

Brace style is also valid:

```pasta
FOR n IN [1, 2, 3] {
    IF n == 2 {
        PRINT "two"
    } OTHERWISE {
        PRINT n
    }
}
```

### Error handling

```pasta
TRY:
    PRINT risky_call()
OTHERWISE:
    PRINT "caught error"
END
```

Brace form:

```pasta
TRY {
    PRINT "work"
} OTHERWISE {
    PRINT "fallback"
} END
```

### Collections

```pasta
items = []
list_append(items, "a")
list_append(items, "b")

record = {"left": items[0], "right": items[1]}
PRINT dict_get(record, "left") + ":" + dict_get(record, "right")
```

### Graphics without a window

```pasta
c = CANVAS(4, 2)
DRAW_GRID(c, 2, 1)
DRAW_GRID_RUNS(c, [
    [1, 0, 1, 4294967295]
])
PRINT canvas_get_pixel(c, 2, 0)
```

## Generation guidance

- Keep scripts direct and imperative.
- Use one `END` per colon-style block.
- When mixing styles, prefer brace blocks only for short control-flow blocks.
- Use current helper names exactly as they appear in examples and tests:
  - `list_append`, `list_len`, `list_slice`
  - `dict_get`, `dict_set`
  - `CANVAS`, `WINDOW`, `SET_DRAW_TARGET`, `DRAW_GRID`, `DRAW_GRID_BATCH`, `DRAW_GRID_RUNS`
  - `canvas_fill_rect`, `canvas_get_pixel`, `SWAP_BUFFER`
- For colors, integer literals and `color(...)` helpers are both acceptable.
- For game logic, prefer batching or run-based drawing instead of per-cell draw calls.
- Keep scripts headless when possible so they run in CI and test environments.

## Avoid these traps

- Do not assume Python syntax works unchanged.
- Do not use function-brace bodies unless parser support is explicitly confirmed for that surface.
- Do not rely on outdated import examples that use deeply nested `FROM:` blocks unless the exact project needs them.
- Do not use `RET.LATE` for ordinary function returns.
- Do not assume lowercase keywords in generated examples, even though aliases exist.
