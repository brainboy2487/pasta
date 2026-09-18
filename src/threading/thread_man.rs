//! src/threading/thread_man.rs
//!
//! High-level ThreadManager: wraps the global registry and OS thread spawning.
//! Replaces/supersedes runtime/threading.rs for interpreter-aware thread management.
//!
//! Stubs are marked TODO for the process manager expansion in :threads UI.

use std::sync::mpsc;
use std::thread;
use anyhow::{Result, anyhow};

use crate::interpreter::Executor;
use crate::threading::threads::{PastaThread, next_thread_id, register_thread, finish_thread};

/// Spawn a script thread and register it globally.
///
/// Note: if this thread spawns an external OS process (via std::process::Command::spawn),
/// call crate::threading::threads::set_thread_child_pid(thid, pid) after spawning to record the OS PID.

/// Returns the assigned PASTA thread ID.
///
/// The `task` closure receives a fresh `Executor` and a kill receiver.
/// It should check `kill_rx.try_recv().is_ok()` periodically and exit early if killed.
pub fn spawn_registered<F>(name: impl Into<String>, task: F) -> Result<u64>
where
    F: FnOnce(Executor, mpsc::Receiver<()>) -> Result<()> + Send + 'static,
{
    let name = name.into();
    let id = next_thread_id();
    let (kill_tx, kill_rx) = mpsc::sync_channel::<()>(1);

    let t = PastaThread::new(id, name.clone()).with_kill_tx(kill_tx);
    register_thread(t);

    let thread_name = name.clone();
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            let exe = Executor::new();
            let result = task(exe, kill_rx);
            if let Err(e) = result {
                eprintln!("[THID:{}] error: {}", id, e);
                crate::threading::threads::global_registry()
                    .lock().unwrap()
                    .threads.get_mut(&id)
                    .map(|t| t.mark_errored(e.to_string()));
            } else {
                finish_thread(id);
            }
        })
        .map_err(|e| anyhow!("failed to spawn thread '{}': {}", name, e))?;

    Ok(id)
}

/// A high-level manager handle.
/// Primarily used to spawn groups of coordinated threads (pipeline stages).
pub struct ThreadManager {
    /// IDs of threads spawned through this manager.
    pub spawned: Vec<u64>,
}

impl ThreadManager {
    pub fn new() -> Self {
        Self { spawned: Vec::new() }
    }

    /// Spawn a script thread, register it, and track its ID.
    pub fn spawn<F>(&mut self, name: impl Into<String>, task: F) -> Result<u64>
    where
        F: FnOnce(Executor, mpsc::Receiver<()>) -> Result<()> + Send + 'static,
    {
        let id = spawn_registered(name, task)?;
        self.spawned.push(id);
        Ok(id)
    }

    /// Kill all threads spawned by this manager.
    pub fn kill_all(&self) {
        for &id in &self.spawned {
            crate::threading::threads::kill_thread(id);
        }
    }

    /// Wait for all spawned threads to finish (polling-based).
    /// Returns when all threads are in Finished, Killed, or Errored state.
    /// `poll_interval_ms` controls how often to check (default 50ms).
    pub fn wait_all(&self, poll_interval_ms: Option<u64>) {
        use std::thread;
        use std::time::Duration;
        use crate::threading::threads::{global_registry, ThreadStatus};

        let interval = Duration::from_millis(poll_interval_ms.unwrap_or(50));

        loop {
            let all_done = {
                let reg = global_registry();
                let guard = reg.lock().unwrap();
                self.spawned.iter().all(|&id| {
                    match guard.threads.get(&id) {
                        None => true, // Thread not found = already cleaned up
                        Some(t) => !matches!(t.status, ThreadStatus::Running),
                    }
                })
            };

            if all_done {
                break;
            }

            thread::sleep(interval);
        }
    }

    /// Get status of all spawned threads.
    /// Returns a Vec of (thread_id, name, status_string).
    pub fn status_all(&self) -> Vec<(u64, String, String)> {
        use crate::threading::threads::global_registry;

        let reg = global_registry();
        let guard = reg.lock().unwrap();

        self.spawned
            .iter()
            .filter_map(|&id| {
                guard.threads.get(&id).map(|t| {
                    (t.id, t.name.clone(), t.status.to_string())
                })
            })
            .collect()
    }

    /// Join all threads: wait for completion, then return their final statuses.
    /// This is a blocking operation.
    pub fn join_all(&self, poll_interval_ms: Option<u64>) -> Vec<(u64, String, String)> {
        self.wait_all(poll_interval_ms);
        self.status_all()
    }
}

impl Default for ThreadManager {
    fn default() -> Self { Self::new() }
}
