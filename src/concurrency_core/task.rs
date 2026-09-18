// src/concurrency_core/task.rs
//! Task handle and status types for concurrency_core.
//! - TaskHandle.id is a numeric u64 (unique registered id).
//! - Epoch remains u64 for resume semantics.
//! - Types are public for use by integration tests and the bridge.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Interpreter-visible handle for a spawned task.
/// id is a numeric `u64` to match the runtime registry.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct TaskHandle {
    /// Unique numeric task id.
    pub id: u64,
    /// Epoch or generation for resume semantics.
    pub epoch: u64,
}

impl TaskHandle {
    /// Create a new TaskHandle from numeric id and epoch.
    pub fn new(id: u64, epoch: u64) -> Self {
        Self { id, epoch }
    }

    /// Convenience: create a handle with epoch 0.
    pub fn with_id(id: u64) -> Self {
        Self { id, epoch: 0 }
    }

    /// Increment the epoch in place.
    pub fn bump_epoch(&mut self) {
        self.epoch = self.epoch.wrapping_add(1);
    }

    /// Return the id as a string (useful for logging or legacy code).
    pub fn id_string(&self) -> String {
        self.id.to_string()
    }
}

impl fmt::Display for TaskHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TaskHandle(id={}, epoch={})", self.id, self.epoch)
    }
}

impl From<u64> for TaskHandle {
    fn from(id: u64) -> Self {
        TaskHandle::with_id(id)
    }
}

impl From<TaskHandle> for u64 {
    fn from(h: TaskHandle) -> Self {
        h.id
    }
}

/// Status of a task in the runtime.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskStatus {
    Running,
    Finished,
    Killed,
    Errored(String),
}

impl TaskStatus {
    /// Helper to check if the task is terminal (finished, killed, or errored).
    pub fn is_terminal(&self) -> bool {
        matches!(self, TaskStatus::Finished | TaskStatus::Killed | TaskStatus::Errored(_))
    }
}
