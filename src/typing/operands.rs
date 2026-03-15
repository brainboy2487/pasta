// src/typing/operands.rs
//!
//! DefaultCoercion engine and StandardExecutor.
//!
//! Type coercion matrix (full):
//!
//!  Left       Right      Op              Result
//!  ─────────────────────────────────────────────────
//!  Int        Int        arith/cmp       Int (or Bool)
//!  Float      Float      arith/cmp       Float (or Bool)
//!  Int        Float      arith/cmp       Float (or Bool)
//!  String     Int/Float  arith           parse string → numeric, then op
//!  Int/Float  String     arith           parse string → numeric, then op
//!  String     String     +               concat
//!  String     String     cmp (<><=>=)    lexicographic
//!  String     Int/Float  cmp (<><=>=)    parse string → float; fallback lex
//!  Int/Float  String     cmp (<><=>=)    parse string → float; fallback lex
//!  Bool       any        cmp/arith       Bool → Int (0/1)
//!  Null       any        cmp             Null → 0
//!  List       any        cmp             List → its length

use crate::typing::types::{
    Coerced, CoercionConfig, CoercionEngine, Operator, OperatorExecutor, TypeError, TypeId, Value,
    FloatRoundingMode,
};
use crate::typing::float;
use crate::typing::util;
use std::any::Any;

// ─────────────────────────────────────────────────────────────────────────────
// DefaultCoercion
// ─────────────────────────────────────────────────────────────────────────────

/// Default coercion engine with per-engine config.
pub struct DefaultCoercion {
    pub cfg: CoercionConfig,
}

impl Default for DefaultCoercion {
    fn default() -> Self {
        Self {
            cfg: CoercionConfig::default(),
        }
    }
}

impl DefaultCoercion {
    fn type_of(value: &Value) -> TypeId {
        match value {
            Value::Bool(_)   => TypeId::Bool,
            Value::Int(_)    => TypeId::Int,
            Value::Float(_)  => TypeId::Float,
            Value::String(_) => TypeId::String,
            Value::Tensor    => TypeId::Tensor,
            Value::Null      => TypeId::Null,
            Value::List(_)   => TypeId::List,
            Value::Map(_)    => TypeId::Map,
            Value::Family(_) => TypeId::Family,
            Value::Custom(id, _) => TypeId::Custom(*id),
        }
    }

    /// Attempt to convert a Value to f64 for numeric operations.
    /// Returns None if the value cannot be meaningfully treated as a number.
    pub fn to_f64(v: &Value) -> Option<f64> {
        match v {
            Value::Int(i)    => Some(*i as f64),
            Value::Float(f)  => Some(*f),
            Value::Bool(b)   => Some(if *b { 1.0 } else { 0.0 }),
            Value::Null      => Some(0.0),
            Value::List(l)   => Some(l.len() as f64),
            Value::String(s) => s.trim().parse::<f64>().ok(),
        }
    }

    /// Attempt to convert a Value to i64 for integer operations.
    pub fn to_i64(v: &Value) -> Option<i64> {
        match v {
            Value::Int(i)    => Some(*i),
            Value::Float(f)  => {
                let r = f.round();
                if (f - r).abs() < 1e-9 && r >= i64::MIN as f64 && r <= i64::MAX as f64 {
                    Some(r as i64)
                } else {
                    None
                }
            }
            Value::Bool(b)   => Some(if *b { 1 } else { 0 }),
            Value::Null      => Some(0),
            Value::String(s) => s.trim().parse::<i64>().ok(),
            Value::List(l)   => Some(l.len() as i64),
        }
    }

    /// Return the string value of v for lexicographic comparison.
    fn to_string_lex(v: &Value) -> String {
        match v {
            Value::String(s) => s.clone(),
            Value::Int(i)    => i.to_string(),
            Value::Float(f)  => format!("{}", f),
            Value::Bool(b)   => b.to_string(),
            Value::Null      => String::new(),
            _ => format!("{:?}", v),
        }
    }

    /// Whether an operator is a comparison that returns Bool.
    fn is_comparison(op: Operator) -> bool {
        matches!(op, Operator::Lt | Operator::Gt | Operator::Le | Operator::Ge | Operator::Eq | Operator::Ne)
    }

