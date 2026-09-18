//! Runtime-managed helper APIs for the compiled Pasta ABI.
//!
//! This layer builds on [`crate::runtime::abi`] by providing stable exported
//! entrypoints for handle allocation and lifecycle management, plus Rust-side
//! conversion helpers between ABI values and interpreter [`Value`]s.

use std::collections::HashMap;
use std::slice;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{LazyLock, Mutex};

use crate::interpreter::environment::{RuntimeTensor, Value};

use super::abi::{PHandleKind, PValue, PValueTag};

/// Status codes returned by exported ABI helper entrypoints.
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PAbiStatus {
    /// Operation completed successfully.
    Ok = 0,
    /// Caller passed a null out-pointer.
    NullOut = 1,
    /// Caller passed a null input pointer where one was required.
    NullInput = 2,
    /// Caller provided bytes that are not valid UTF-8.
    InvalidUtf8 = 3,
    /// Caller referenced a handle ID that does not exist.
    InvalidHandle = 4,
    /// Caller used a handle with the wrong runtime kind.
    HandleKindMismatch = 5,
    /// Caller provided structurally invalid arguments.
    InvalidArgument = 6,
    /// Caller attempted to bridge an interpreter-only value across the ABI.
    UnsupportedValue = 7,
    /// Caller passed the wrong number of arguments to a compiled module export.
    ArityMismatch = 8,
    /// Caller passed a value whose ABI type did not match the export signature.
    TypeMismatch = 9,
}

impl PAbiStatus {
    pub const fn code(self) -> i32 {
        self as i32
    }

    /// Convert a raw ABI status code into the corresponding enum variant.
    pub const fn from_code(code: i32) -> Option<Self> {
        match code {
            0 => Some(Self::Ok),
            1 => Some(Self::NullOut),
            2 => Some(Self::NullInput),
            3 => Some(Self::InvalidUtf8),
            4 => Some(Self::InvalidHandle),
            5 => Some(Self::HandleKindMismatch),
            6 => Some(Self::InvalidArgument),
            7 => Some(Self::UnsupportedValue),
            8 => Some(Self::ArityMismatch),
            9 => Some(Self::TypeMismatch),
            _ => None,
        }
    }
}

/// Dictionary-entry ABI used by `pasta_dict_new`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PDictEntry {
    /// UTF-8 key bytes.
    pub key_ptr: *const u8,
    /// Key byte length.
    pub key_len: usize,
    /// Value stored for the key.
    pub value: PValue,
}

#[derive(Debug, Clone)]
enum HandlePayload {
    String(String),
    List(Vec<PValue>),
    Dict(HashMap<String, PValue>),
    Tensor(RuntimeTensor),
}

#[derive(Debug, Clone)]
struct HandleEntry {
    kind: PHandleKind,
    refcount: u64,
    payload: HandlePayload,
}

static NEXT_HANDLE_ID: AtomicU64 = AtomicU64::new(1);
static HANDLE_REGISTRY: LazyLock<Mutex<HashMap<u64, HandleEntry>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Convert a compiled ABI value into the interpreter/runtime value model.
pub fn abi_value_to_runtime_value(value: PValue) -> Result<Value, PAbiStatus> {
    match value.tag {
        PValueTag::None => Ok(Value::None),
        PValueTag::Bool => Ok(Value::Bool(unsafe { value.data.boolean != 0 })),
        PValueTag::Number => Ok(Value::Number(unsafe { value.data.number })),
        PValueTag::Pointer => Ok(Value::Pointer(unsafe { value.data.pointer_id })),
        PValueTag::FamilyNode => {
            let node = unsafe { value.data.family_node };
            Ok(Value::FamilyNode {
                id: node.id,
                mutable: node.mutable_flag != 0,
            })
        }
        PValueTag::Handle => load_handle_value(value),
    }
}

