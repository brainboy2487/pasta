//! Pointer object definition for PASTA v1.4.4
//!
//! A Pointer is a unified abstraction over different resource types:
//! - MEM: raw memory blocks
//! - FILE: file handles
//! - DEV: device handles (GPIO, serial, etc.)
//! - NET: network sockets

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use crate::interpreter::Value;

/// Unique identifier for a pointer
pub type PointerId = u64;

/// The kind of resource a pointer references
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerKind {
    /// Raw memory block
    Mem,
    /// File handle
    File,
    /// Device handle
    Dev,
    /// Network socket
    Net,
}

impl PointerKind {
    /// Parse a string into a PointerKind
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_uppercase().as_str() {
            "MEM" => Some(PointerKind::Mem),
            "FILE" => Some(PointerKind::File),
            "DEV" => Some(PointerKind::Dev),
            "NET" => Some(PointerKind::Net),
            _ => None,
        }
    }

    /// Convert to string representation
    pub fn as_str(&self) -> &'static str {
        match self {
            PointerKind::Mem => "MEM",
            PointerKind::File => "FILE",
            PointerKind::Dev => "DEV",
            PointerKind::Net => "NET",
        }
    }
}

/// Metadata attached to a pointer
#[derive(Debug, Clone, Default)]
pub struct PointerMetadata {
    /// Key-value pairs for custom metadata
    pub data: HashMap<String, Value>,
}

impl PointerMetadata {
    /// Create empty metadata
    pub fn new() -> Self {
        Self { data: HashMap::new() }
    }

    /// Set a metadata value
    pub fn set(&mut self, key: impl Into<String>, value: Value) {
        self.data.insert(key.into(), value);
    }

    /// Get a metadata value
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.data.get(key)
    }
}

/// The target of a pointer - what it actually points to
#[derive(Debug)]
pub enum PointerTarget {
    /// Memory block with raw bytes
    Memory {
        /// Allocated bytes
        data: Vec<u8>,
        /// Current read/write offset
        offset: usize,
    },
    /// File handle
    File {
        /// File path
        path: String,
        /// Current byte offset for streaming reads/writes
        offset: u64,
        /// File mode (r, w, rw, a)
        mode: String,
    },
    /// Device handle
    Device {
        /// Device identifier
        device_id: String,
        /// Device type (gpio, serial, etc.)
        device_type: String,
    },
    /// Network socket
    Network {
        /// Host address
        host: String,
        /// Port number
        port: u16,
        /// Persistent TCP stream — Arc<Mutex<>> so it survives clone (shared handle)
        stream: Option<Arc<Mutex<std::net::TcpStream>>>,
    },
}

impl Clone for PointerTarget {
    fn clone(&self) -> Self {
        match self {
            PointerTarget::Memory { data, offset } =>
                PointerTarget::Memory { data: data.clone(), offset: *offset },
            PointerTarget::File { path, offset, mode } =>
                PointerTarget::File { path: path.clone(), offset: *offset, mode: mode.clone() },
            PointerTarget::Device { device_id, device_type } =>
                PointerTarget::Device { device_id: device_id.clone(), device_type: device_type.clone() },
            PointerTarget::Network { host, port, stream } =>
                PointerTarget::Network { host: host.clone(), port: *port, stream: stream.clone() },
        }
    }
}

/// A Pointer object in PASTA
#[derive(Debug, Clone)]
pub struct Pointer {
    /// Unique identifier
    pub id: PointerId,
    /// Kind of pointer
    pub kind: PointerKind,
    /// What this pointer points to
    pub target: PointerTarget,
    /// Optional metadata
    pub metadata: PointerMetadata,
    /// Whether this pointer is still valid
    pub alive: bool,
    /// Whether this is a temporary pointer (auto-freed on scope exit)
    pub temporary: bool,
}

impl Pointer {
    /// Create a new memory pointer
    pub fn new_mem(id: PointerId, size: usize) -> Self {
        Self {
            id,
            kind: PointerKind::Mem,
            target: PointerTarget::Memory {
                data: vec![0u8; size],
                offset: 0,
            },
            metadata: PointerMetadata::new(),
            alive: true,
            temporary: false,
        }
    }

    /// Create a new file pointer
    pub fn new_file(id: PointerId, path: String, mode: String) -> Self {
        Self {
            id,
            kind: PointerKind::File,
            target: PointerTarget::File {
                path,
                offset: 0,
                mode,
            },
            metadata: PointerMetadata::new(),
            alive: true,
            temporary: false,
        }
    }