    /// Whether an operator is ordering (not equality).
    fn is_ordering(op: Operator) -> bool {
        matches!(op, Operator::Lt | Operator::Gt | Operator::Le | Operator::Ge)
    }
}

impl CoercionEngine for DefaultCoercion {
    fn coerce_for_op(
        &self,
        op: Operator,
        left: Value,
        right: Value,
    ) -> Result<Coerced, TypeError> {
        let lt = Self::type_of(&left);
        let rt = Self::type_of(&right);

        // ── Both numeric ──────────────────────────────────────────────────────
        let both_numeric = matches!(lt, TypeId::Int | TypeId::Float)
            && matches!(rt, TypeId::Int | TypeId::Float);

        if both_numeric {
            let result_type = if lt == TypeId::Float || rt == TypeId::Float {
                TypeId::Float
            } else {
                TypeId::Int
            };
            return Ok(Coerced { left, right, result_type });
        }

        // ── Bool → Int promotion ──────────────────────────────────────────────
        let left = if lt == TypeId::Bool {
            match &left {
                Value::Bool(b) => Value::Int(if *b { 1 } else { 0 }),
                _ => left,
            }
        } else {
            left
        };
        let right = if rt == TypeId::Bool {
            match &right {
                Value::Bool(b) => Value::Int(if *b { 1 } else { 0 }),
                _ => right,
            }
        } else {
            right
        };
        // Refresh types after promotion
        let lt = Self::type_of(&left);
        let rt = Self::type_of(&right);

        // ── Null → 0 for numerics ─────────────────────────────────────────────
        let left = if lt == TypeId::Null && (matches!(rt, TypeId::Int | TypeId::Float) || Self::is_comparison(op)) {
            Value::Int(0)
        } else { left };
        let right = if rt == TypeId::Null && (matches!(lt, TypeId::Int | TypeId::Float) || Self::is_comparison(op)) {
            Value::Int(0)
        } else { right };
        let lt = Self::type_of(&left);
        let rt = Self::type_of(&right);

        // ── String × numeric  ─────────────────────────────────────────────────
        // Strategy:
        //  - For arithmetic (+*-/ etc): parse string as number; error if not numeric
        //  - For ordering comparisons: try numeric; fallback to lexicographic
        //  - For equality: exact match first, then numeric coerce

        let left_is_str  = lt == TypeId::String;
        let right_is_str = rt == TypeId::String;
        let left_is_num  = matches!(lt, TypeId::Int | TypeId::Float);
        let right_is_num = matches!(rt, TypeId::Int | TypeId::Float);

        if left_is_str || right_is_str {
            match op {
                // String concat: both must be strings or coerce RHS to string
                Operator::Add if left_is_str && right_is_str => {
                    return Ok(Coerced { left, right, result_type: TypeId::String });
                }
                // String + number → concat (handled in execute_in_domain)
                Operator::Add if left_is_str && right_is_num => {
                    return Ok(Coerced { left, right, result_type: TypeId::String });
                }
                // Number + string → try numeric add first
                Operator::Add if left_is_num && right_is_str => {
                    if let Some(r) = Self::to_f64(&right) {
                        let promoted = Value::Float(r);
                        let result_type = if lt == TypeId::Float { TypeId::Float } else { TypeId::Float };
                        return Ok(Coerced { left, right: promoted, result_type });
                    }
                    // fallback: stringify left and concat
                    return Ok(Coerced { left, right, result_type: TypeId::String });
                }

                // Arithmetic (sub, mul, div, mod, pow): require numeric parse
                Operator::Sub | Operator::Mul | Operator::Div | Operator::Mod | Operator::Pow => {
                    let lf = Self::to_f64(&left);
                    let rf = Self::to_f64(&right);
                    match (lf, rf) {
                        (Some(lv), Some(rv)) => {
                            return Ok(Coerced {
                                left: Value::Float(lv),
                                right: Value::Float(rv),
                                result_type: TypeId::Float,
                            });
                        }
                        _ => {
                            return Err(TypeError::with_context(
                                format!("Cannot apply {:?} to non-numeric string", op),
                                Some(lt), Some(rt), Some(op),
                            ));
                        }
                    }
                }

                // Ordering comparisons: numeric if parseable, else lexicographic
                Operator::Lt | Operator::Gt | Operator::Le | Operator::Ge => {
                    // Mark with a special sentinel: pass as-is; execute_in_domain handles
                    // the string path via compare_values()
                    return Ok(Coerced {
                        left,
                        right,
                        result_type: TypeId::String, // signals string/mixed compare path
                    });
                }

                // Equality: exact, then numeric coerce
                Operator::Eq | Operator::Ne => {
                    return Ok(Coerced { left, right, result_type: TypeId::String });
                }

                // And/Or: coerce to bool via truthiness
                Operator::And | Operator::Or => {
                    return Ok(Coerced { left, right, result_type: TypeId::Bool });
                }
            }
        }

        // ── List length coercion for comparisons ──────────────────────────────
        if (lt == TypeId::List || rt == TypeId::List) && Self::is_comparison(op) {
            return Ok(Coerced { left, right, result_type: TypeId::Int });
        }

        // ── Same-type passthrough ─────────────────────────────────────────────
        if lt == rt {
            return Ok(Coerced { left, right, result_type: lt });
        }

        // ── Fallback: mixed types not handled above ────────────────────────────
        Err(TypeError::with_context(
            format!("Type mismatch: {:?} {:?} {:?}", lt, op, rt),
            Some(lt), Some(rt), Some(op),
        ))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// StandardExecutor
// ─────────────────────────────────────────────────────────────────────────────

pub struct StandardExecutor<C: CoercionEngine> {
    pub coercion: C,
}

impl<C: CoercionEngine> StandardExecutor<C> {
    pub fn new(coercion: C) -> Self {
        Self { coercion }
    }
}

impl<C: CoercionEngine> OperatorExecutor for StandardExecutor<C> {
    fn apply(
        &self,
        op: Operator,
        left: Value,
        right: Value,
    ) -> Result<Value, TypeError> {
        let coerced = self.coercion.coerce_for_op(op, left, right)?;
        execute_in_domain(&self.coercion, op, coerced)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Domain execution
// ─────────────────────────────────────────────────────────────────────────────

/// Central dispatch: performs the operation in the coerced domain.
fn execute_in_domain<C: CoercionEngine>(
    engine: &C,
    op: Operator,
    c: Coerced,
) -> Result<Value, TypeError> {
    use Operator::*;

    // ── Numeric domain ────────────────────────────────────────────────────────
    if matches!(c.result_type, TypeId::Int | TypeId::Float) {
        return compute_numeric_op(engine, op, &c.left, &c.right, &c.result_type);
    }

    // ── String domain ─────────────────────────────────────────────────────────
    if c.result_type == TypeId::String {
        // String concatenation
        if op == Operator::Add {
            let ls = value_to_display_string(&c.left);
            let rs = value_to_display_string(&c.right);
            return Ok(Value::String(ls + &rs));
        }

        // Comparisons: try numeric first, fall back to lexicographic
        if matches!(op, Lt | Gt | Le | Ge) {
            let lf = DefaultCoercion::to_f64(&c.left);
            let rf = DefaultCoercion::to_f64(&c.right);
            if let (Some(lv), Some(rv)) = (lf, rf) {
                return Ok(Value::Bool(compare_f64(op, lv, rv)));
            }
            // Lexicographic fallback
            let ls = DefaultCoercion::to_string_lex(&c.left);
            let rs = DefaultCoercion::to_string_lex(&c.right);
            return Ok(Value::Bool(compare_str(op, &ls, &rs)));
        }

        // Equality
        if op == Operator::Eq {
            // Exact match first
            if c.left == c.right { return Ok(Value::Bool(true)); }
            // Numeric coerce: "42" == 42.0
            if let (Some(lv), Some(rv)) = (DefaultCoercion::to_f64(&c.left), DefaultCoercion::to_f64(&c.right)) {
                return Ok(Value::Bool((lv - rv).abs() < 1e-12));
            }
            // String compare
            let ls = DefaultCoercion::to_string_lex(&c.left);
            let rs = DefaultCoercion::to_string_lex(&c.right);
            return Ok(Value::Bool(ls == rs));
        }
        if op == Operator::Ne {
            // Invert eq
            if c.left == c.right { return Ok(Value::Bool(false)); }
            if let (Some(lv), Some(rv)) = (DefaultCoercion::to_f64(&c.left), DefaultCoercion::to_f64(&c.right)) {
                return Ok(Value::Bool((lv - rv).abs() >= 1e-12));
            }
            let ls = DefaultCoercion::to_string_lex(&c.left);
            let rs = DefaultCoercion::to_string_lex(&c.right);
            return Ok(Value::Bool(ls != rs));
        }
    }

    // ── Bool domain (And/Or) ──────────────────────────────────────────────────
    if c.result_type == TypeId::Bool {
        let lb = value_is_truthy(&c.left);
        let rb = value_is_truthy(&c.right);
        return match op {
            Operator::And => Ok(Value::Bool(lb && rb)),
            Operator::Or  => Ok(Value::Bool(lb || rb)),
            Operator::Eq  => Ok(Value::Bool(lb == rb)),
            Operator::Ne  => Ok(Value::Bool(lb != rb)),
            _ => Err(TypeError::new(format!("Bool does not support {:?}", op))),
        };
    }

    // ── Int fallback for List/Null comparisons ────────────────────────────────
    if c.result_type == TypeId::Int {
        if let (Some(lv), Some(rv)) = (DefaultCoercion::to_f64(&c.left), DefaultCoercion::to_f64(&c.right)) {
            return Ok(Value::Bool(compare_f64(op, lv, rv)));
        }
    }

    Err(TypeError::new(format!("Operator {:?} not implemented for {:?}", op, c.result_type)))
}

// ─────────────────────────────────────────────────────────────────────────────
// Numeric computation
// ─────────────────────────────────────────────────────────────────────────────

fn compute_numeric_op<C: CoercionEngine>(
    engine: &C,
    op: Operator,
    left: &Value,
    right: &Value,
    result_type: &TypeId,
) -> Result<Value, TypeError> {
    // Int/Int fast path
    if let (Value::Int(a), Value::Int(b)) = (left, right) {
        return match op {
            Operator::Add => Ok(Value::Int(a.wrapping_add(*b))),
            Operator::Sub => Ok(Value::Int(a.wrapping_sub(*b))),
            Operator::Mul => Ok(Value::Int(a.wrapping_mul(*b))),
            Operator::Div => {
                if *b == 0 {
                    Err(TypeError::new("Division by zero"))
                } else {
                    Ok(Value::Int(a / b))
                }
            }
            Operator::Mod => {
                if *b == 0 {
                    Err(TypeError::new("Modulo by zero"))
                } else {
                    Ok(Value::Int(a % b))
                }
            }
            Operator::Lt => Ok(Value::Bool(a < b)),
            Operator::Gt => Ok(Value::Bool(a > b)),
            Operator::Le => Ok(Value::Bool(a <= b)),
            Operator::Ge => Ok(Value::Bool(a >= b)),
            Operator::Eq => Ok(Value::Bool(a == b)),
            Operator::Ne => Ok(Value::Bool(a != b)),
            _ => Err(TypeError::new(format!("Integer does not support {:?}", op))),
        };
    }

    // Float path (promote both to f64)
    if let Some((lf, rf)) = util::promote_pair_to_float(left, right) {
        match op {
            Operator::Lt => return Ok(Value::Bool(lf < rf)),
            Operator::Gt => return Ok(Value::Bool(lf > rf)),
            Operator::Le => return Ok(Value::Bool(lf <= rf)),
            Operator::Ge => return Ok(Value::Bool(lf >= rf)),
            Operator::Eq => return Ok(Value::Bool((lf - rf).abs() < 1e-12)),
            Operator::Ne => return Ok(Value::Bool((lf - rf).abs() >= 1e-12)),
            _ => {}
        }

        let raw = match op {
            Operator::Add => lf + rf,
            Operator::Sub => lf - rf,
            Operator::Mul => lf * rf,
            Operator::Div => {
                if rf == 0.0 { return Err(TypeError::new("Division by zero")); }
                lf / rf
            }
            Operator::Mod => {
                if rf == 0.0 { return Err(TypeError::new("Modulo by zero")); }
                lf % rf
            }
            Operator::Pow => lf.powf(rf),
            _ => return Err(TypeError::new(format!("Float operator {:?} not implemented", op))),
        };

        // Try to extract config for rounding/downcast
        let any_ref = engine as &dyn Any;
        if let Some(dc) = any_ref.downcast_ref::<DefaultCoercion>() {
            return apply_round_and_downcast(raw, &dc.cfg, op);
        }
        // Fallback: global helper
        if let Some(i) = float::float_to_int_if_integral(raw) {
            return Ok(Value::Int(i));
        }
        return Ok(Value::Float(raw));
    }

    Err(TypeError::new("Numeric promotion failed"))
}

/// Apply rounding and downcast according to CoercionConfig.
fn apply_round_and_downcast(res: f64, cfg: &CoercionConfig, op: Operator) -> Result<Value, TypeError> {
    // Division: if always_float, skip downcast
    if op == Operator::Div && cfg.division_always_float {
        let mode = map_rounding_mode(cfg.float_rounding_mode);
        if let Some(rounded) = float::round_with_level(res, cfg.float_tolerance_level, mode) {
            return Ok(Value::Float(rounded));
        }
        return Ok(Value::Float(res));
    }

    let level = cfg.float_tolerance_level;
    let mode = map_rounding_mode(cfg.float_rounding_mode);

    if let Some(rounded) = float::round_with_level(res, level, mode) {
        if level == 1 && cfg.allow_downcast_on_level1 {
            if let Some(i) = float::float_to_int_if_integral_with_eps(rounded, float::get_global_float_downcast_eps()) {
                return Ok(Value::Int(i));
            }
        }
        return Ok(Value::Float(rounded));
    }
    Ok(Value::Float(res))
}

fn map_rounding_mode(m: FloatRoundingMode) -> float::FloatRoundingMode {
    match m {
        FloatRoundingMode::RoundHalfEven         => float::FloatRoundingMode::RoundHalfEven,
        FloatRoundingMode::RoundHalfAwayFromZero => float::FloatRoundingMode::RoundHalfAwayFromZero,
        FloatRoundingMode::Truncate              => float::FloatRoundingMode::Truncate,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Comparison helpers
// ─────────────────────────────────────────────────────────────────────────────

fn compare_f64(op: Operator, a: f64, b: f64) -> bool {
    match op {
        Operator::Lt => a < b,
        Operator::Gt => a > b,
        Operator::Le => a <= b,
        Operator::Ge => a >= b,
        Operator::Eq => (a - b).abs() < 1e-12,
        Operator::Ne => (a - b).abs() >= 1e-12,
        _ => false,
    }
}

fn compare_str(op: Operator, a: &str, b: &str) -> bool {
    match op {
        Operator::Lt => a < b,
        Operator::Gt => a > b,
        Operator::Le => a <= b,
        Operator::Ge => a >= b,
        Operator::Eq => a == b,
        Operator::Ne => a != b,
        _ => false,
    }
}

fn value_is_truthy(v: &Value) -> bool {
    match v {
        Value::Bool(b)   => *b,
        Value::Int(i)    => *i != 0,
        Value::Float(f)  => *f != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Null      => false,
        Value::List(l)   => !l.is_empty(),
        _                => true,
    }
}

fn value_to_display_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Int(i)    => i.to_string(),
        Value::Float(f)  => {
            if f.fract() == 0.0 && f.abs() < 1e15 { format!("{}", *f as i64) }
            else { format!("{}", f) }
        }
        Value::Bool(b)   => b.to_string(),
        Value::Null      => String::new(),
        _ => format!("{:?}", v),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::typing::types::{Operator, Value};

    fn exec(op: Operator, l: Value, r: Value) -> Value {
        StandardExecutor::new(DefaultCoercion::default()).apply(op, l, r).unwrap()
    }

    fn exec_err(op: Operator, l: Value, r: Value) -> bool {
        StandardExecutor::new(DefaultCoercion::default()).apply(op, l, r).is_err()
    }

    // ── Numeric ───────────────────────────────────────────────────────────────
    #[test] fn int_add()     { assert_eq!(exec(Operator::Add, Value::Int(2), Value::Int(3)), Value::Int(5)); }
    #[test] fn float_add()   { assert!(matches!(exec(Operator::Add, Value::Float(1.5), Value::Float(1.5)), Value::Int(3) | Value::Float(_))); }
    #[test] fn int_cmp_gt()  { assert_eq!(exec(Operator::Gt, Value::Int(5), Value::Int(3)), Value::Bool(true)); }
    #[test] fn float_cmp_lt(){ assert_eq!(exec(Operator::Lt, Value::Float(2.0), Value::Float(3.0)), Value::Bool(true)); }

    // ── String vs String ──────────────────────────────────────────────────────
    #[test] fn str_concat()  { assert_eq!(exec(Operator::Add, Value::String("a".into()), Value::String("b".into())), Value::String("ab".into())); }
    #[test] fn str_cmp_gt()  { assert_eq!(exec(Operator::Gt,  Value::String("b".into()), Value::String("a".into())), Value::Bool(true)); }
    #[test] fn str_cmp_lt()  { assert_eq!(exec(Operator::Lt,  Value::String("abc".into()), Value::String("abd".into())), Value::Bool(true)); }
    #[test] fn str_eq()      { assert_eq!(exec(Operator::Eq,  Value::String("hi".into()), Value::String("hi".into())), Value::Bool(true)); }

    // ── String vs Int (the key new capability) ────────────────────────────────
    #[test] fn str_int_gt_numeric() {
        // "42" > 10 → numeric: 42 > 10 → true
        assert_eq!(exec(Operator::Gt, Value::String("42".into()), Value::Int(10)), Value::Bool(true));
    }
    #[test] fn str_int_lt_numeric() {
        // "5" < 10 → numeric: 5 < 10 → true
        assert_eq!(exec(Operator::Lt, Value::String("5".into()), Value::Int(10)), Value::Bool(true));
    }
    #[test] fn int_str_gt_numeric() {
        // 100 > "99" → numeric: 100 > 99 → true
        assert_eq!(exec(Operator::Gt, Value::Int(100), Value::String("99".into())), Value::Bool(true));
    }
    #[test] fn str_int_add_numeric() {
        // "3" + 4 → 7.0 (numeric coerce)
        let r = exec(Operator::Add, Value::Int(3), Value::String("4".into()));
        assert!(matches!(r, Value::Int(7) | Value::Float(f) if (f - 7.0).abs() < 1e-9));
    }
    #[test] fn str_int_lex_fallback() {
        // "banana" > 10 → lexicographic: "banana" > "10" → true ('b' > '1')
        assert_eq!(exec(Operator::Gt, Value::String("banana".into()), Value::Int(10)), Value::Bool(true));
    }