/// Convert an interpreter/runtime value into the stable compiled ABI value model.
pub fn runtime_value_to_abi_value(value: &Value) -> Result<PValue, PAbiStatus> {
    match value {
        Value::None => Ok(PValue::none()),
        Value::Bool(value) => Ok(PValue::boolean(*value)),
        Value::Number(value) => Ok(PValue::number(*value)),
        Value::String(value) => create_string_handle(value.clone()),
        Value::List(items) => {
            let mut abi_items = Vec::with_capacity(items.len());
            for item in items {
                abi_items.push(runtime_value_to_abi_value(item)?);
            }
            let result = create_list_handle(&abi_items);
            release_many(abi_items.iter().copied());
            result
        }
        Value::Dict(items) => {
            let mut abi_items = HashMap::with_capacity(items.len());
            for (key, value) in items {
                abi_items.insert(key.clone(), runtime_value_to_abi_value(value)?);
            }
            let result = create_dict_handle(abi_items.clone());
            release_many(abi_items.values().copied());
            result
        }
        Value::Tensor(tensor) => create_tensor_handle(tensor.clone()),
        Value::Pointer(id) => Ok(PValue::pointer(*id)),
        Value::FamilyNode { id, mutable } => Ok(PValue::family_node(*id, *mutable)),
        Value::Lambda(_, _, _)
        | Value::LazyImport { .. }
        | Value::Heap(_)
        | Value::Pending(_, _)
        | Value::Builtin(_) => Err(PAbiStatus::UnsupportedValue),
    }
}

/// Retain a value crossing the compiled ABI.
#[no_mangle]
pub extern "C" fn pasta_value_retain(value: PValue) -> i32 {
    retain_value(value).unwrap_or_else(|status| status).code()
}

/// Release a value crossing the compiled ABI.
#[no_mangle]
pub extern "C" fn pasta_value_release(value: PValue) -> i32 {
    release_value(value).unwrap_or_else(|status| status).code()
}

/// Create a runtime-owned string handle from raw UTF-8 bytes.
#[no_mangle]
pub extern "C" fn pasta_string_new(bytes: *const u8, len: usize, out: *mut PValue) -> i32 {
    with_out_value(out, || {
        let bytes = read_bytes(bytes, len)?;
        let text = std::str::from_utf8(bytes).map_err(|_| PAbiStatus::InvalidUtf8)?;
        create_string_handle(text.to_string())
    })
}

/// View the raw UTF-8 bytes behind a runtime-owned string handle.
#[no_mangle]
pub extern "C" fn pasta_string_view(
    value: PValue,
    out_bytes: *mut *const u8,
    out_len: *mut usize,
) -> i32 {
    if out_bytes.is_null() || out_len.is_null() {
        return PAbiStatus::NullOut.code();
    }
    match with_registry(|registry| {
        let handle_id = expect_handle_id(value)?;
        let entry = registry.get(&handle_id).ok_or(PAbiStatus::InvalidHandle)?;
        if entry.kind != PHandleKind::String || entry.kind as u32 != value.handle_kind {
            return Err(PAbiStatus::HandleKindMismatch);
        }
        match &entry.payload {
            HandlePayload::String(text) => Ok((text.as_ptr(), text.len())),
            _ => Err(PAbiStatus::HandleKindMismatch),
        }
    }) {
        Ok((bytes, len)) => {
            unsafe {
                *out_bytes = bytes;
                *out_len = len;
            }
            PAbiStatus::Ok.code()
        }
        Err(status) => status.code(),
    }
}

/// Create a runtime-owned list handle from ABI values.
#[no_mangle]
pub extern "C" fn pasta_list_new(values: *const PValue, len: usize, out: *mut PValue) -> i32 {
    with_out_value(out, || {
        let values = read_slice(values, len)?;
        create_list_handle(values)
    })
}

/// Create a runtime-owned dictionary handle from UTF-8 key/value entries.
#[no_mangle]
pub extern "C" fn pasta_dict_new(entries: *const PDictEntry, len: usize, out: *mut PValue) -> i32 {
    with_out_value(out, || {
        let entries = read_slice(entries, len)?;
        let mut dict = HashMap::with_capacity(entries.len());
        for entry in entries {
            let key_bytes = read_bytes(entry.key_ptr, entry.key_len)?;
            let key = std::str::from_utf8(key_bytes)
                .map_err(|_| PAbiStatus::InvalidUtf8)?
                .to_string();
            dict.insert(key, entry.value);
        }
        create_dict_handle(dict)
    })
}

