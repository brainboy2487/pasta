// src/typing/types.rs
//!
//! Core type identifiers, Value, and coercion infrastructure for the PASTA typing system.
//!
//! This module defines the canonical type lattice. The executor (interpreter/executor.rs)
//! uses its own `environment::Value`; the typing module has its own richer `Value` used
//! by the standalone `DefaultCoercion` engine. A bridge `typing_value_from_env` / `typing_value_to_env` pair in the executor
//! converts between the two for cross-module operations.

use std::collections::BTreeMap;
use std::sync::Arc;

// ─────────────────────────────────────────────────────────────────────────────
// Shape and DType
// ─────────────────────────────────────────────────────────────────────────────

/// Tensor shape alias.
pub type Shape = Vec<usize>;

/// Element data types for tensors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DType {
    Int64,
    Float64,
    Bool,
    Utf8,
    /// Reference to an object family by id.
    ObjectRef(u64),
    Custom(u16),
}

// ─────────────────────────────────────────────────────────────────────────────
// Float tolerance struct (per-tensor or per-engine override)
// ─────────────────────────────────────────────────────────────────────────────

/// Float rounding/downcast configuration that can be stored per-tensor or per-engine.
#[derive(Debug, Clone, Copy)]
pub struct FloatTolerance {
    /// Level 1..=20. Level 1 -> 0 decimals; Level N -> N-1 decimals.
    pub level: u8,
    /// Rounding mode.
    pub rounding: FloatRoundingMode,
    /// Whether level 1 rounded integral results may be downcast to Int.
    pub allow_downcast_on_level1: bool,
}

