//! Pointer Operations - PULL/PUSH implementations for each pointer kind
//!
//! Provides typed read/write operations for memory, file, device, and network pointers.

use crate::interpreter::Value;
use super::pointer::{Pointer, PointerKind, PointerTarget};
use std::sync::{Arc, Mutex};

/// Type of data for PULL/PUSH operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// Single-byte integer data.
    Byte,
    /// Signed integer data.
    Int,
    /// Floating-point data.
    Float,
    /// UTF-8 string data.
    Str,
    /// Raw byte-array data.
    Bytes,
}

impl DataType {
    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "BYTE" => Some(DataType::Byte),
            "INT" => Some(DataType::Int),
            "FLOAT" => Some(DataType::Float),
            "STR" | "STRING" => Some(DataType::Str),
            "BYTES" => Some(DataType::Bytes),
            _ => None,
        }
    }
}

/// Error type for pointer operations
#[derive(Debug, Clone)]
pub struct PointerError {
    /// Error message
    pub message: String,
}

impl PointerError {
    /// Create a new pointer error
    pub fn new(msg: impl Into<String>) -> Self {
        Self { message: msg.into() }
    }
}

impl std::fmt::Display for PointerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for PointerError {}

/// Result type for pointer operations
pub type OpResult = Result<Value, PointerError>;

/// PULL operation - read from pointer
pub fn pull(ptr: &mut Pointer, dtype: DataType, args: &[Value]) -> OpResult {
    if !ptr.alive {
        return Err(PointerError::new("Attempt to PULL from dead pointer"));
    }

    match (&mut ptr.target, ptr.kind) {
        (PointerTarget::Memory { data, offset }, PointerKind::Mem) => {
            pull_memory(data, offset, dtype, args)
        }
        (PointerTarget::File { path, offset, mode }, PointerKind::File) => {
            pull_file(path, offset, mode, dtype, args)
        }
        (PointerTarget::Device { device_id, device_type }, PointerKind::Dev) => {
            pull_device(device_id, device_type, dtype, args)
        }
        (PointerTarget::Network { host, port, stream }, PointerKind::Net) => {
            pull_network(host, port, stream, dtype, args)
        }
        _ => Err(PointerError::new("Pointer kind mismatch")),
    }
}

/// PUSH operation - write to pointer
pub fn push(ptr: &mut Pointer, dtype: DataType, value: Value, args: &[Value]) -> OpResult {
    if !ptr.alive {
        return Err(PointerError::new("Attempt to PUSH to dead pointer"));
    }

    match (&mut ptr.target, ptr.kind) {
        (PointerTarget::Memory { data, offset }, PointerKind::Mem) => {
            push_memory(data, offset, dtype, value, args)
        }
        (PointerTarget::File { path, offset, mode }, PointerKind::File) => {
            push_file(path, offset, mode, dtype, value, args)
        }
        (PointerTarget::Device { device_id, device_type }, PointerKind::Dev) => {
            push_device(device_id, device_type, dtype, value, args)
        }
        (PointerTarget::Network { host, port, stream }, PointerKind::Net) => {
            push_network(host, port, stream, dtype, value, args)
        }
        _ => Err(PointerError::new("Pointer kind mismatch")),
    }
}

// ─── Memory Operations ─────────────────────────────────────────────────────────

