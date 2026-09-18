use crate::typing::types::{TypeId, Value};

pub fn to_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        // Future: optional truthiness rules for Int/Float/String
        _ => None,
    }
}

pub fn type_id() -> TypeId {
    TypeId::Bool
}