impl Default for FloatTolerance {
    fn default() -> Self {
        Self {
            level: 1,
            rounding: FloatRoundingMode::RoundHalfEven,
            allow_downcast_on_level1: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tensor storage (simple, GC-friendly start)
// ─────────────────────────────────────────────────────────────────────────────

/// Simple typed storage enum for tensors. Start simple: Vec-backed buffers.
#[derive(Debug, Clone)]
pub enum TensorStorage {
    Int(Vec<i64>),
    Float(Vec<f64>),
    Bool(Vec<bool>),
    Utf8(Vec<String>),
    /// Object references stored as Values for GC simplicity.
    Object(Vec<Value>),
}

impl TensorStorage {
    /// Number of elements in the storage.
    pub fn len(&self) -> usize {
        match self {
            TensorStorage::Int(v) => v.len(),
            TensorStorage::Float(v) => v.len(),
            TensorStorage::Bool(v) => v.len(),
            TensorStorage::Utf8(v) => v.len(),
            TensorStorage::Object(v) => v.len(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TensorValue — lightweight runtime tensor descriptor
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime tensor descriptor used inside the typing module.
#[derive(Debug, Clone)]
pub struct TensorValue {
    pub dtype: DType,
    pub shape: Shape,
    pub strides: Vec<usize>,
    pub storage: Arc<TensorStorage>,
    pub offset: usize,
    /// Optional per-tensor float tolerance override.
    pub float_tolerance: Option<FloatTolerance>,
}

impl TensorValue {
    /// Create a minimal placeholder tensor (zero-rank, empty storage).
    pub fn placeholder() -> Self {
        TensorValue {
            dtype: DType::Float64,
            shape: vec![],
            strides: vec![],
            storage: Arc::new(TensorStorage::Float(Vec::new())),
            offset: 0,
            float_tolerance: None,
        }
    }

    /// Compute row-major strides for a shape.
    pub fn compute_row_major_strides(shape: &Shape) -> Vec<usize> {
        let mut strides = vec![0; shape.len()];
        let mut acc = 1usize;
        for (i, dim) in shape.iter().rev().enumerate() {
            strides[shape.len() - 1 - i] = acc;
            acc = acc.saturating_mul(*dim);
        }
        strides
    }

    /// Number of elements.
    pub fn numel(&self) -> usize {
        self.shape.iter().product()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeId — the type lattice
// ─────────────────────────────────────────────────────────────────────────────

/// Core type identifiers. Ordered by promotion precedence where relevant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TypeId {
    /// Boolean (true/false). Promotes to Int for arithmetic.
    Bool,
    /// Signed 64-bit integer.
    Int,
    /// 64-bit IEEE float.
    Float,
    /// UTF-8 string. Can be compared numerically (if parseable) or lexicographically.
    String,
    /// Tensor (shape + dtype + data).
    Tensor,
    /// Null / None / absence of value. Numeric value = 0.
    Null,
    /// Ordered list of values. Comparison uses length.
    List,
    /// String-keyed ordered map.
    Map,
    /// First-class object family handle.
    Family,
    /// User-defined / extension type.
    Custom(u16),
}

impl TypeId {
    /// Returns true when the type can be treated as a number for arithmetic.
    pub fn is_numeric(self) -> bool {
        matches!(self, TypeId::Int | TypeId::Float | TypeId::Bool | TypeId::Null)
    }

    /// Returns true for String, Int, Float, Bool, Null — all printable scalars.
    pub fn is_scalar(self) -> bool {
        matches!(self, TypeId::Int | TypeId::Float | TypeId::String | TypeId::Bool | TypeId::Null)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Value — runtime value representation for the typing engine
// ─────────────────────────────────────────────────────────────────────────────

/// Runtime value representation used within the typing module.
///
/// Note: the interpreter's `environment::Value` is a different (older) type.
/// A bridge `typing_value_from_env` / `typing_value_to_env` pair in the executor
/// converts between the two for cross-module operations.
#[derive(Debug, Clone)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
    /// First-class tensor value.
    Tensor(TensorValue),
    Null,
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
    Family(FamilyHandle),
    Custom(u16, Box<dyn CustomValue>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Bool(a),   Value::Bool(b))   => a == b,
            (Value::Int(a),    Value::Int(b))     => a == b,
            (Value::Float(a),  Value::Float(b))   => (a - b).abs() < 1e-12,
            (Value::Int(a),    Value::Float(b))   => (*a as f64 - b).abs() < 1e-12,
            (Value::Float(a),  Value::Int(b))     => (a - *b as f64).abs() < 1e-12,
            (Value::String(a), Value::String(b))  => a == b,
            (Value::Null,      Value::Null)        => true,
            (Value::List(a),   Value::List(b))     => a == b,
            // Tensor equality is not deep-implemented here; compare pointer equality for now.
            (Value::Tensor(a), Value::Tensor(b))   => {
                Arc::ptr_eq(&a.storage, &b.storage) && a.offset == b.offset && a.shape == b.shape && a.dtype == b.dtype
            }
            _ => false,
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Bool(b)     => write!(f, "{}", b),
            Value::Int(i)      => write!(f, "{}", i),
            Value::Float(v)    => {
                if v.fract() == 0.0 && v.abs() < 1e15 { write!(f, "{}", *v as i64) }
                else { write!(f, "{}", v) }
            }
            Value::String(s)   => write!(f, "{}", s),
            Value::Tensor(t)   => write!(f, "<tensor dtype={:?} shape={:?}>", t.dtype, t.shape),
            Value::Null        => write!(f, "null"),
            Value::List(items) => {
                write!(f, "[")?;
                for (i, v) in items.iter().enumerate() {
                    if i > 0 { write!(f, ", ")?; }
                    write!(f, "{}", v)?;
                }
                write!(f, "]")
            }
            Value::Map(_)      => write!(f, "<map>"),
            Value::Family(_)   => write!(f, "<family>"),
            Value::Custom(..)  => write!(f, "<custom>"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Operator
// ─────────────────────────────────────────────────────────────────────────────

/// Arithmetic and comparison operators supported by the coercion engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operator {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    Lt,
    Gt,
    Le,
    Ge,
    Eq,
    Ne,
    And,
    Or,
    Not,
}

// ─────────────────────────────────────────────────────────────────────────────
// TypeError
// ─────────────────────────────────────────────────────────────────────────────

/// Structured type error with optional context.
#[derive(Debug)]
pub struct TypeError {
    pub message: String,
    pub left_type: Option<TypeId>,
    pub right_type: Option<TypeId>,
    pub op: Option<Operator>,
}

impl TypeError {
    pub fn new(msg: impl Into<String>) -> Self {
        Self {
            message: msg.into(),
            left_type: None,
            right_type: None,
            op: None,
        }
    }

    pub fn with_context(
        msg: impl Into<String>,
        left_type: Option<TypeId>,
        right_type: Option<TypeId>,
        op: Option<Operator>,
    ) -> Self {
        Self {
            message: msg.into(),
            left_type,
            right_type,
            op,
        }
    }

    /// Convenience constructor for shape mismatch errors.
    pub fn shape_mismatch(msg: impl Into<String>, left: Option<&Shape>, right: Option<&Shape>) -> Self {
        let mut e = TypeError::new(msg);
        // embed shape info in message for now
        if let Some(ls) = left {
            e.message = format!("{} (left shape: {:?})", e.message, ls);
        }
        if let Some(rs) = right {
            e.message = format!("{} (right shape: {:?})", e.message, rs);
        }
        e
    }
}

impl std::fmt::Display for TypeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TypeError: {}", self.message)?;
        if let (Some(lt), Some(rt)) = (&self.left_type, &self.right_type) {
            write!(f, " ({:?} vs {:?})", lt, rt)?;
        }
        if let Some(op) = &self.op {
            write!(f, " for {:?}", op)?;
        }
        Ok(())
    }
}

impl std::error::Error for TypeError {}

// ─────────────────────────────────────────────────────────────────────────────
// Extension points
// ─────────────────────────────────────────────────────────────────────────────

/// Trait for custom value types.
pub trait CustomValue: std::fmt::Debug + Send {
    fn type_id(&self) -> TypeId;
    fn clone_box(&self) -> Box<dyn CustomValue>;
    /// Return any child Values for GC marking (default none).
    fn gc_children(&self) -> Vec<Value> { Vec::new() }
    /// Optional operator override hook. Return Some(Ok/Err) to handle, or None to skip.
    fn try_operator(&self, _op: Operator, _rhs: &Value) -> Option<Result<Value, TypeError>> {
        None
    }
}

impl Clone for Box<dyn CustomValue> {
    fn clone(&self) -> Self { self.clone_box() }
}

/// Handle to a first-class object family.
#[derive(Debug, Clone)]
pub struct FamilyHandle {
    pub id: u64,
}

// ─────────────────────────────────────────────────────────────────────────────
// Coercion result and engine traits
// ─────────────────────────────────────────────────────────────────────────────

/// Result of coercion: coerced operands and the domain/result type.
#[derive(Debug)]
pub struct Coerced {
    pub left: Value,
    pub right: Value,
    /// The domain in which the operation should execute.
    pub result_type: TypeId,
}

/// Coercion engine: maps (op, left, right) to a coerced form ready for execution.
pub trait CoercionEngine: std::any::Any {
    fn coerce_for_op(
        &self,
        op: Operator,
        left: Value,
        right: Value,
    ) -> Result<Coerced, TypeError>;
}

/// Operator executor: applies an operation using a coercion engine.
pub trait OperatorExecutor {
    fn apply(
        &self,
        op: Operator,
        left: Value,
        right: Value,
    ) -> Result<Value, TypeError>;
}

// ─────────────────────────────────────────────────────────────────────────────
// Configuration
// ─────────────────────────────────────────────────────────────────────────────

/// Rounding modes for float rounding behavior.
#[derive(Debug, Clone, Copy)]
pub enum FloatRoundingMode {
    /// Round half to even (banker's rounding). Default.
    RoundHalfEven,
    /// Round half away from zero (standard mathematical).
    RoundHalfAwayFromZero,
    /// Truncate toward zero.
    Truncate,
}

/// Per-engine coercion configuration.
///
/// Controls float rounding/downcast behavior and division semantics.
/// Construct via `Default::default()` for sensible defaults.
#[derive(Debug, Clone)]
pub struct CoercionConfig {
    /// Rounding level 1..=20.
    ///   Level 1 → round to 0 decimal places, optionally downcast to Int.
    ///   Level N (N≥2) → round to (N-1) decimal places.
    pub float_tolerance_level: u8,

    /// Rounding mode applied at the selected level.
    pub float_rounding_mode: FloatRoundingMode,

    /// If true, Level 1 will downcast integral rounded results to Int.
    pub allow_downcast_on_level1: bool,

    /// If true, division always returns Float regardless of level.
    pub division_always_float: bool,

    /// Canonical default float tolerance (per-engine).
    pub default_float_tolerance: FloatTolerance,

    /// Allow per-tensor float tolerance overrides.
    pub allow_per_tensor_tolerance: bool,
}

impl Default for CoercionConfig {
    fn default() -> Self {
        Self {
            float_tolerance_level: 1,
            float_rounding_mode: FloatRoundingMode::RoundHalfEven,
            allow_downcast_on_level1: true,
            division_always_float: false,
            default_float_tolerance: FloatTolerance::default(),
            allow_per_tensor_tolerance: true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typeid_numeric() {
        assert!(TypeId::Int.is_numeric());
        assert!(TypeId::Float.is_numeric());
        assert!(TypeId::Bool.is_numeric());
        assert!(!TypeId::String.is_numeric());
        assert!(!TypeId::List.is_numeric());
    }

    #[test]
    fn value_display_int() {
        assert_eq!(format!("{}", Value::Int(42)), "42");
    }

    #[test]
    fn value_display_float_integral() {
        assert_eq!(format!("{}", Value::Float(3.0)), "3");
    }

    #[test]
    fn value_eq_int_float_cross() {
        assert_eq!(Value::Int(5), Value::Float(5.0));
        assert_eq!(Value::Float(5.0), Value::Int(5));
    }

    #[test]
    fn value_list_display() {
        let v = Value::List(vec![Value::Int(1), Value::Int(2)]);
        assert_eq!(format!("{}", v), "[1, 2]");
    }

    #[test]
    fn coercion_config_defaults() {
        let cfg = CoercionConfig::default();
        assert_eq!(cfg.float_tolerance_level, 1);
        assert!(cfg.allow_downcast_on_level1);
        assert!(!cfg.division_always_float);
        assert_eq!(cfg.default_float_tolerance.level, 1);
    }

    #[test]
    fn tensor_placeholder_display() {
        let t = Value::Tensor(TensorValue::placeholder());
        let s = format!("{}", t);
        assert!(s.contains("<tensor"));
    }
}