fn pull_memory(data: &[u8], offset: &mut usize, dtype: DataType, _args: &[Value]) -> OpResult {
    match dtype {
        DataType::Byte => {
            if *offset >= data.len() {
                return Err(PointerError::new("Memory read past end of buffer"));
            }
            let b = data[*offset];
            *offset += 1;
            Ok(Value::Number(b as f64))
        }
        DataType::Int => {
            if *offset + 4 > data.len() {
                return Err(PointerError::new("Memory read past end of buffer"));
            }
            let bytes: [u8; 4] = data[*offset..*offset+4].try_into().unwrap();
            let val = i32::from_le_bytes(bytes);
            *offset += 4;
            Ok(Value::Number(val as f64))
        }
        DataType::Float => {
            if *offset + 8 > data.len() {
                return Err(PointerError::new("Memory read past end of buffer"));
            }
            let bytes: [u8; 8] = data[*offset..*offset+8].try_into().unwrap();
            let val = f64::from_le_bytes(bytes);
            *offset += 8;
            Ok(Value::Number(val))
        }
        DataType::Str => {
            // Read null-terminated string
            let start = *offset;
            while *offset < data.len() && data[*offset] != 0 {
                *offset += 1;
            }
            let s = String::from_utf8_lossy(&data[start..*offset]).to_string();
            if *offset < data.len() {
                *offset += 1; // Skip null terminator
            }
            Ok(Value::String(s))
        }
        DataType::Bytes => {
            // Return remaining bytes as list
            let bytes: Vec<Value> = data[*offset..].iter()
                .map(|b| Value::Number(*b as f64))
                .collect();
            *offset = data.len();
            Ok(Value::List(bytes))
        }
    }
}

fn push_memory(data: &mut Vec<u8>, offset: &mut usize, dtype: DataType, value: Value, _args: &[Value]) -> OpResult {
    match dtype {
        DataType::Byte => {
            let b = match value {
                Value::Number(n) => n as u8,
                _ => return Err(PointerError::new("PUSH.BYTE requires number")),
            };
            if *offset >= data.len() {
                data.resize(*offset + 1, 0);
            }
            data[*offset] = b;
            *offset += 1;
            Ok(Value::None)
        }
        DataType::Int => {
            let n = match value {
                Value::Number(n) => n as i32,
                _ => return Err(PointerError::new("PUSH.INT requires number")),
            };
            let bytes = n.to_le_bytes();
            if *offset + 4 > data.len() {
                data.resize(*offset + 4, 0);
            }
            data[*offset..*offset+4].copy_from_slice(&bytes);
            *offset += 4;
            Ok(Value::None)
        }
        DataType::Float => {
            let n = match value {
                Value::Number(n) => n,
                _ => return Err(PointerError::new("PUSH.FLOAT requires number")),
            };
            let bytes = n.to_le_bytes();
            if *offset + 8 > data.len() {
                data.resize(*offset + 8, 0);
            }
            data[*offset..*offset+8].copy_from_slice(&bytes);
            *offset += 8;
            Ok(Value::None)
        }
        DataType::Str => {
            let s = match value {
                Value::String(s) => s,
                _ => return Err(PointerError::new("PUSH.STR requires string")),
            };
            let bytes = s.as_bytes();
            if *offset + bytes.len() + 1 > data.len() {
                data.resize(*offset + bytes.len() + 1, 0);
            }
            data[*offset..*offset+bytes.len()].copy_from_slice(bytes);
            data[*offset + bytes.len()] = 0; // Null terminator
            *offset += bytes.len() + 1;
            Ok(Value::None)
        }
        DataType::Bytes => {
            let bytes: Vec<u8> = match value {
                Value::List(list) => {
                    list.iter().map(|v| match v {
                        Value::Number(n) => *n as u8,
                        _ => 0,
                    }).collect()
                }
                _ => return Err(PointerError::new("PUSH.BYTES requires list")),
            };
            if *offset + bytes.len() > data.len() {
                data.resize(*offset + bytes.len(), 0);
            }
            data[*offset..*offset+bytes.len()].copy_from_slice(&bytes);
            *offset += bytes.len();
            Ok(Value::None)
        }
    }
}

// ─── File Operations ───────────────────────────────────────────────────────────

