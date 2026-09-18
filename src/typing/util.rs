// src/typing/util.rs
//!
//! Numeric promotion helpers for the PASTA typing engine.

use crate::typing::types::{TypeId, Value};

/// Promote a numeric pair to (f64, f64) when at least one side is Float.
/// Returns None for Int/Int — caller should handle the integer fast-path.
pub fn promote_pair_to_float(l: &Value, r: &Value) -> Option<(f64, f64)> {
    match (l, r) {
        (Value::Int(_),   Value::Int(_))   => None,
        (Value::Int(a),   Value::Float(b)) => Some((*a as f64, *b)),
        (Value::Float(a), Value::Int(b))   => Some((*a, *b as f64)),
        (Value::Float(a), Value::Float(b)) => Some((*a, *b)),
        _ => None,
    }
}

/// Determine the TypeId of a Value.
pub fn type_of(v: &Value) -> TypeId {
    match v {
        Value::Bool(_)      => TypeId::Bool,
        Value::Int(_)       => TypeId::Int,
        Value::Float(_)     => TypeId::Float,
        Value::String(_)    => TypeId::String,
        Value::Tensor(_)    => TypeId::Tensor,
        Value::Null         => TypeId::Null,
        Value::List(_)      => TypeId::List,
        Value::Map(_)       => TypeId::Map,
        Value::Family(_)    => TypeId::Family,
        Value::Custom(id,_) => TypeId::Custom(*id),
    }
}

/// Attempt to read a numeric f64 from any scalar value.
/// Returns None only for compound or opaque types.
pub fn to_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Int(i)    => Some(*i as f64),
        Value::Float(f)  => Some(*f),
        Value::Bool(b)   => Some(if *b { 1.0 } else { 0.0 }),
        Value::Null      => Some(0.0),
        Value::List(l)   => Some(l.len() as f64),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

/// Promote a value to Bool via truthiness rules.
pub fn to_bool(v: &Value) -> bool {
    match v {
        Value::Bool(b)   => *b,
        Value::Int(i)    => *i != 0,
        Value::Float(f)  => *f != 0.0,
        Value::String(s) => !s.is_empty(),
        Value::Null      => false,
        Value::List(l)   => !l.is_empty(),
        _ => true,
    }
}

/// Determine the promotion result type for a numeric pair.
/// Int × Int → Int; anything × Float → Float.
pub fn numeric_result_type(lt: TypeId, rt: TypeId) -> TypeId {
    if lt == TypeId::Float || rt == TypeId::Float {
        TypeId::Float
    } else {
        TypeId::Int
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn promote_int_float() {
        let l = Value::Int(2);
        let r = Value::Float(3.5);
        assert_eq!(promote_pair_to_float(&l, &r), Some((2.0, 3.5)));
    }

    #[test]
    fn promote_int_int_is_none() {
        let l = Value::Int(2);
        let r = Value::Int(3);
        assert!(promote_pair_to_float(&l, &r).is_none());
    }

    #[test]
    fn to_f64_string_numeric() {
        assert_eq!(to_f64(&Value::String("3.14".into())), Some(3.14));
    }

    #[test]
    fn to_f64_string_non_numeric() {
        assert_eq!(to_f64(&Value::String("hello".into())), None);
    }

    #[test]
    fn to_bool_truthy() {
        assert!(to_bool(&Value::Int(1)));
        assert!(!to_bool(&Value::Int(0)));
        assert!(to_bool(&Value::String("x".into())));
        assert!(!to_bool(&Value::String("".into())));
    }

    #[test]
    fn numeric_result_type_mixed() {
        assert_eq!(numeric_result_type(TypeId::Int, TypeId::Float), TypeId::Float);
        assert_eq!(numeric_result_type(TypeId::Int, TypeId::Int), TypeId::Int);
    }
}