/// Create a runtime-owned tensor handle.
#[no_mangle]
pub extern "C" fn pasta_tensor_new(
    shape_ptr: *const u64,
    shape_len: usize,
    dtype_ptr: *const u8,
    dtype_len: usize,
    data_ptr: *const f64,
    data_len: usize,
    device_ptr: *const u8,
    device_len: usize,
    out: *mut PValue,
) -> i32 {
    with_out_value(out, || {
        let shape_words = read_slice(shape_ptr, shape_len)?;
        let shape = shape_words
            .iter()
            .map(|dim| usize::try_from(*dim).map_err(|_| PAbiStatus::InvalidArgument))
            .collect::<Result<Vec<_>, _>>()?;
        let dtype = std::str::from_utf8(read_bytes(dtype_ptr, dtype_len)?)
            .map_err(|_| PAbiStatus::InvalidUtf8)?;
        let device = std::str::from_utf8(read_bytes(device_ptr, device_len)?)
            .map_err(|_| PAbiStatus::InvalidUtf8)?;
        let data = read_slice(data_ptr, data_len)?.to_vec();
        let expected_len = shape.iter().try_fold(1usize, |acc, dim| {
            acc.checked_mul(*dim).ok_or(PAbiStatus::InvalidArgument)
        })?;
        if expected_len != data.len() {
            return Err(PAbiStatus::InvalidArgument);
        }
        create_tensor_handle(RuntimeTensor::with_device(shape, dtype.to_string(), data, device))
    })
}

fn with_out_value(out: *mut PValue, build: impl FnOnce() -> Result<PValue, PAbiStatus>) -> i32 {
    if out.is_null() {
        return PAbiStatus::NullOut.code();
    }
    match build() {
        Ok(value) => {
            unsafe { *out = value };
            PAbiStatus::Ok.code()
        }
        Err(status) => status.code(),
    }
}

fn read_bytes<'a>(ptr: *const u8, len: usize) -> Result<&'a [u8], PAbiStatus> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(PAbiStatus::NullInput);
    }
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn read_slice<'a, T>(ptr: *const T, len: usize) -> Result<&'a [T], PAbiStatus> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(PAbiStatus::NullInput);
    }
    Ok(unsafe { slice::from_raw_parts(ptr, len) })
}

fn create_string_handle(value: String) -> Result<PValue, PAbiStatus> {
    store_handle(PHandleKind::String, HandlePayload::String(value))
}

fn create_list_handle(values: &[PValue]) -> Result<PValue, PAbiStatus> {
    with_registry(|registry| {
        for value in values {
            validate_value_for_storage(registry, *value)?;
        }
        for value in values {
            retain_value_locked(registry, *value)?;
        }
        store_handle_locked(registry, PHandleKind::List, HandlePayload::List(values.to_vec()))
    })
}

fn create_dict_handle(values: HashMap<String, PValue>) -> Result<PValue, PAbiStatus> {
    with_registry(|registry| {
        for value in values.values() {
            validate_value_for_storage(registry, *value)?;
        }
        for value in values.values() {
            retain_value_locked(registry, *value)?;
        }
        store_handle_locked(registry, PHandleKind::Dict, HandlePayload::Dict(values))
    })
}

fn create_tensor_handle(value: RuntimeTensor) -> Result<PValue, PAbiStatus> {
    store_handle(PHandleKind::Tensor, HandlePayload::Tensor(value))
}

fn store_handle(kind: PHandleKind, payload: HandlePayload) -> Result<PValue, PAbiStatus> {
    with_registry(|registry| store_handle_locked(registry, kind, payload))
}