fn pull_file(path: &str, offset: &mut u64, _mode: &str, dtype: DataType, _args: &[Value]) -> OpResult {
    use std::fs::File;
    use std::io::{Read, BufRead, BufReader, Seek, SeekFrom};

    let mut file = File::open(path)
        .map_err(|e| PointerError::new(format!("Failed to open file '{}': {}", path, e)))?;

    file.seek(SeekFrom::Start(*offset))
        .map_err(|e| PointerError::new(format!("Seek error: {}", e)))?;

    match dtype {
        DataType::Byte => {
            let mut buf = [0u8; 1];
            let n = file.read(&mut buf)
                .map_err(|e| PointerError::new(format!("Read error: {}", e)))?;
            if n == 0 { return Ok(Value::None); }
            *offset += 1;
            Ok(Value::Number(buf[0] as f64))
        }
        DataType::Int => {
            let mut buf = [0u8; 4];
            file.read_exact(&mut buf)
                .map_err(|e| PointerError::new(format!("Read error: {}", e)))?;
            *offset += 4;
            Ok(Value::Number(i32::from_le_bytes(buf) as f64))
        }
        DataType::Float => {
            let mut buf = [0u8; 8];
            file.read_exact(&mut buf)
                .map_err(|e| PointerError::new(format!("Read error: {}", e)))?;
            *offset += 8;
            Ok(Value::Number(f64::from_le_bytes(buf)))
        }
        DataType::Str => {
            let mut reader = BufReader::new(file);
            let mut line = String::new();
            let n = reader.read_line(&mut line)
                .map_err(|e| PointerError::new(format!("Read error: {}", e)))?;
            if n == 0 { return Ok(Value::None); }
            *offset += n as u64;
            Ok(Value::String(line.trim_end_matches(&['\n', '\r']).to_string()))
        }
        DataType::Bytes => {
            let mut contents = Vec::new();
            file.read_to_end(&mut contents)
                .map_err(|e| PointerError::new(format!("Read error: {}", e)))?;
            *offset += contents.len() as u64;
            Ok(Value::List(contents.iter().map(|b| Value::Number(*b as f64)).collect()))
        }
    }
}

fn push_file(path: &str, offset: &mut u64, mode: &str, dtype: DataType, value: Value, _args: &[Value]) -> OpResult {
    use std::fs::OpenOptions;
    use std::io::{Write, Seek, SeekFrom};

    let append = mode == "a";
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .append(append)
        .open(path)
        .map_err(|e| PointerError::new(format!("Failed to open file '{}': {}", path, e)))?;

    if !append {
        file.seek(SeekFrom::Start(*offset))
            .map_err(|e| PointerError::new(format!("Seek error: {}", e)))?;
    }

    let written = match dtype {
        DataType::Byte => {
            let b = match value {
                Value::Number(n) => n as u8,
                _ => return Err(PointerError::new("PUSH.BYTE requires number")),
            };
            file.write_all(&[b])
                .map_err(|e| PointerError::new(format!("Write error: {}", e)))?;
            1u64
        }
        DataType::Int => {
            let n = match value {
                Value::Number(n) => n as i32,
                _ => return Err(PointerError::new("PUSH.INT requires number")),
            };
            file.write_all(&n.to_le_bytes())
                .map_err(|e| PointerError::new(format!("Write error: {}", e)))?;
            4
        }
        DataType::Float => {
            let n = match value {
                Value::Number(n) => n,
                _ => return Err(PointerError::new("PUSH.FLOAT requires number")),
            };
            file.write_all(&n.to_le_bytes())
                .map_err(|e| PointerError::new(format!("Write error: {}", e)))?;
            8
        }
        DataType::Str => {
            let s = match value {
                Value::String(s) => s,
                _ => return Err(PointerError::new("PUSH.STR requires string")),
            };
            let bytes = s.as_bytes();
            file.write_all(bytes)
                .map_err(|e| PointerError::new(format!("Write error: {}", e)))?;
            file.write_all(b"\n")
                .map_err(|e| PointerError::new(format!("Write error: {}", e)))?;
            bytes.len() as u64 + 1
        }
        DataType::Bytes => {
            let bytes: Vec<u8> = match value {
                Value::List(list) => list.iter().map(|v| match v {
                    Value::Number(n) => *n as u8,
                    _ => 0,
                }).collect(),
                _ => return Err(PointerError::new("PUSH.BYTES requires list")),
            };
            let len = bytes.len() as u64;
            file.write_all(&bytes)
                .map_err(|e| PointerError::new(format!("Write error: {}", e)))?;
            len
        }
    };

    *offset += written;
    Ok(Value::None)
}

