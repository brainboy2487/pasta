//! Pointer Registry - global storage for all active pointers
//!
//! Provides thread-safe registration, lookup, and management of pointers.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use super::pointer::{Pointer, PointerId};

/// Global pointer registry
pub struct PointerRegistry {
    /// Map of pointer ID to pointer
    pointers: HashMap<PointerId, Pointer>,
    /// Next available pointer ID
    next_id: PointerId,
}

impl PointerRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            pointers: HashMap::new(),
            next_id: 1,
        }
    }

    /// Create a new shared (thread-safe) registry
    pub fn new_shared() -> SharedPointerRegistry {
        Arc::new(RwLock::new(Self::new()))
    }

    /// Allocate a new pointer ID
    fn alloc_id(&mut self) -> PointerId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Register a memory pointer
    pub fn register_mem(&mut self, size: usize) -> PointerId {
        let id = self.alloc_id();
        let ptr = Pointer::new_mem(id, size);
        self.pointers.insert(id, ptr);
        id
    }

    /// Register a memory pointer with initial data
    pub fn register_mem_with_data(&mut self, data: Vec<u8>) -> PointerId {
        let id = self.alloc_id();
        let mut ptr = Pointer::new_mem(id, data.len());
        if let super::pointer::PointerTarget::Memory { data: ref mut d, .. } = ptr.target {
            *d = data;
        }
        self.pointers.insert(id, ptr);
        id
    }

    /// Register a file pointer
    pub fn register_file(&mut self, path: String, mode: String) -> PointerId {
        let id = self.alloc_id();
        let ptr = Pointer::new_file(id, path, mode);
        self.pointers.insert(id, ptr);
        id
    }

    /// Register a device pointer
    pub fn register_device(&mut self, device_id: String, device_type: String) -> PointerId {
        let id = self.alloc_id();
        let ptr = Pointer::new_device(id, device_id, device_type);
        self.pointers.insert(id, ptr);
        id
    }

    /// Register a network pointer
    pub fn register_network(&mut self, host: String, port: u16) -> PointerId {
        let id = self.alloc_id();
        let ptr = Pointer::new_network(id, host, port);
        self.pointers.insert(id, ptr);
        id
    }

    /// Look up a pointer by ID
    pub fn lookup(&self, id: PointerId) -> Option<&Pointer> {
        self.pointers.get(&id)
    }

    /// Look up a pointer mutably by ID
    pub fn lookup_mut(&mut self, id: PointerId) -> Option<&mut Pointer> {
        self.pointers.get_mut(&id)
    }

    /// Kill (mark as dead) a pointer
    pub fn kill(&mut self, id: PointerId) -> bool {
        if let Some(ptr) = self.pointers.get_mut(&id) {
            ptr.kill();
            true
        } else {
            false
        }
    }

    /// Remove a dead pointer from the registry
    pub fn remove(&mut self, id: PointerId) -> Option<Pointer> {
        self.pointers.remove(&id)
    }

    /// Get info about a pointer
    pub fn info(&self, id: PointerId) -> Option<crate::interpreter::Value> {
        self.pointers.get(&id).map(|p| p.info())
    }

    /// Mark a pointer as temporary (auto-freed on scope exit)
    pub fn set_temporary(&mut self, id: PointerId, temp: bool) {
        if let Some(ptr) = self.pointers.get_mut(&id) {
            ptr.temporary = temp;
        }
    }

    /// Get all temporary pointers
    pub fn get_temporary_pointers(&self) -> Vec<PointerId> {
        self.pointers.iter()
            .filter(|(_, p)| p.temporary && p.alive)
            .map(|(id, _)| *id)
            .collect()
    }

    /// Free all temporary pointers (called on DO-block exit)
    pub fn free_temporaries(&mut self) -> Vec<PointerId> {
        let temps: Vec<PointerId> = self.get_temporary_pointers();
        for id in &temps {
            self.kill(*id);
        }
        temps
    }

    /// Count of live pointers
    pub fn live_count(&self) -> usize {
        self.pointers.values().filter(|p| p.alive).count()
    }

    /// Count of all pointers (including dead)
    pub fn total_count(&self) -> usize {
        self.pointers.len()
    }

    /// Sweep dead pointers from registry
    pub fn sweep_dead(&mut self) -> usize {
        let dead: Vec<PointerId> = self.pointers.iter()
            .filter(|(_, p)| !p.alive)
            .map(|(id, _)| *id)
            .collect();
        let count = dead.len();
        for id in dead {
            self.pointers.remove(&id);
        }
        count
    }
}

impl Default for PointerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-safe wrapper around PointerRegistry
pub type SharedPointerRegistry = Arc<RwLock<PointerRegistry>>;

/// Create a new shared pointer registry
pub fn new_shared_registry() -> SharedPointerRegistry {
    Arc::new(RwLock::new(PointerRegistry::new()))
}


