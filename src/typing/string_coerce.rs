use crate::typing::types::{TypeId, Value};
use crate::typing::float;

pub fn to_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            // Use global formatting level for now; per-engine display formatting can be added later.
            float::format_with_level(*f, 2) // default display level 2 => 1 decimal place
        }
        Value::Bool(b) => if *b { "TRUE".into() } else { "FALSE".into() },
        Value::Null => "NULL".into(),
        _ => format!("{:?}", value),
    }
}

/// parse_numeric returns (int_candidate, float_candidate) when parsing succeeds.
/// It is intentionally strict: no trailing garbage allowed.
pub fn parse_numeric(s: &str) -> Option<(i64, f64)> {
    if let Ok(i) = s.parse::<i64>() {
        return Some((i, i as f64));
    }
    if let Ok(f) = s.parse::<f64>() {
        return Some((f as i64, f));
    }
    None
}

pub fn type_id() -> TypeId {
    TypeId::String
}