// ─── Device Operations ─────────────────────────────────────────────────

/// Device read operation - reads data from a device pointer.
/// Currently supports basic device types with simulated reads.
fn pull_device(device_id: &str, device_type: &str, dtype: DataType, _args: &[Value]) -> OpResult {
    // Basic device simulation - in production, wire to actual device drivers
    match device_type {
        "null" => {
            // /dev/null equivalent - always returns zeros/empty
            match dtype {
                DataType::Byte | DataType::Int => Ok(Value::Number(0.0)),
                DataType::Float => Ok(Value::Number(0.0)),
                DataType::Str => Ok(Value::String(String::new())),
                DataType::Bytes => Ok(Value::List(vec![])),
            }
        }
        "zero" => {
            // /dev/zero equivalent - always returns zeros
            match dtype {
                DataType::Byte | DataType::Int => Ok(Value::Number(0.0)),
                DataType::Float => Ok(Value::Number(0.0)),
                DataType::Str => Ok(Value::String(String::new())),
                DataType::Bytes => Ok(Value::List(vec![Value::Number(0.0); 256])),
            }
        }
        "random" | "urandom" => {
            use std::time::{SystemTime, UNIX_EPOCH};
            let seed = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos() as u64;
            match dtype {
                DataType::Byte => Ok(Value::Number(((seed >> 8) & 0xFF) as f64)),
                DataType::Int => Ok(Value::Number((seed & 0xFFFFFFFF) as f64)),
                DataType::Float => Ok(Value::Number((seed as f64) / (u64::MAX as f64))),
                DataType::Bytes => {
                    let mut bytes = Vec::with_capacity(256);
                    let mut s = seed;
                    for _ in 0..256 {
                        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
                        bytes.push(Value::Number(((s >> 33) & 0xFF) as f64));
                    }
                    Ok(Value::List(bytes))
                }
                DataType::Str => Ok(Value::String(format!("{:016x}", seed))),
            }
        }
        "stdin" => {
            use std::io::BufRead;
            match dtype {
                DataType::Str => {
                    let stdin = std::io::stdin();
                    let mut line = String::new();
                    let n = stdin.lock().read_line(&mut line)
                        .map_err(|e| PointerError::new(format!("stdin read error: {}", e)))?;
                    if n == 0 { return Ok(Value::None); }
                    Ok(Value::String(line.trim_end_matches(&['\n', '\r']).to_string()))
                }
                DataType::Byte => {
                    use std::io::Read;
                    let mut buf = [0u8; 1];
                    let n = std::io::stdin().read(&mut buf)
                        .map_err(|e| PointerError::new(format!("stdin read error: {}", e)))?;
                    if n == 0 { return Ok(Value::None); }
                    Ok(Value::Number(buf[0] as f64))
                }
                DataType::Bytes => {
                    use std::io::Read;
                    let mut buf = Vec::new();
                    std::io::stdin().read_to_end(&mut buf)
                        .map_err(|e| PointerError::new(format!("stdin read error: {}", e)))?;
                    Ok(Value::List(buf.iter().map(|b| Value::Number(*b as f64)).collect()))
                }
                _ => Err(PointerError::new("stdin PULL supports Byte, Str, Bytes")),
            }
        }
        "serial" => {
            // Read from a serial port — device_id is the port path e.g. /dev/ttyS0
            // We open, read one chunk, close. For persistent serial, use a FILE pointer.
            use std::fs::OpenOptions;
            use std::io::{Read, BufRead, BufReader};
            let mut f = OpenOptions::new().read(true).open(device_id)
                .map_err(|e| PointerError::new(format!("Cannot open serial port '{}': {}", device_id, e)))?;
            match dtype {
                DataType::Byte => {
                    let mut buf = [0u8; 1];
                    f.read_exact(&mut buf)
                        .map_err(|e| PointerError::new(format!("Serial read error: {}", e)))?;
                    Ok(Value::Number(buf[0] as f64))
                }
                DataType::Str => {
                    let mut reader = BufReader::new(f);
                    let mut line = String::new();
                    reader.read_line(&mut line)
                        .map_err(|e| PointerError::new(format!("Serial read error: {}", e)))?;
                    Ok(Value::String(line.trim_end_matches(&['\n', '\r']).to_string()))
                }
                DataType::Bytes => {
                    let mut buf = vec![0u8; 256];
                    let n = f.read(&mut buf)
                        .map_err(|e| PointerError::new(format!("Serial read error: {}", e)))?;
                    Ok(Value::List(buf[..n].iter().map(|b| Value::Number(*b as f64)).collect()))
                }
                _ => Err(PointerError::new("Serial PULL supports Byte, Str, Bytes")),
            }
        }
        _ => Err(PointerError::new(format!(
            "Device type '{}' (id: {}) not supported for PULL",
            device_type, device_id
        ))),
    }
}

