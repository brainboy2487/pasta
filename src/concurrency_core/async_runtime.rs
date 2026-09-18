// src/concurrency_core/async_runtime.rs
//! Local async runtime adapter and trait used by the Bridge.
//!
//! - Exposes a small, stable trait `AsyncRuntimeApi` that the Bridge calls.
//! - `LocalRuntime` is a thin wrapper around the in-repo `Runtime` scheduler.
//! - Methods are intentionally synchronous and return `anyhow::Result` so the
//!   Bridge and interpreter can handle errors uniformly.
//!
//! NOTE: This file is a drop-in replacement that matches the scheduler and
//! task types we've been using (numeric `u64` task ids).

use anyhow::Result;
use std::path::PathBuf;

use crate::concurrency_core::clock::LogicalClock;
use crate::concurrency_core::scheduler::Runtime;
use crate::concurrency_core::task::{TaskHandle, TaskStatus};

/// Runtime API used by the Bridge. Implementations should provide non-blocking
/// probes and snapshot persistence hooks required by the Bridge.
pub trait AsyncRuntimeApi {
    /// Spawn a new task with a human-readable name. Returns a `TaskHandle`
    /// containing a unique numeric id assigned by the runtime.
    fn spawn(&mut self, name: &str) -> Result<TaskHandle>;

    /// Query the current status of a task.
    fn status(&self, handle: &TaskHandle) -> Result<TaskStatus>;

    /// Non-blocking probe: Ok(Some(status)) if finished/terminal, Ok(None) if still running.
    fn try_wait(&mut self, handle: &TaskHandle) -> Result<Option<TaskStatus>>;

    /// Request cancellation of a running task.
    fn cancel(&mut self, handle: &TaskHandle) -> Result<()>;

    /// Return the logical clock associated with a task.
    fn task_clock(&self, handle: &TaskHandle) -> Result<LogicalClock>;

    /// Persist a snapshot for the given task id to the provided filesystem path.
    /// Returns a token or path string identifying the snapshot.
    fn persist_snapshot_with_path(&mut self, id: u64, path: PathBuf) -> Result<String>;

    /// Persist a snapshot for the given task id using an expression/result descriptor.
    /// Returns a token or path string identifying the snapshot.
    fn persist_snapshot_with_expr(&mut self, id: u64, expr: String) -> Result<String>;
}

/// A tiny concrete runtime wrapper around the scheduler for local testing.
/// This keeps a deterministic `next_id` counter for reproducible ids.
pub struct LocalRuntime {
    pub inner: Runtime,
    next_id: u64,
}

impl LocalRuntime {
    /// Create a new LocalRuntime backed by the scheduler.
    pub fn new() -> Self {
        Self {
            inner: Runtime::new(),
            next_id: 1,
        }
    }

    /// Allocate a deterministic unique id.
    fn allocate_id(&mut self) -> u64 {
        let id = self.next_id;
        // wrap-around is extremely unlikely in tests; use wrapping_add for safety.
        self.next_id = self.next_id.wrapping_add(1);
        id
    }
}

impl AsyncRuntimeApi for LocalRuntime {
    fn spawn(&mut self, _name: &str) -> Result<TaskHandle> {
        // Allocate a numeric id and register with the scheduler.
        let id = self.allocate_id();
        let handle = TaskHandle::new(id, 0);
        // Register with scheduler; convert scheduler error into anyhow::Error
        self.inner
            .spawn_task(handle.clone())
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(handle)
    }

    fn status(&self, handle: &TaskHandle) -> Result<TaskStatus> {
        Ok(self.inner.task_status(handle))
    }

    fn try_wait(&mut self, handle: &TaskHandle) -> Result<Option<TaskStatus>> {
        // Non-blocking probe: return None if Running, Some(status) otherwise.
        let status = self.inner.task_status(handle);
        match status {
            TaskStatus::Running => Ok(None),
            other => Ok(Some(other)),
        }
    }

    fn cancel(&mut self, handle: &TaskHandle) -> Result<()> {
        // Best-effort cancellation: mark as Killed in the scheduler maps.
        // Real cancellation semantics (interrupting work) should be implemented
        // in the scheduler/executor layer.
        self.inner.tasks.insert(handle.id, TaskStatus::Killed);
        Ok(())
    }

    fn task_clock(&self, handle: &TaskHandle) -> Result<LogicalClock> {
        // Return the stored logical clock if present, otherwise zero.
        Ok(self
            .inner
            .clocks
            .get(&handle.id)
            .cloned()
            .unwrap_or_else(LogicalClock::zero))
    }

    fn persist_snapshot_with_path(&mut self, _id: u64, _path: PathBuf) -> Result<String> {
        // Stub implementation: return a placeholder token.
        // Replace with real persistence logic (serialize state, write to path).
        Ok("snapshot-token-path".to_string())
    }

    fn persist_snapshot_with_expr(&mut self, _id: u64, _expr: String) -> Result<String> {
        // Stub implementation: return a placeholder token.
        // Replace with real persistence logic that evaluates/records the expression result.
        Ok("snapshot-token-expr".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency_core::task::TaskStatus;

    #[test]
    fn local_runtime_spawn_and_status() {
        let mut rt = LocalRuntime::new();
        let h = rt.spawn("test-task").expect("spawn");
        // After spawn, scheduler should report Running
        let s = rt.status(&h).expect("status");
        assert!(matches!(s, TaskStatus::Running));
    }

    #[test]
    fn try_wait_behavior() {
        let mut rt = LocalRuntime::new();
        let h = rt.spawn("t").expect("spawn");
        let p = rt.try_wait(&h).expect("try_wait");
        // LocalRuntime stub reports Running by default, so probe returns None.
        assert!(p.is_none());
    }
}
