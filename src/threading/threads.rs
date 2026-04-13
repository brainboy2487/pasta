//! src/threading/threads.rs
//!
//! Core thread primitives for PASTA:
//!   - ThreadStatus  — lifecycle enum
//!   - PastaThread   — one entry in the global registry
//!   - THREAD_REGISTRY — process-wide Arc<Mutex<ThreadRegistry>>
//!
//! All spawned script threads register here so :threads in the REPL
//! can always see them, regardless of which Executor spawned them.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
use libc;

/// Lifecycle of a PASTA script thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadStatus {
    /// Thread has been registered and is actively running.
    Running,
    /// Thread completed normally.
    Finished,
    /// Thread was killed via :threads:kill.
    Killed,
    /// Thread exited with an error.
    Errored(String),
}

impl std::fmt::Display for ThreadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ThreadStatus::Running       => write!(f, "running"),
            ThreadStatus::Finished      => write!(f, "finished"),
            ThreadStatus::Killed        => write!(f, "killed"),
            ThreadStatus::Errored(e)    => write!(f, "errored: {}", e),
        }
    }
}

/// A single thread entry in the global registry.
#[derive(Debug, Clone)]
pub struct PastaThread {
    /// Monotonically increasing ID, starting at 1.
    pub id: u64,
    /// Human-readable name (e.g. "repl-pipeline-left").
    pub name: String,
    /// Current lifecycle status.
    pub status: ThreadStatus,
    /// Unix-ms timestamp when the thread was registered.
    pub started_at_ms: u64,
    /// Unix-ms timestamp when the thread finished/was killed (0 = still running).
    pub ended_at_ms: u64,
    /// Sender half of a kill channel. Send `()` to request graceful shutdown.
    /// The running thread should poll this channel periodically.
    /// None if kill is not yet wired (stub threads).
    #[doc(hidden)]
    pub kill_tx: Option<std::sync::mpsc::SyncSender<()>>,
    /// Optional OS child PID if this thread spawned an external process.
    pub child_pid: Option<u32>,
    /// If this thread is part of a script pipeline, the pipeline's unique ID.
    pub pipeline_id: Option<u64>,
    /// 0-based index of this stage within its pipeline.
    pub pipeline_stage: Option<usize>,
    /// Total number of stages in the pipeline.
    pub pipeline_total: Option<usize>,
}

impl PastaThread {
    /// Create a new Running entry with no kill channel.
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            status: ThreadStatus::Running,
            started_at_ms: now_ms(),
            ended_at_ms: 0,
            kill_tx: None,
            child_pid: None,
            pipeline_id: None,
            pipeline_stage: None,
            pipeline_total: None,
        }
    }

    /// Attach a kill-channel sender so this thread can be stopped.
    pub fn with_kill_tx(mut self, tx: std::sync::mpsc::SyncSender<()>) -> Self {
        self.kill_tx = Some(tx);
        self
    }


    /// Attach an OS child PID to this thread record.
    pub fn with_child_pid(mut self, pid: u32) -> Self {
        self.child_pid = Some(pid);
        self
    }

    /// Set child PID on an existing thread record (mutable).
    pub fn set_child_pid(&mut self, pid: u32) {
        self.child_pid = Some(pid);
    }

    /// Mark the thread as finished and record the end timestamp.
    pub fn mark_finished(&mut self) {
        self.status = ThreadStatus::Finished;
        self.ended_at_ms = now_ms();
    }

    /// Mark the thread as killed and record the end timestamp.
    pub fn mark_killed(&mut self) {
        self.status = ThreadStatus::Killed;
        self.ended_at_ms = now_ms();
    }

    /// Mark the thread as errored.
    pub fn mark_errored(&mut self, msg: impl Into<String>) {
        self.status = ThreadStatus::Errored(msg.into());
        self.ended_at_ms = now_ms();
    }

    /// Send a kill signal. Returns true if the signal was sent, false if no kill_tx.
    pub fn send_kill(&self) -> bool {
        if let Some(ref tx) = self.kill_tx {
            tx.try_send(()).is_ok()
        } else {
            false
        }
    }

    /// Elapsed milliseconds since the thread started (0 if clock unavailable).
    pub fn elapsed_ms(&self) -> u64 {
        let end = if self.ended_at_ms > 0 { self.ended_at_ms } else { now_ms() };
        end.saturating_sub(self.started_at_ms)
    }
}

