// src/typing/tensor_type.rs
//! Minimal tensor helpers used by the typing module.
//! Lightweight, Vec-backed storage and basic indexing utilities.
//! Intended as a safe skeleton to iterate on; performance and features
//! (views, broadcasting, kernels) come in later patches.

use std::sync::Arc;

use crate::typing::types::{
    DType, FloatTolerance, Shape, TensorStorage, TensorValue, Value,
};

/// Create a zeros tensor for numeric dtypes (Int/Float) and sensible defaults for others.
pub fn zeros(dtype: DType, shape: Shape) -> TensorValue {
    let numel: usize = shape.iter().product();
    let storage = match dtype {
        DType::Int64 => TensorStorage::Int(vec![0i64; numel]),
        DType::Float64 => TensorStorage::Float(vec![0f64; numel]),
        DType::Bool => TensorStorage::Bool(vec![false; numel]),
        DType::Utf8 => TensorStorage::Utf8(vec![String::new(); numel]),
        DType::ObjectRef(_) | DType::Custom(_) => {
            TensorStorage::Object(vec![Value::Null; numel])
        }
    };
    let strides = TensorValue::compute_row_major_strides(&shape);
    TensorValue {
        dtype,
        shape,
        strides,
        storage: Arc::new(storage),
        offset: 0,
        float_tolerance: Some(FloatTolerance::default()),
    }
}

/// Construct an Int tensor from a Vec<i64>.
/// Panics if `data.len()` != product(shape).
pub fn from_vec_int(shape: Shape, data: Vec<i64>) -> TensorValue {
    let expected: usize = shape.iter().product();
    assert_eq!(expected, data.len(), "from_vec_int: data length mismatch");
    let strides = TensorValue::compute_row_major_strides(&shape);
    TensorValue {
        dtype: DType::Int64,
        shape,
        strides,
        storage: Arc::new(TensorStorage::Int(data)),
        offset: 0,
        float_tolerance: None,
    }
}

/// Construct a Float tensor from a Vec<f64>.
/// Panics if `data.len()` != product(shape).
pub fn from_vec_float(shape: Shape, data: Vec<f64>) -> TensorValue {
    let expected: usize = shape.iter().product();
    assert_eq!(expected, data.len(), "from_vec_float: data length mismatch");
    let strides = TensorValue::compute_row_major_strides(&shape);
    TensorValue {
        dtype: DType::Float64,
        shape,
        strides,
        storage: Arc::new(TensorStorage::Float(data)),
        offset: 0,
        float_tolerance: None,
    }
}

/// Compute a flat index from multi-dimensional indices (row-major).
/// Returns None if indices length mismatches or an index is out of bounds.
pub fn flat_index(t: &TensorValue, idx: &[usize]) -> Option<usize> {
    if idx.len() != t.shape.len() { return None; }
    let mut flat = t.offset;
    for (i, &v) in idx.iter().enumerate() {
        if v >= t.shape[i] { return None; }
        // strides are row-major precomputed
        flat = flat.saturating_add(v.saturating_mul(t.strides[i]));
    }
    Some(flat)
}

/// Read an Int element by multi-index. Returns None on mismatch or non-Int dtype.
pub fn get_int(t: &TensorValue, idx: &[usize]) -> Option<i64> {
    if t.dtype != DType::Int64 { return None; }
    let flat = flat_index(t, idx)?;
    match &*t.storage {
        TensorStorage::Int(v) => v.get(flat).copied(),
        _ => None,
    }
}

/// Read a Float element by multi-index. Returns None on mismatch or non-Float dtype.
pub fn get_float(t: &TensorValue, idx: &[usize]) -> Option<f64> {
    if t.dtype != DType::Float64 { return None; }
    let flat = flat_index(t, idx)?;
    match &*t.storage {
        TensorStorage::Float(v) => v.get(flat).copied(),
        _ => None,
    }
}