/// Device write operation - writes data to a device pointer.
fn push_device(device_id: &str, device_type: &str, dtype: DataType, value: Value, _args: &[Value]) -> OpResult {
    match device_type {
        "null" => Ok(Value::None),
        "stdout" | "console" => {
            match value {
                Value::String(s) => { print!("{}", s); Ok(Value::Number(s.len() as f64)) }
                Value::Number(n) => {
                    if matches!(dtype, DataType::Byte) {
                        print!("{}", (n as u8) as char);
                        Ok(Value::Number(1.0))
                    } else {
                        let s = n.to_string();
                        print!("{}", s);
                        Ok(Value::Number(s.len() as f64))
                    }
                }
                Value::List(items) => {
                    let mut written = 0;
                    for item in &items {
                        if let Value::Number(n) = item {
                            print!("{}", (*n as u8) as char);
                            written += 1;
                        }
                    }
                    Ok(Value::Number(written as f64))
                }
                _ => Ok(Value::Number(0.0)),
            }
        }
        "stderr" => {
            match value {
                Value::String(s) => { eprint!("{}", s); Ok(Value::Number(s.len() as f64)) }
                Value::Number(n) => {
                    if matches!(dtype, DataType::Byte) {
                        eprint!("{}", (n as u8) as char);
                        Ok(Value::Number(1.0))
                    } else {
                        let s = n.to_string();
                        eprint!("{}", s);
                        Ok(Value::Number(s.len() as f64))
                    }
                }
                _ => Ok(Value::Number(0.0)),
            }
        }
        "stdin" => Err(PointerError::new("Cannot PUSH to stdin device")),
        "serial" => {
            use std::fs::OpenOptions;
            use std::io::Write;
            let mut f = OpenOptions::new().write(true).open(device_id)
                .map_err(|e| PointerError::new(format!("Cannot open serial port '{}': {}", device_id, e)))?;
            let bytes: Vec<u8> = match value {
                Value::String(s) => s.into_bytes(),
                Value::Number(n) => vec![n as u8],
                Value::List(lst) => lst.iter().map(|v| match v {
                    Value::Number(n) => *n as u8, _ => 0
                }).collect(),
                _ => return Err(PointerError::new("Serial PUSH requires string, number, or list")),
            };
            f.write_all(&bytes)
                .map_err(|e| PointerError::new(format!("Serial write error: {}", e)))?;
            Ok(Value::Number(bytes.len() as f64))
        }
        _ => Err(PointerError::new(format!(
            "Device type '{}' (id: {}) not supported for PUSH",
            device_type, device_id
        ))),
    }
}

// ─── Network Operations ────────────────────────────────────────────────

