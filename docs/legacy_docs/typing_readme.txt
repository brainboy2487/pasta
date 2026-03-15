Pasta typing module — wiring and helper notes

This patch centralizes numeric promotion, rounding, and downcast logic into small helpers:
- util.rs: promotion helper and engine-config extraction placeholder.
- operands.rs: compute_numeric_op and apply_round_and_downcast centralize numeric operator behavior.
- float.rs: rounding and formatting helpers used by operands and string formatting.

Integration notes:
- DefaultCoercion carries CoercionConfig; StandardExecutor attempts to downcast engine to DefaultCoercion to read config.
- For other coercion engines, the executor falls back to global float helpers.
- string::to_string currently uses a global display level (2) for formatting floats; consider wiring per-engine display level later.

Dependency:
- Add 'once_cell = "1.18"' to Cargo.toml if not present.

Testing:
- Add unit tests that assert both Value variant and numeric content.
- Test rounding levels 1..5 and division_always_float behavior.

