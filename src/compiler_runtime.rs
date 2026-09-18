use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::process::abort;
use std::sync::Mutex;

use once_cell::sync::Lazy;

use crate::runtime::{PValue, abi_value_to_runtime_value, runtime_value_to_abi_value};
use crate::{Executor, Value};

static COMPILED_EXECUTOR: Lazy<Mutex<Executor>> = Lazy::new(|| Mutex::new(Executor::new()));
static STRING_POOL: Lazy<Mutex<HashMap<String, CString>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

fn with_executor<T>(f: impl FnOnce(&mut Executor) -> T) -> T {
    let mut guard = COMPILED_EXECUTOR.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    f(&mut guard)
}

fn read_c_string(arg_name: &str, ptr: *const c_char) -> String {
    if ptr.is_null() {
        eprintln!("[pasta/compiler-runtime] {arg_name} must not be null");
        abort();
    }
    match unsafe { CStr::from_ptr(ptr) }.to_str() {
        Ok(value) => value.to_string(),
        Err(err) => {
            eprintln!("[pasta/compiler-runtime] invalid utf-8 for {arg_name}: {err}");
            abort();
        }
    }
}

fn intern_string(value: String) -> *const c_char {
    let mut pool = STRING_POOL.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if !pool.contains_key(&value) {
        let c_string = match CString::new(value.clone()) {
            Ok(c_string) => c_string,
            Err(err) => {
                eprintln!("[pasta/compiler-runtime] string contains interior NUL: {err}");
                abort();
            }
        };
        pool.insert(value.clone(), c_string);
    }
    pool.get(&value).expect("interned string must exist").as_ptr()
}

fn runtime_abort(call_name: &str, err: impl std::fmt::Display) -> ! {
    eprintln!("[pasta/compiler-runtime] {call_name} failed: {err}");
    abort();
}

#[repr(C)]
pub struct PastaRtDictNumberEntry {
    key: *const c_char,
    value: f64,
}

fn expect_runtime_number(call_name: &str, value: Value) -> f64 {
    match value {
        Value::Number(number) => number,
        other => runtime_abort(call_name, format!("unexpected return value: {other:?}")),
    }
}

fn format_runtime_number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{}", value as i64)
    } else {
        value.to_string()
    }
}

fn abi_to_runtime(call_name: &str, value: PValue) -> Value {
    abi_value_to_runtime_value(value).unwrap_or_else(|err| runtime_abort(call_name, format!("{err:?}")))
}

fn runtime_to_abi(call_name: &str, value: &Value) -> PValue {
    runtime_value_to_abi_value(value).unwrap_or_else(|err| runtime_abort(call_name, format!("{err:?}")))
}

fn read_pvalue(arg_name: &str, ptr: *const PValue) -> PValue {
    if ptr.is_null() {
        runtime_abort(arg_name, "value pointer must not be null");
    }
    unsafe { *ptr }
}

