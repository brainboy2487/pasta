// src/concurrency_core/scheduler.rs
//! Minimal runtime scheduler skeleton.
//! - Uses numeric u64 task ids for maps and lookups.
//! - Synchronous, intentionally small for initial testing.
//! - Add cooperative/preemptive scheduling later as needed.

use crate::concurrency_core::task::{TaskHandle, TaskStatus};
use crate::concurrency_core::clock::LogicalClock;
use crate::concurrency_core::lineage::LineageRegistry;
use std::collections::HashMap;

/// Minimal runtime scheduler.
/// Public fields are kept for tests and simple integration; consider encapsulating later.
pub struct Runtime {
    /// Map from numeric task id to status.
    pub tasks: HashMap<u64, TaskStatus>,
    /// Map from numeric task id to logical clock.
    pub clocks: HashMap<u64, LogicalClock>,
    /// Lineage registry for task relationships.
    pub lineage: LineageRegistry,
}

impl Runtime {
    /// Create a new runtime scheduler.
    pub fn new() -> Self {
        Self {
            tasks: HashMap::new(),
            clocks: HashMap::new(),
            lineage: LineageRegistry::new(),
        }
    }

    /// Register a spawned task with the scheduler.
    /// Returns Err if the id is already registered.
    pub fn spawn_task(&mut self, handle: TaskHandle) -> Result<(), String> {
        let id = handle.id;
        if self.tasks.contains_key(&id) {
            return Err(format!("task id {} already registered", id));
        }
        self.tasks.insert(id, TaskStatus::Running);
        self.clocks.insert(id, LogicalClock::zero());
        // Optionally register lineage here: self.lineage.register(node)
        Ok(())
    }

    /// Return the current status for a handle.
    /// If the task is not found, return an Errored status with a message.
    pub fn task_status(&self, handle: &TaskHandle) -> TaskStatus {
        self.tasks
            .get(&handle.id)
            .cloned()
            .unwrap_or(TaskStatus::Errored("not found".into()))
    }

    /// Advance the logical clock for the given task handle by one tick.
    /// No-op if the task is not registered.
    pub fn tick(&mut self, handle: &TaskHandle) {
        if let Some(c) = self.clocks.get_mut(&handle.id) {
            c.tick();
        }
    }

    /// Unregister a task id from scheduler maps. Best-effort.
    pub fn unregister_task(&mut self, id: u64) {
        self.tasks.remove(&id);
        self.clocks.remove(&id);
        // Optionally remove from lineage: self.lineage.remove(id)
    }

    /// Convenience: ensure a task id exists in maps (idempotent).
    pub fn ensure_registered(&mut self, id: u64) {
        self.tasks.entry(id).or_insert(TaskStatus::Running);
        self.clocks.entry(id).or_insert_with(LogicalClock::zero);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency_core::task::TaskHandle;

    #[test]
    fn spawn_and_tick() {
        let mut rt = Runtime::new();
        let h = TaskHandle::new(42, 0);
        assert!(rt.spawn_task(h).is_ok());
        let h2 = TaskHandle::new(42, 0);
        // status should be Running
        assert_eq!(rt.task_status(&h2), TaskStatus::Running);
        // tick should advance clock
        rt.tick(&h2);
        assert_eq!(rt.clocks.get(&42).map(|c| c.get()), Some(1));
        // unregister removes entries
        rt.unregister_task(42);
        assert!(matches!(rt.task_status(&h2), TaskStatus::Errored(_)));
    }
}