/// Network read operation - reads data from a network socket.
/// Note: Full implementation requires async I/O. This provides basic blocking reads.
fn pull_network(
    host: &str,
    port: &u16,
    stream: &mut Option<Arc<Mutex<std::net::TcpStream>>>,
    dtype: DataType,
    _args: &[Value],
) -> OpResult {
    use std::io::{Read, BufRead};
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};

    // Connect once; reuse on subsequent calls
    if stream.is_none() {
        let s = TcpStream::connect(format!("{}:{}", host, port))
            .map_err(|e| PointerError::new(format!("Connection failed to {}:{}: {}", host, port, e)))?;
        s.set_read_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| PointerError::new(format!("Socket timeout error: {}", e)))?;
        *stream = Some(Arc::new(Mutex::new(s)));
    }

    let arc = stream.as_ref().unwrap().clone();
    let mut guard = arc.lock().map_err(|_| PointerError::new("Network mutex poisoned"))?;

    match dtype {
        DataType::Byte => {
            let mut buf = [0u8; 1];
            match guard.read(&mut buf) {
                Ok(0) => Ok(Value::None),
                Ok(_) => Ok(Value::Number(buf[0] as f64)),
                Err(e) => Err(PointerError::new(format!("Network read error: {}", e))),
            }
        }
        DataType::Int => {
            let mut buf = [0u8; 4];
            guard.read_exact(&mut buf)
                .map_err(|e| PointerError::new(format!("Network read error: {}", e)))?;
            Ok(Value::Number(i32::from_le_bytes(buf) as f64))
        }
        DataType::Float => {
            let mut buf = [0u8; 8];
            guard.read_exact(&mut buf)
                .map_err(|e| PointerError::new(format!("Network read error: {}", e)))?;
            Ok(Value::Number(f64::from_le_bytes(buf)))
        }
        DataType::Str => {
            let mut reader = std::io::BufReader::new(&mut *guard);
            let mut line = String::new();
            let n = reader.read_line(&mut line)
                .map_err(|e| PointerError::new(format!("Network read error: {}", e)))?;
            if n == 0 { return Ok(Value::None); }
            Ok(Value::String(line.trim_end_matches(&['\n', '\r']).to_string()))
        }
        DataType::Bytes => {
            let mut buf = vec![0u8; 4096];
            let n = guard.read(&mut buf)
                .map_err(|e| PointerError::new(format!("Network read error: {}", e)))?;
            Ok(Value::List(buf[..n].iter().map(|&b| Value::Number(b as f64)).collect()))
        }
    }
}

/// Network write — connects once, keeps socket alive for subsequent PUSH calls.
fn push_network(
    host: &str,
    port: &u16,
    stream: &mut Option<Arc<Mutex<std::net::TcpStream>>>,
    dtype: DataType,
    value: Value,
    _args: &[Value],
) -> OpResult {
    use std::io::Write;
    use std::net::TcpStream;
    use std::sync::{Arc, Mutex};

    if stream.is_none() {
        let s = TcpStream::connect(format!("{}:{}", host, port))
            .map_err(|e| PointerError::new(format!("Connection failed to {}:{}: {}", host, port, e)))?;
        s.set_write_timeout(Some(std::time::Duration::from_secs(10)))
            .map_err(|e| PointerError::new(format!("Socket timeout error: {}", e)))?;
        *stream = Some(Arc::new(Mutex::new(s)));
    }

    let arc = stream.as_ref().unwrap().clone();
    let mut guard = arc.lock().map_err(|_| PointerError::new("Network mutex poisoned"))?;

    let bytes: Vec<u8> = match (dtype, value) {
        (DataType::Byte, Value::Number(n)) => vec![n as u8],
        (DataType::Int,  Value::Number(n)) => (n as i32).to_le_bytes().to_vec(),
        (DataType::Float,Value::Number(n)) => n.to_le_bytes().to_vec(),
        (DataType::Str,  Value::String(s)) => { let mut b = s.into_bytes(); b.push(b'\n'); b }
        (DataType::Bytes,Value::List(lst)) => lst.iter().map(|v| match v {
            Value::Number(n) => *n as u8, _ => 0
        }).collect(),
        _ => return Err(PointerError::new("PUSH.NET: type mismatch")),
    };

    guard.write_all(&bytes)
        .map_err(|e| PointerError::new(format!("Network write error: {}", e)))?;
    Ok(Value::Number(bytes.len() as f64))
}
