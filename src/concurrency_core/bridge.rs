// src/concurrency_core/bridge.rs
//! Bridge: integration surface between the Pasta interpreter and the concurrency_core runtime.
//!
//! - Non-blocking await semantics (interpreter continues executing other work).
//! - Anyhow error model.
//! - Numeric unique task ids (u64).
//! - try_wait returns Result<Option<TaskStatus>>.
//! - Snapshot API requires a SnapshotArg.
//! - Structured logging via formatted messages (keeps current `log` backend).

use anyhow::{anyhow, Result};
use log::info;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::concurrency_core::async_runtime::{AsyncRuntimeApi, LocalRuntime};
use crate::concurrency_core::clock::LogicalClock;
use crate::concurrency_core::task::{TaskHandle, TaskStatus};

/// Snapshot argument required by the bridge API.
/// - `Path(PathBuf, u64)` corresponds to PATHBUF(**).ID_number
/// - `ResultExpr(String, u64)` corresponds to RESULT(ARGS)<Status>.ID_number
#[derive(Debug, Clone)]
pub enum SnapshotArg {
    Path(PathBuf, u64),
    ResultExpr(String, u64),
}

/// Bridge is the single integration point. It owns a boxed runtime implementation.
/// The runtime must implement AsyncRuntimeApi.
pub struct Bridge {
    runtime: Box<dyn AsyncRuntimeApi + Send + Sync>,
    /// Optional local cache for quick lookups; protected by a mutex for simplicity.
    registry: Arc<Mutex<Vec<u64>>>,
}

impl Bridge {
    /// Default bridge backed by LocalRuntime.
    pub fn new() -> Self {
        let rt = LocalRuntime::new();
        Self {
            runtime: Box::new(rt),
            registry: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Create a bridge with a custom runtime implementation.
    pub fn new_with_runtime<R>(runtime: R) -> Self
    where
        R: AsyncRuntimeApi + Send + Sync + 'static,
    {
        Self {
            runtime: Box::new(runtime),
            registry: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Spawn a new task. Returns a TaskHandle with a numeric id.
    /// The bridge registers the id locally for bookkeeping.
    pub fn spawn(&mut self, name: &str) -> Result<TaskHandle> {
        let handle = self
            .runtime
            .spawn(name)
            .map_err(|e| anyhow!(e.to_string()))?;
        if let Ok(mut reg) = self.registry.lock() {
            reg.push(handle.id);
        }
        info!(
            target: "concurrency_core",
            "event=spawn task_id={} name={}",
            handle.id,
            name
        );
        Ok(handle)
    }

    /// Non-blocking probe: returns Ok(Some(status)) if finished or errored,
    /// Ok(None) if still running, Err on runtime error.
    pub fn try_wait(&mut self, handle: &TaskHandle) -> Result<Option<TaskStatus>> {
        let res = self
            .runtime
            .try_wait(handle)
            .map_err(|e| anyhow!(e.to_string()))?;
        match &res {
            Some(status) => info!(
                target: "concurrency_core",
                "event=try_wait task_id={} status={:?}",
                handle.id,
                status
            ),
            None => info!(
                target: "concurrency_core",
                "event=try_wait task_id={} status=running",
                handle.id
            ),
        }
        // If terminal, optionally unregister from registry (best-effort).
        if let Some(status) = &res {
            if status.is_terminal() {
                if let Ok(mut reg) = self.registry.lock() {
                    reg.retain(|&id| id != handle.id);
                }
            }
        }
        Ok(res)
    }

    /// Request cancellation. Non-blocking: returns Ok(()) if cancellation requested.
    pub fn cancel(&mut self, handle: &TaskHandle) -> Result<()> {
        self.runtime
            .cancel(handle)
            .map_err(|e| anyhow!(e.to_string()))?;
        info!(
            target: "concurrency_core",
            "event=cancel task_id={}",
            handle.id
        );
        Ok(())
    }

    /// Return the logical clock for a task.
    pub fn task_clock(&self, handle: &TaskHandle) -> Result<LogicalClock> {
        self.runtime
            .task_clock(handle)
            .map_err(|e| anyhow!(e.to_string()))
    }

    /// Persist a snapshot for the given task using a required SnapshotArg.
    /// Returns a token or path string identifying the snapshot.
    pub fn persist_snapshot(&mut self, arg: SnapshotArg) -> Result<String> {
        let token = match arg {
            SnapshotArg::Path(path, id) => {
                info!(
                    target: "concurrency_core",
                    "event=persist_snapshot task_id={} path={}",
                    id,
                    path.display()
                );
                self.runtime
                    .persist_snapshot_with_path(id, path)
                    .map_err(|e| anyhow!(e.to_string()))?
            }
            SnapshotArg::ResultExpr(expr, id) => {
                info!(
                    target: "concurrency_core",
                    "event=persist_snapshot task_id={} expr={}",
                    id,
                    expr
                );
                self.runtime
                    .persist_snapshot_with_expr(id, expr)
                    .map_err(|e| anyhow!(e.to_string()))?
            }
        };
        Ok(token)
    }

    /// Replace the runtime implementation at runtime (hot-swap).
    pub fn replace_runtime<R>(&mut self, runtime: R)
    where
        R: AsyncRuntimeApi + Send + Sync + 'static,
    {
        self.runtime = Box::new(runtime);
        info!(target: "concurrency_core", "event=replace_runtime");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::concurrency_core::async_runtime::LocalRuntime;

    #[test]
    fn bridge_spawn_try_wait_cycle() {
        let mut b = Bridge::new_with_runtime(LocalRuntime::new());
        let h = b.spawn("test").expect("spawn");
        // Immediately probe; LocalRuntime stub may return None (running).
        let p = b.try_wait(&h).expect("try_wait");
        assert!(p.is_none() || matches!(p, Some(TaskStatus::Finished) | Some(TaskStatus::Errored(_))));
    }
}