    /// Create a new device pointer
    pub fn new_device(id: PointerId, device_id: String, device_type: String) -> Self {
        Self {
            id,
            kind: PointerKind::Dev,
            target: PointerTarget::Device {
                device_id,
                device_type,
            },
            metadata: PointerMetadata::new(),
            alive: true,
            temporary: false,
        }
    }

    /// Create a new network pointer
    pub fn new_network(id: PointerId, host: String, port: u16) -> Self {
        Self {
            id,
            kind: PointerKind::Net,
            target: PointerTarget::Network {
                host,
                port,
                stream: None,
            },
            metadata: PointerMetadata::new(),
            alive: true,
            temporary: false,
        }
    }

    /// Mark pointer as dead (freed)
    pub fn kill(&mut self) {
        self.alive = false;
    }

    /// Check if pointer is usable
    pub fn is_valid(&self) -> bool {
        self.alive
    }

    /// Reset the read/write offset to 0 (MEM and FILE pointers)
    pub fn reset_offset(&mut self) {
        match &mut self.target {
            PointerTarget::Memory { offset, .. } => *offset = 0,
            PointerTarget::File { offset, .. } => *offset = 0,
            _ => {}
        }
    }

    /// Set the read/write offset to a specific position (MEM and FILE)
    pub fn seek(&mut self, position: usize) -> Result<(), String> {
        match &mut self.target {
            PointerTarget::Memory { data, offset } => {
                if position > data.len() {
                    return Err(format!("Seek position {} exceeds buffer size {}", position, data.len()));
                }
                *offset = position;
                Ok(())
            }
            PointerTarget::File { offset, .. } => {
                *offset = position as u64;
                Ok(())
            }
            _ => Err("SEEK only supported for MEM and FILE pointers".to_string()),
        }
    }

    /// Get info about this pointer as a Value (for INFO statement)
    /// Returns a list of [key, value] pairs representing pointer metadata
    pub fn info(&self) -> Value {
        let mut pairs = Vec::new();
        
        pairs.push(Value::List(vec![
            Value::String("id".to_string()),
            Value::Number(self.id as f64),
        ]));
        pairs.push(Value::List(vec![
            Value::String("kind".to_string()),
            Value::String(self.kind.as_str().to_string()),
        ]));
        pairs.push(Value::List(vec![
            Value::String("alive".to_string()),
            Value::Bool(self.alive),
        ]));
        pairs.push(Value::List(vec![
            Value::String("temporary".to_string()),
            Value::Bool(self.temporary),
        ]));
        
        // Add target-specific info
        match &self.target {
            PointerTarget::Memory { data, offset } => {
                pairs.push(Value::List(vec![
                    Value::String("size".to_string()),
                    Value::Number(data.len() as f64),
                ]));
                pairs.push(Value::List(vec![
                    Value::String("offset".to_string()),
                    Value::Number(*offset as f64),
                ]));
            }
            PointerTarget::File { path, offset, mode } => {
                pairs.push(Value::List(vec![
                    Value::String("path".to_string()),
                    Value::String(path.clone()),
                ]));
                pairs.push(Value::List(vec![
                    Value::String("offset".to_string()),
                    Value::Number(*offset as f64),
                ]));
                pairs.push(Value::List(vec![
                    Value::String("mode".to_string()),
                    Value::String(mode.clone()),
                ]));
            }
            PointerTarget::Device { device_id, device_type } => {
                pairs.push(Value::List(vec![
                    Value::String("device_id".to_string()),
                    Value::String(device_id.clone()),
                ]));
                pairs.push(Value::List(vec![
                    Value::String("device_type".to_string()),
                    Value::String(device_type.clone()),
                ]));
            }
            PointerTarget::Network { host, port, stream } => {
                pairs.push(Value::List(vec![
                    Value::String("host".to_string()),
                    Value::String(host.clone()),
                ]));
                pairs.push(Value::List(vec![
                    Value::String("port".to_string()),
                    Value::Number(*port as f64),
                ]));
                pairs.push(Value::List(vec![
                    Value::String("connected".to_string()),
                    Value::Bool(stream.is_some()),
                ]));
            }
        }
        
        // Add metadata as nested list of pairs
        if !self.metadata.data.is_empty() {
            let meta_pairs: Vec<Value> = self.metadata.data.iter()
                .map(|(k, v)| Value::List(vec![Value::String(k.clone()), v.clone()]))
                .collect();
            pairs.push(Value::List(vec![
                Value::String("metadata".to_string()),
                Value::List(meta_pairs),
            ]));
        }
        
        Value::List(pairs)
    }
}