/// The flat map backing the global registry.
pub struct ThreadRegistry {
    pub threads: HashMap<u64, PastaThread>,
    next_id: u64,
}

impl ThreadRegistry {
    pub fn new() -> Self {
        Self { threads: HashMap::new(), next_id: 1 }
    }

    /// Allocate the next thread ID (monotonically increasing).
    pub fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Register a thread. Returns its assigned ID.
    pub fn register(&mut self, t: PastaThread) -> u64 {
        let id = t.id;
        self.threads.insert(id, t);
        id
    }

    /// Mark a thread finished by ID.
    pub fn finish(&mut self, id: u64) {
        if let Some(t) = self.threads.get_mut(&id) {
            t.mark_finished();
        }
    }

    /// Send a kill signal and mark as killed.
    pub fn kill(&mut self, id: u64) -> bool {
        if let Some(t) = self.threads.get_mut(&id) {
            // If this thread recorded an OS child PID, attempt to kill that process first (Unix only).
            if let Some(pid) = t.child_pid {
                #[cfg(unix)] {
                    unsafe { libc::kill(pid as i32, libc::SIGTERM); }
                }
                t.mark_killed();
                return true;
            }
            // Fall back to sending the internal kill channel if present.
            t.send_kill();
                    t.mark_killed();
            true
        } else {
            false
        }
    }
    /// Sorted snapshot of all threads (for display).
    pub fn snapshot(&self) -> Vec<PastaThread> {
        let mut v: Vec<PastaThread> = self.threads.values().cloned().collect();
        v.sort_by_key(|t| t.id);
        v
    }

    /// Remove finished/killed threads older than `max_age_ms`. Returns count removed.
    pub fn gc(&mut self, max_age_ms: u64) -> usize {
        let now = now_ms();
        let before = self.threads.len();
        self.threads.retain(|_, t| {
            match t.status {
                ThreadStatus::Running => true,
                _ => {
                    if t.ended_at_ms == 0 { return false; }
                    now.saturating_sub(t.ended_at_ms) < max_age_ms
                }
            }
        });
        before - self.threads.len()
    }
}

impl Default for ThreadRegistry {
    fn default() -> Self { Self::new() }
}

// ── Global singleton ─────────────────────────────────────────────────────────

/// Process-wide thread registry. Shared by all executors and the REPL.
pub static THREAD_REGISTRY: OnceLock<Arc<Mutex<ThreadRegistry>>> = OnceLock::new();

/// Get (or initialize) the global registry.
pub fn global_registry() -> Arc<Mutex<ThreadRegistry>> {
    THREAD_REGISTRY.get_or_init(|| Arc::new(Mutex::new(ThreadRegistry::new()))).clone()
}

/// Allocate the next globally unique thread ID.
pub fn next_thread_id() -> u64 {
    global_registry().lock().unwrap().alloc_id()
}

/// Register a new thread in the global registry.
pub fn register_thread(t: PastaThread) -> u64 {
    global_registry().lock().unwrap().register(t)
}

/// Set the OS child PID for a registered thread.
pub fn set_thread_child_pid(id: u64, pid: u32) -> bool {
    let binding = global_registry();
    let mut reg = binding.lock().unwrap();
    if let Some(t) = reg.threads.get_mut(&id) {
        t.child_pid = Some(pid);
        true
    } else {
        false
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Mark a registered thread as finished.
pub fn finish_thread(id: u64) {
    global_registry().lock().unwrap().finish(id);
}

/// Send kill signal and mark a thread as killed. Returns true if found.
pub fn kill_thread(id: u64) -> bool {
    global_registry().lock().unwrap().kill(id)
}

/// Return a sorted snapshot of all registered threads.
pub fn list_threads() -> Vec<PastaThread> {
    global_registry().lock().unwrap().snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_finish() {
        let id = next_thread_id();
        let t = PastaThread::new(id, "test-thread");
        register_thread(t);
        let snap = list_threads();
        assert!(snap.iter().any(|t| t.id == id && t.status == ThreadStatus::Running));
        finish_thread(id);
        let snap2 = list_threads();
        assert!(snap2.iter().any(|t| t.id == id && t.status == ThreadStatus::Finished));
    }

    #[test]
    fn kill_sends_signal() {
        let id = next_thread_id();
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        let t = PastaThread::new(id, "killable").with_kill_tx(tx);
        register_thread(t);
        let killed = kill_thread(id);
        assert!(killed);
        // Signal should have been sent
        assert!(rx.try_recv().is_ok());
    }
}
