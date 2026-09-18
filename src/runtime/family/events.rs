// src/runtime/family/events.rs
//! Family event system — hybrid model.
//!
//! Internal runtime events flow through this module.
//! User Pasta code only sees DOES_PARENT_EXIST (TRUE/FALSE).
//! Global root-space events require USE UNSAFE-READ or USE UNSAFE-WRITE.

use std::sync::{Arc, Mutex};
use crate::runtime::family::types::{AdoptionEvent, AdoptionEventType, EventScope};

/// Callback for local read-only adoption events.
pub type LocalCallback  = Box<dyn Fn(&AdoptionEvent) + Send + 'static>;
/// Callback for global read-only adoption events.
pub type GlobalRCallback = Box<dyn Fn(&AdoptionEvent) + Send + 'static>;
/// Callback for global read/write adoption events.
pub type GlobalWCallback = Box<dyn Fn(&mut AdoptionEvent) + Send + 'static>;

/// Permission level granted to the current script/session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum UnsafePermission {
    /// Default: no access to global root-space events.
    None,
    /// USE UNSAFE-READ / --allow-unsafe: read-only global events.
    Read,
    /// USE UNSAFE-WRITE / --allow-unsafe-write: full ASM hooks.
    Write,
}

/// Central event bus for the family system.
pub struct FamilyEventBus {
    permission: UnsafePermission,
    local_subs:   Arc<Mutex<Vec<(AdoptionEventType, LocalCallback)>>>,
    global_r_subs: Arc<Mutex<Vec<(AdoptionEventType, GlobalRCallback)>>>,
    global_w_subs: Arc<Mutex<Vec<(AdoptionEventType, GlobalWCallback)>>>,
    event_log: Arc<Mutex<Vec<String>>>,
}

impl FamilyEventBus {
    /// Create a new event bus with the given unsafe-permission level.
    pub fn new(permission: UnsafePermission) -> Self {
        FamilyEventBus {
            permission,
            local_subs:    Arc::new(Mutex::new(Vec::new())),
            global_r_subs: Arc::new(Mutex::new(Vec::new())),
            global_w_subs: Arc::new(Mutex::new(Vec::new())),
            event_log:     Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Subscribe to a local userspace event (always allowed).
    pub fn subscribe_local(&self, event: AdoptionEventType, cb: LocalCallback) {
        self.local_subs.lock().unwrap().push((event, cb));
    }

    /// Subscribe to a global read-only event (requires Read or Write permission).
    pub fn subscribe_global_read(
        &self,
        event: AdoptionEventType,
        cb: GlobalRCallback,
    ) -> Result<(), crate::runtime::family::errors::LineageError> {
        if self.permission < UnsafePermission::Read {
            return Err(crate::runtime::family::errors::LineageError::PermissionDenied {
                action: "subscribe_global_read".into(),
                required_permission: "USE UNSAFE-READ".into(),
            });
        }
        self.global_r_subs.lock().unwrap().push((event, cb));
        Ok(())
    }

    /// Subscribe to a global read/write event hook (requires Write permission).
    pub fn subscribe_global_write(
        &self,
        event: AdoptionEventType,
        cb: GlobalWCallback,
    ) -> Result<(), crate::runtime::family::errors::LineageError> {
        if self.permission < UnsafePermission::Write {
            return Err(crate::runtime::family::errors::LineageError::PermissionDenied {
                action: "subscribe_global_write".into(),
                required_permission: "USE UNSAFE-WRITE".into(),
            });
        }
        self.global_w_subs.lock().unwrap().push((event, cb));
        Ok(())
    }

    /// Emit an event through all applicable subscribers.
    pub fn emit(&self, mut event: AdoptionEvent) {
        // Log entry
        {
            let entry = format!("{:?} child={} scope={:?}", event.event_type, event.child_id, event.scope);
            let mut log = self.event_log.lock().unwrap();
            if log.len() >= 256 { log.remove(0); }
            log.push(entry);
        }

        // Local subs: always delivered for LocalUserspace events
        if event.scope == EventScope::LocalUserspace {
            let subs = self.local_subs.lock().unwrap();
            for (kind, cb) in subs.iter() {
                if *kind == event.event_type { cb(&event); }
            }
        }

        // Global read subs
        if event.scope == EventScope::GlobalRootSpace && self.permission >= UnsafePermission::Read {
            let subs = self.global_r_subs.lock().unwrap();
            for (kind, cb) in subs.iter() {
                if *kind == event.event_type { cb(&event); }
            }
        }

        // Global write subs (can mutate the event)
        if event.scope == EventScope::GlobalRootSpace && self.permission >= UnsafePermission::Write {
            let mut subs = self.global_w_subs.lock().unwrap();
            for (kind, cb) in subs.iter_mut() {
                if *kind == event.event_type { cb(&mut event); }
            }
        }
    }

    /// Return a snapshot of the recent event log (used in diagnostics).
    pub fn log_snapshot(&self) -> Vec<String> {
        self.event_log.lock().unwrap().clone()
    }
}