fn store_handle_locked(
    registry: &mut HashMap<u64, HandleEntry>,
    kind: PHandleKind,
    payload: HandlePayload,
) -> Result<PValue, PAbiStatus> {
    let handle_id = NEXT_HANDLE_ID.fetch_add(1, Ordering::Relaxed);
    let replaced = registry.insert(
        handle_id,
        HandleEntry {
            kind,
            refcount: 1,
            payload,
        },
    );
    debug_assert!(replaced.is_none(), "handle ids should be unique");
    Ok(PValue::handle(kind, handle_id))
}

fn load_handle_value(value: PValue) -> Result<Value, PAbiStatus> {
    let entry = with_registry(|registry| load_handle_entry(registry, value))?;
    match entry.payload {
        HandlePayload::String(text) => Ok(Value::String(text)),
        HandlePayload::Tensor(tensor) => Ok(Value::Tensor(tensor)),
        HandlePayload::List(items) => {
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                out.push(abi_value_to_runtime_value(item)?);
            }
            Ok(Value::List(out))
        }
        HandlePayload::Dict(items) => {
            let mut out = HashMap::with_capacity(items.len());
            for (key, value) in items {
                out.insert(key, abi_value_to_runtime_value(value)?);
            }
            Ok(Value::Dict(out))
        }
    }
}

fn load_handle_entry(
    registry: &HashMap<u64, HandleEntry>,
    value: PValue,
) -> Result<HandleEntry, PAbiStatus> {
    let handle_id = expect_handle_id(value)?;
    let entry = registry.get(&handle_id).ok_or(PAbiStatus::InvalidHandle)?;
    if entry.kind as u32 != value.handle_kind {
        return Err(PAbiStatus::HandleKindMismatch);
    }
    Ok(entry.clone())
}

fn validate_value_for_storage(
    registry: &HashMap<u64, HandleEntry>,
    value: PValue,
) -> Result<(), PAbiStatus> {
    if value.tag == PValueTag::Handle {
        let _ = load_handle_entry(registry, value)?;
    }
    Ok(())
}

fn retain_value(value: PValue) -> Result<PAbiStatus, PAbiStatus> {
    with_registry(|registry| retain_value_locked(registry, value).map(|_| PAbiStatus::Ok))
}

fn retain_value_locked(
    registry: &mut HashMap<u64, HandleEntry>,
    value: PValue,
) -> Result<(), PAbiStatus> {
    if value.tag != PValueTag::Handle {
        return Ok(());
    }
    let handle_id = expect_handle_id(value)?;
    let entry = registry.get_mut(&handle_id).ok_or(PAbiStatus::InvalidHandle)?;
    if entry.kind as u32 != value.handle_kind {
        return Err(PAbiStatus::HandleKindMismatch);
    }
    entry.refcount = entry
        .refcount
        .checked_add(1)
        .ok_or(PAbiStatus::InvalidArgument)?;
    Ok(())
}

fn release_value(value: PValue) -> Result<PAbiStatus, PAbiStatus> {
    if value.tag != PValueTag::Handle {
        return Ok(PAbiStatus::Ok);
    }
    let children = with_registry(|registry| release_handle_locked(registry, value))?;
    release_many(children);
    Ok(PAbiStatus::Ok)
}

fn release_handle_locked(
    registry: &mut HashMap<u64, HandleEntry>,
    value: PValue,
) -> Result<Vec<PValue>, PAbiStatus> {
    let handle_id = expect_handle_id(value)?;
    let entry = registry.get_mut(&handle_id).ok_or(PAbiStatus::InvalidHandle)?;
    if entry.kind as u32 != value.handle_kind {
        return Err(PAbiStatus::HandleKindMismatch);
    }
    if entry.refcount > 1 {
        entry.refcount -= 1;
        return Ok(Vec::new());
    }
    let entry = registry.remove(&handle_id).ok_or(PAbiStatus::InvalidHandle)?;
    Ok(match entry.payload {
        HandlePayload::String(_) | HandlePayload::Tensor(_) => Vec::new(),
        HandlePayload::List(items) => items,
        HandlePayload::Dict(items) => items.into_values().collect(),
    })
}