    // ── Bool coercion ─────────────────────────────────────────────────────────
    #[test] fn bool_add_int() {
        let r = exec(Operator::Add, Value::Bool(true), Value::Int(5));
        assert!(matches!(r, Value::Int(6) | Value::Float(f) if (f - 6.0).abs() < 1e-9));
    }
    #[test] fn bool_gt_int() {
        assert_eq!(exec(Operator::Gt, Value::Bool(true), Value::Int(0)), Value::Bool(true));
    }

    // ── Null coercion ─────────────────────────────────────────────────────────
    #[test] fn null_lt_int() {
        assert_eq!(exec(Operator::Lt, Value::Null, Value::Int(1)), Value::Bool(true));
    }
    #[test] fn null_eq_zero() {
        // Null == 0: after Null→0 promotion, 0 == 0
        let r = exec(Operator::Eq, Value::Null, Value::Int(0));
        assert_eq!(r, Value::Bool(true));
    }

    // ── Division always float cfg ─────────────────────────────────────────────
    #[test] fn div_always_float() {
        let mut eng = DefaultCoercion::default();
        eng.cfg.division_always_float = true;
        let r = StandardExecutor::new(eng).apply(Operator::Div, Value::Int(10), Value::Int(2)).unwrap();
        assert!(matches!(r, Value::Float(_)));
    }
}
