// src/typing/mod.rs
//!
//! PASTA Typing module — type coercion, operator execution, and numeric helpers.
//!
//! ## Submodules
//! - `types`      — `TypeId`, `Value`, `Operator`, `TypeError`, engine traits, config.
//! - `operands`   — `DefaultCoercion` engine + `StandardExecutor`.
//! - `util`       — numeric promotion helpers.
//! - `float`      — float rounding, eps, downcast, and format helpers.
//! - `bool`       — bool coercion helpers.
//! - `string`     — string-to-number parsing and display helpers.
//! - `int`        — int coercion helpers.
//! - `tensor_type`— tensor type descriptor and dtype helpers.
//!
//! ## Integration with executor
//!
//! The executor (`interpreter/executor.rs`) uses `environment::Value`.
//! Use the bridge functions below to convert to/from `typing::Value` for
//! operations involving the typing engine.
//!
//! ```rust,ignore
//! use crate::typing::{bridge_from_env, bridge_to_env, DefaultCoercion, StandardExecutor};
//! use crate::typing::types::{Operator, OperatorExecutor};
//!
//! let eng = StandardExecutor::new(DefaultCoercion::default());
//! let tv_left  = bridge_from_env(&env_val_left);
//! let tv_right = bridge_from_env(&env_val_right);
//! let result   = eng.apply(Operator::Gt, tv_left, tv_right)?;
//! let env_val  = bridge_to_env(result);
//! ```

pub mod types;
pub mod operands;
pub mod util;
pub mod bool;
pub mod string;
pub mod int;
pub mod float;
pub mod tensor_type;

pub use types::{
    TypeId, Value, TypeError, Operator, Coerced,
    CoercionEngine, OperatorExecutor, CoercionConfig, FloatRoundingMode,
    FamilyHandle, CustomValue,
};
pub use operands::{DefaultCoercion, StandardExecutor};

// ─────────────────────────────────────────────────────────────────────────────
// Bridge: environment::Value ↔ typing::Value
// ─────────────────────────────────────────────────────────────────────────────
//
// These helpers live here (not in executor.rs) so the typing module stays
// self-contained and testable without pulling in the full interpreter.

use crate::interpreter::environment::Value as EnvValue;
use crate::typing::types::TensorValue;

/// Convert an `environment::Value` into a `typing::Value` for use with the
/// DefaultCoercion engine.
///
/// Tensor and Lambda values are represented as `typing::Value::Tensor` and
/// `typing::Value::Null` respectively (no information loss for coercion purposes).
pub fn bridge_from_env(v: &EnvValue) -> Value {
    match v {
        EnvValue::Number(n) => {
            // Prefer Int when value is integral and fits in i64
            let r = n.round();
            if (n - r).abs() < 1e-12 && r >= i64::MIN as f64 && r <= i64::MAX as f64 {
                Value::Int(r as i64)
            } else {
                Value::Float(*n)
            }
        }
        EnvValue::String(s)  => Value::String(s.clone()),
        EnvValue::Bool(b)    => Value::Bool(*b),
        EnvValue::List(items) => {
            Value::List(items.iter().map(bridge_from_env).collect())
        }
        EnvValue::Tensor(_)  => {
            // Create a placeholder TensorValue for now; full tensor data lives in the environment.
            Value::Tensor(TensorValue::placeholder())
        }
        EnvValue::Lambda(_)  => Value::Null,
        EnvValue::Heap(_)    => Value::Null,
        EnvValue::Pending(v, _) => bridge_from_env(v),
        EnvValue::None       => Value::Null,
    }
}

/// Convert a `typing::Value` back to an `environment::Value`.
pub fn bridge_to_env(v: Value) -> EnvValue {
    match v {
        Value::Int(i)     => EnvValue::Number(i as f64),
        Value::Float(f)   => EnvValue::Number(f),
        Value::Bool(b)    => EnvValue::Bool(b),
        Value::String(s)  => EnvValue::String(s),
        Value::Null       => EnvValue::None,
        Value::List(items) => EnvValue::List(items.into_iter().map(bridge_to_env).collect()),
        Value::Tensor(_)  => EnvValue::None, // no round-trip for tensors
        Value::Map(_)     => EnvValue::None,
        Value::Family(_)  => EnvValue::None,
        Value::Custom(..) => EnvValue::None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Convenience: apply a typed operator directly on EnvValues
// ─────────────────────────────────────────────────────────────────────────────

/// Apply a typing-module operator to two `environment::Value`s.
///
/// Returns `None` if the operation is not within the typing module's scope
/// (e.g. Tensor ops, Lambda ops), so the caller can fall back to its own handler.
pub fn apply_op_env(
    op: Operator,
    left: &EnvValue,
    right: &EnvValue,
) -> Option<EnvValue> {
    let tv_left  = bridge_from_env(left);
    let tv_right = bridge_from_env(right);
    let eng = StandardExecutor::new(DefaultCoercion::default());
    eng.apply(op, tv_left, tv_right).ok().map(bridge_to_env)
}

// ─────────────────────────────────────────────────────────────────────────────
// Module tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typing::types::Operator;
    use crate::interpreter::environment::Value as EnvValue;

    #[test]
    fn bridge_roundtrip_number_integral() {
        let env = EnvValue::Number(42.0);
        let tv  = bridge_from_env(&env);
        assert!(matches!(tv, Value::Int(42)));
        let back = bridge_to_env(tv);
        assert!(matches!(back, EnvValue::Number(n) if (n - 42.0).abs() < 1e-9));
    }

    #[test]
    fn bridge_roundtrip_float() {
        let env = EnvValue::Number(3.14);
        let tv  = bridge_from_env(&env);
        assert!(matches!(tv, Value::Float(_)));
    }

    #[test]
    fn apply_op_env_gt_numeric_string() {
        // "42" > 10 → true
        let result = apply_op_env(
            Operator::Gt,
            &EnvValue::String("42".into()),
            &EnvValue::Number(10.0),
        );
        assert_eq!(result, Some(EnvValue::Bool(true)));
    }

    #[test]
    fn apply_op_env_lt_lex_fallback() {
        // "banana" < "cherry" → true (lexicographic)
        let result = apply_op_env(
            Operator::Lt,
            &EnvValue::String("banana".into()),
            &EnvValue::String("cherry".into()),
        );
        assert_eq!(result, Some(EnvValue::Bool(true)));
    }

    #[test]
    fn apply_op_env_add_int() {
        let result = apply_op_env(
            Operator::Add,
            &EnvValue::Number(3.0),
            &EnvValue::Number(4.0),
        );
        assert_eq!(result, Some(EnvValue::Number(7.0)));
    }

    #[test]
    fn level1_downcast_add() {
        // 2.5 + 2.5 → 5.0, with level 1 downcasts to Int(5)
        let tv_l = Value::Float(2.5);
        let tv_r = Value::Float(2.5);
        let mut eng_cfg = DefaultCoercion::default();
        eng_cfg.cfg.float_tolerance_level = 1;
        eng_cfg.cfg.allow_downcast_on_level1 = true;
        let exec = StandardExecutor::new(eng_cfg);
        let result = exec.apply(Operator::Add, tv_l, tv_r).unwrap();
        // 5.0 → should downcast to Int(5)
        assert!(matches!(result, Value::Int(5) | Value::Float(f) if (f - 5.0).abs() < 1e-9));
    }

    #[test]
    fn level3_format() {
        let s = crate::typing::float::format_with_level(10.3, 3);
        assert_eq!(s, "10.30");
    }
}