fn expect_handle_id(value: PValue) -> Result<u64, PAbiStatus> {
    if value.tag != PValueTag::Handle {
        return Err(PAbiStatus::InvalidArgument);
    }
    Ok(unsafe { value.data.handle_id })
}

fn release_many(values: impl IntoIterator<Item = PValue>) {
    for value in values {
        let _ = release_value(value);
    }
}

fn with_registry<T>(
    callback: impl FnOnce(&mut HashMap<u64, HandleEntry>) -> Result<T, PAbiStatus>,
) -> Result<T, PAbiStatus> {
    let mut registry = HANDLE_REGISTRY
        .lock()
        .map_err(|_| PAbiStatus::InvalidArgument)?;
    callback(&mut registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_handle_round_trips_through_runtime_value() {
        let value = Value::String("hello abi".to_string());
        let abi = runtime_value_to_abi_value(&value).expect("string to abi");
        let round_trip = abi_value_to_runtime_value(abi).expect("abi to string");
        assert_eq!(round_trip, value);
        assert_eq!(pasta_value_release(abi), PAbiStatus::Ok.code());
    }

    #[test]
    fn nested_handles_survive_container_round_trip() {
        let mut dict = HashMap::new();
        dict.insert("name".to_string(), Value::String("Ada".to_string()));
        dict.insert("flag".to_string(), Value::Bool(true));
        let value = Value::List(vec![Value::Number(3.0), Value::Dict(dict)]);

        let abi = runtime_value_to_abi_value(&value).expect("list to abi");
        let round_trip = abi_value_to_runtime_value(abi).expect("abi to list");
        assert_eq!(round_trip, value);
        assert_eq!(pasta_value_release(abi), PAbiStatus::Ok.code());
    }

    #[test]
    fn tensor_helper_validates_shape_and_utf8() {
        let shape = [2u64, 2];
        let data = [1.0f64, 2.0, 3.0, 4.0];
        let dtype = b"float32";
        let device = b"cpu";
        let mut out = PValue::none();
        let status = pasta_tensor_new(
            shape.as_ptr(),
            shape.len(),
            dtype.as_ptr(),
            dtype.len(),
            data.as_ptr(),
            data.len(),
            device.as_ptr(),
            device.len(),
            &mut out,
        );
        assert_eq!(status, PAbiStatus::Ok.code());
        let round_trip = abi_value_to_runtime_value(out).expect("tensor round trip");
        match round_trip {
            Value::Tensor(tensor) => {
                assert_eq!(tensor.shape, vec![2, 2]);
                assert_eq!(tensor.dtype, "float32");
                assert_eq!(tensor.device, "cpu");
                assert_eq!(tensor.data, vec![1.0, 2.0, 3.0, 4.0]);
            }
            other => panic!("expected tensor, got {other:?}"),
        }
        assert_eq!(pasta_value_release(out), PAbiStatus::Ok.code());

        let mut bad_out = PValue::none();
        let bad_status = pasta_tensor_new(
            shape.as_ptr(),
            shape.len(),
            dtype.as_ptr(),
            dtype.len(),
            data.as_ptr(),
            3,
            device.as_ptr(),
            device.len(),
            &mut bad_out,
        );
        assert_eq!(bad_status, PAbiStatus::InvalidArgument.code());
    }

    #[test]
    fn exported_helpers_use_status_codes() {
        let bytes = b"ffi string";
        assert_eq!(
            pasta_string_new(bytes.as_ptr(), bytes.len(), std::ptr::null_mut()),
            PAbiStatus::NullOut.code()
        );

        let mut out = PValue::none();
        let status = pasta_string_new(bytes.as_ptr(), bytes.len(), &mut out);
        assert_eq!(status, PAbiStatus::Ok.code());
        assert_eq!(pasta_value_retain(out), PAbiStatus::Ok.code());
        assert_eq!(pasta_value_release(out), PAbiStatus::Ok.code());
        assert_eq!(pasta_value_release(out), PAbiStatus::Ok.code());
        assert_eq!(pasta_value_release(out), PAbiStatus::InvalidHandle.code());
    }
}
