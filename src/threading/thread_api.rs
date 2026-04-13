//! src/threading/thread_api.rs
//!
//! Public API surface used by executor.rs, repl.rs, and shell CLI.
//! All thread spawning for script pipelines should go through here.

use std::sync::mpsc;
use std::fs;
use anyhow::{Result, anyhow};

use crate::interpreter::Executor;
use crate::threading::threads::{PastaThread, next_thread_id, register_thread, finish_thread, list_threads, kill_thread};


/// Return the current thread's PASTA thread id if the thread name is "THID:<id>".
pub fn current_pasta_thid() -> Option<u64> {
    std::thread::current()
        .name()
        .and_then(|n| n.strip_prefix("THID:"))
        .and_then(|s| s.parse::<u64>().ok())
}

/// Spawn a single script file as an independent PASTA thread.
/// Returns the assigned PASTA thread ID.
pub fn spawn_script_thread(path: &str, name: impl Into<String>) -> Result<u64> {
    let name = name.into();
    let abs_path = {
        let p = std::path::Path::new(path);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            std::env::current_dir().unwrap_or_default().join(p)
        }
    };

    let src = fs::read_to_string(&abs_path)
        .map_err(|e| anyhow!("failed to read '{}': {}", abs_path.display(), e))?;

    let id = next_thread_id();
    let (kill_tx, kill_rx) = mpsc::sync_channel::<()>(1);
    let t = PastaThread::new(id, name.clone()).with_kill_tx(kill_tx);
    register_thread(t);

    let thread_name = format!("THID:{}", id);
    std::thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            // Wire the kill receiver into the thread-local so execute_statement
            // can poll it on every statement boundary.
            crate::interpreter::executor::set_kill_rx(kill_rx);
            let result = Executor::run(&src);
            match result {
                Ok(_) => finish_thread(id),
                Err(e) => {
                    let msg = e.to_string();
                    // "thread killed" is a clean kill signal, not a real error.
                    if msg != "thread killed" {
                        eprintln!("[THID:{}] error: {}", id, msg);
                    }
                    crate::threading::threads::global_registry()
                        .lock().unwrap()
                        .threads.get_mut(&id)
                        .map(|t| {
                            if t.status == crate::threading::threads::ThreadStatus::Running {
                                if msg == "thread killed" {
                                    t.mark_killed();
                                } else {
                                    t.mark_errored(msg);
                                }
                            }
                        });
                }
            }
        })
        .map_err(|e| anyhow!("failed to spawn thread '{}': {}", name, e))?;

    Ok(id)
}

/// Kill a thread by PASTA thread ID. Returns true if found and signal sent.
pub fn kill_thread_by_id(id: u64) -> bool {
    kill_thread(id)
}

/// Return a formatted snapshot of all threads for display in :threads.
pub fn threads_snapshot() -> Vec<ThreadDisplayRow> {
    list_threads().into_iter().map(|t| ThreadDisplayRow {
        id:         t.id,
        name:       t.name.clone(),
        status:     t.status.to_string(),
        elapsed_ms: t.elapsed_ms(),
        pid:        t.child_pid,
    }).collect()
}

/// A display-ready row for :threads output.
pub struct ThreadDisplayRow {
    /// Internal thread id.
    pub id:         u64,
    /// Human-readable thread name.
    pub name:       String,
    /// Current status label.
    pub status:     String,
    /// Elapsed runtime in milliseconds.
    pub elapsed_ms: u64,
    /// Child process id, if the thread owns one.
    pub pid:        Option<u32>,
}

impl std::fmt::Display for ThreadDisplayRow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f,
            "  THID:{:<4}  {:<32}  {:<12}  {:<6}  {}ms",
            self.id, self.name, self.status, match self.pid { Some(p) => format!("{}", p), None => "-".to_string() }, self.elapsed_ms
        )
    }
}
/// Set OS child PID for a PASTA thread (convenience wrapper).
pub fn set_thread_child_pid(id: u64, pid: u32) -> bool {
    crate::threading::threads::set_thread_child_pid(id, pid)
}
