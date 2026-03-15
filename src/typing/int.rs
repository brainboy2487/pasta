use crate::typing::types::{TypeId, Value};

pub fn to_int(value: &Value) -> Option<i64> {
    match value {
        Value::Int(i) => Some(*i),
        Value::Bool(b) => Some(if *b { 1 } else { 0 }),
        Value::Float(f) => Some(*f as i64),
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

pub fn type_id() -> TypeId {
    TypeId::Int
}