fn write_pvalue(out_name: &str, ptr: *mut PValue, value: PValue) {
    if ptr.is_null() {
        runtime_abort(out_name, "output pointer must not be null");
    }
    unsafe {
        *ptr = value;
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_window_new(
    title: *const c_char,
    width: f64,
    height: f64,
) -> *const c_char {
    let title = read_c_string("title", title);
    let result = with_executor(|executor| {
        executor.call_builtin(
            "window",
            vec![Value::String(title), Value::Number(width), Value::Number(height)],
        )
    });
    match result {
        Ok(Value::String(handle)) => intern_string(handle),
        Ok(other) => runtime_abort("WINDOW", format!("unexpected return value: {other:?}")),
        Err(err) => runtime_abort("WINDOW", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_window_poll(window: *const c_char) -> i32 {
    let window = read_c_string("window", window);
    let result = with_executor(|executor| {
        executor.call_builtin("window_poll", vec![Value::String(window)])
    });
    match result {
        Ok(Value::Bool(open)) => i32::from(open),
        Ok(other) => runtime_abort("WINDOW_POLL", format!("unexpected return value: {other:?}")),
        Err(err) => runtime_abort("WINDOW_POLL", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_window_key(window: *const c_char) -> *const c_char {
    let window = read_c_string("window", window);
    let result = with_executor(|executor| {
        executor.call_builtin("window_key", vec![Value::String(window)])
    });
    match result {
        Ok(Value::String(key)) => intern_string(key),
        Ok(other) => runtime_abort("WINDOW_KEY", format!("unexpected return value: {other:?}")),
        Err(err) => runtime_abort("WINDOW_KEY", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_window_close(window: *const c_char) {
    let window = read_c_string("window", window);
    let result = with_executor(|executor| {
        executor.call_builtin("window_close", vec![Value::String(window)])
    });
    if let Err(err) = result {
        runtime_abort("WINDOW_CLOSE", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_set_draw_target(window: *const c_char) {
    let window = read_c_string("window", window);
    let result = with_executor(|executor| {
        executor.call_builtin("set_draw_target", vec![Value::String(window)])
    });
    if let Err(err) = result {
        runtime_abort("SET_DRAW_TARGET", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_set_color_packed(color: f64) {
    let result = with_executor(|executor| {
        executor.call_builtin("set_color", vec![Value::Number(color)])
    });
    if let Err(err) = result {
        runtime_abort("SET_COLOR", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_canvas_fill_rect(
    window: *const c_char,
    x1: f64,
    y1: f64,
    x2: f64,
    y2: f64,
) {
    let window = read_c_string("window", window);
    let result = with_executor(|executor| {
        executor.call_builtin(
            "canvas_fill_rect",
            vec![
                Value::String(window),
                Value::Number(x1),
                Value::Number(y1),
                Value::Number(x2),
                Value::Number(y2),
            ],
        )
    });
    if let Err(err) = result {
        runtime_abort("canvas_fill_rect", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_swap_buffer(window: *const c_char) {
    let window = read_c_string("window", window);
    let result = with_executor(|executor| {
        executor.call_builtin("swap_buffer", vec![Value::String(window)])
    });
    if let Err(err) = result {
        runtime_abort("SWAP_BUFFER", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_fps_init(target: f64) {
    let result = with_executor(|executor| {
        executor.call_builtin("fps_init", vec![Value::Number(target)])
    });
    if let Err(err) = result {
        runtime_abort("fps_init", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_fps_begin(target: f64) {
    let result = with_executor(|executor| {
        executor.call_builtin("fps_begin", vec![Value::Number(target)])
    });
    if let Err(err) = result {
        runtime_abort("fps_begin", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_fps_end() {
    let result = with_executor(|executor| executor.call_builtin("fps_end", vec![]));
    if let Err(err) = result {
        runtime_abort("fps_end", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_fps_tick() {
    let result = with_executor(|executor| executor.call_builtin("fps_tick", vec![]));
    if let Err(err) = result {
        runtime_abort("fps_tick", err);
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_rand_int2(min: f64, max: f64) -> f64 {
    let result = with_executor(|executor| {
        executor.call_builtin("rand.int", vec![Value::Number(min), Value::Number(max)])
    });
    match result {
        Ok(value) => expect_runtime_number("rand.int", value),
        Err(err) => runtime_abort("rand.int", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_list_new_numbers(values: *const f64, len: usize, out: *mut PValue) {
    if len > 0 && values.is_null() {
        runtime_abort("list literal", "values must not be null");
    }
    let values = unsafe { std::slice::from_raw_parts(values, len) };
    let items = values.iter().map(|value| Value::Number(*value)).collect::<Vec<_>>();
    write_pvalue("list literal", out, runtime_to_abi("list literal", &Value::List(items)));
}

#[no_mangle]
pub extern "C" fn pasta_rt_dict_new_string_number(
    entries: *const PastaRtDictNumberEntry,
    len: usize,
    out: *mut PValue,
) {
    if len > 0 && entries.is_null() {
        runtime_abort("dict literal", "entries must not be null");
    }
    let entries = unsafe { std::slice::from_raw_parts(entries, len) };
    let items = entries
        .iter()
        .map(|entry| {
            (
                read_c_string("dict key", entry.key),
                Value::Number(entry.value),
            )
        })
        .collect::<HashMap<_, _>>();
    write_pvalue("dict literal", out, runtime_to_abi("dict literal", &Value::Dict(items)));
}

#[no_mangle]
pub extern "C" fn pasta_rt_list_len(value: *const PValue) -> f64 {
    let result = with_executor(|executor| {
        executor.call_builtin("list_len", vec![abi_to_runtime("list_len", read_pvalue("list_len", value))])
    });
    match result {
        Ok(value) => expect_runtime_number("list_len", value),
        Err(err) => runtime_abort("list_len", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_list_slice(
    value: *const PValue,
    start: f64,
    end: f64,
    out: *mut PValue,
) {
    let result = with_executor(|executor| {
        executor.call_builtin(
            "list_slice",
            vec![
                abi_to_runtime("list_slice", read_pvalue("list_slice", value)),
                Value::Number(start),
                Value::Number(end),
            ],
        )
    });
    match result {
        Ok(value) => write_pvalue("list_slice", out, runtime_to_abi("list_slice", &value)),
        Err(err) => runtime_abort("list_slice", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_list_concat(left: *const PValue, right: *const PValue, out: *mut PValue) {
    let result = with_executor(|executor| {
        executor.call_builtin(
            "list_concat",
            vec![
                abi_to_runtime("list_concat", read_pvalue("list_concat", left)),
                abi_to_runtime("list_concat", read_pvalue("list_concat", right)),
            ],
        )
    });
    match result {
        Ok(value) => write_pvalue("list_concat", out, runtime_to_abi("list_concat", &value)),
        Err(err) => runtime_abort("list_concat", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_list_index_number(value: *const PValue, index: f64) -> f64 {
    let runtime_value = abi_to_runtime("list index", read_pvalue("list index", value));
    match runtime_value {
        Value::List(items) => {
            let index = index as isize;
            if index < 0 || (index as usize) >= items.len() {
                runtime_abort("list index", "index out of bounds");
            }
            expect_runtime_number("list index", items[index as usize].clone())
        }
        other => runtime_abort("list index", format!("expected list, got {other:?}")),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_dict_get_number(value: *const PValue, key: *const c_char) -> f64 {
    let key_string = read_c_string("dict key", key);
    let result = with_executor(|executor| {
        executor.call_builtin(
            "dict_get",
            vec![
                abi_to_runtime("dict_get", read_pvalue("dict_get", value)),
                Value::String(key_string),
            ],
        )
    });
    match result {
        Ok(value) => expect_runtime_number("dict_get", value),
        Err(err) => runtime_abort("dict_get", err),
    }
}

#[no_mangle]
pub extern "C" fn pasta_rt_number_to_string(value: f64) -> *const c_char {
    intern_string(format_runtime_number(value))
}

#[no_mangle]
pub extern "C" fn pasta_rt_string_concat(left: *const c_char, right: *const c_char) -> *const c_char {
    let left = read_c_string("left", left);
    let right = read_c_string("right", right);
    intern_string(format!("{left}{right}"))
}
