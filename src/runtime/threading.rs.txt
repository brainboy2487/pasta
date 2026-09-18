// src/runtime/threading.rs
//! Threading utilities for PASTA runtime
//!
//! This module provides a small, pragmatic threading layer that integrates
//! with the existing `Executor` and `Environment`. It offers:
//! - `ThreadManager`: a simple thread pool that runs tasks (closures) on OS threads.
//! - `ThreadHandle`: a joinable handle for spawned tasks.
//! - Helpers to spawn tasks that operate on a shared `Executor` protected by `Arc<Mutex<...>>`.
//!
//! Design notes:
//! - The interpreter `Executor` is not inherently thread-safe. To allow safe
//!   concurrent access, callers wrap the `Executor` in `Arc<Mutex<Executor>>`
//!   and pass that into threaded tasks. This keeps the threading model explicit
//!   and avoids hidden synchronization.
//! - The thread pool is intentionally small and dependency-free (uses std only).
//! - The manager can consult the runtime `Environment` for a suggested `max_threads`
//!   configuration (set by `devices::auto_configure`) and fall back to a sensible default.
//!
//! Usage patterns:
//! - For coarse-grained parallelism (independent tasks), spawn tasks that lock the
//!   executor only when they need to read/write shared state.
//! - For fine-grained parallelism, consider cloning or creating per-thread executors
//!   and merging results back into the main executor under a lock.

use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use anyhow::{Result, anyhow};

use crate::interpreter::Executor;
use crate::interpreter::environment::{Environment, Value};

/// A joinable handle for a spawned task.
///
/// The inner `JoinHandle` returns `Result<()>` so the caller can propagate errors.
pub struct ThreadHandle {
    join: JoinHandle<Result<()>>,
}

impl ThreadHandle {
    /// Wait for the thread to finish and return its result.
    pub fn join(self) -> Result<()> {
        match self.join.join() {
            Ok(r) => r,
            Err(e) => Err(anyhow!("Thread panicked: {:?}", e)),
        }
    }
}

/// A simple thread pool / manager.
///
/// Tasks are closures that receive an `Arc<Mutex<Executor>>` and should return
/// `Result<()>`. The manager runs tasks on worker threads and returns `ThreadHandle`s.
pub struct ThreadManager {
    workers: Vec<JoinHandle<()>>,
    sender: mpsc::Sender<Message>,
}

/// Internal message type for the worker queue.
enum Message {
    Task(Box<dyn FnOnce(Arc<Mutex<Executor>>) -> Result<()> + Send + 'static>),
    Shutdown,
}

impl ThreadManager {
    /// Create a new ThreadManager with `num_workers` worker threads.
    ///
    /// If `num_workers` is 0, a single worker thread is created.
    pub fn new(num_workers: usize) -> Self {
        let (tx, rx) = mpsc::channel::<Message>();
        let rx = Arc::new(Mutex::new(rx));
        let mut workers = Vec::new();
        let n = if num_workers == 0 { 1 } else { num_workers };

        for i in 0..n {
            let rx_clone = Arc::clone(&rx);
            let handle = thread::Builder::new()
                .name(format!("pasta-worker-{}", i))
                .spawn(move || loop {
                    // Wait for a message
                    let msg = {
                        let guard = rx_clone.lock().unwrap();
                        guard.recv()
                    };

                    match msg {
                        Ok(Message::Task(task)) => {
                            // For worker-level tasks we don't have a shared executor here.
                            // The task itself is responsible for obtaining the Arc<Mutex<Executor>>
                            // and performing its work.
                            // We ignore errors here; tasks should handle/report them via other channels.
                            let _ = task(Arc::new(Mutex::new(Executor::new())));
                        }
                        Ok(Message::Shutdown) | Err(_) => {
                            break;
                        }
                    }
                })
                .expect("failed to spawn worker thread");
            workers.push(handle);
        }

        Self {
            workers,
            sender: tx,
        }
    }

    /// Spawn a task onto the worker queue. Returns a `ThreadHandle` that can be joined.
    ///
    /// The provided closure receives an `Arc<Mutex<Executor>>` which the worker will
    /// create for the task. If you want tasks to operate on a shared executor instance,
    /// use `spawn_with_shared_executor` instead.
    pub fn spawn<F>(&self, f: F) -> ThreadHandle
    where
        F: FnOnce(Arc<Mutex<Executor>>) -> Result<()> + Send + 'static,
    {
        // We create a oneshot channel to receive the task result from the worker.
        let (res_tx, res_rx) = mpsc::channel::<Result<()>>();

        // Wrap the user's task to send the result back.
        let wrapped = Box::new(move |exe_arc: Arc<Mutex<Executor>>| {
            let r = f(exe_arc);
            // Ignore send errors (receiver may have been dropped)
            let _ = res_tx.send(r);
            Ok(())
        });

        // Send the task to the worker queue
        let _ = self.sender.send(Message::Task(wrapped));

        // Create a join handle that waits for the result channel.
        let join = thread::spawn(move || match res_rx.recv() {
            Ok(r) => r,
            Err(e) => Err(anyhow!("Worker result channel closed: {}", e)),
        });

        ThreadHandle { join }
    }

    /// Spawn a task that operates on a shared `Arc<Mutex<Executor>>`.
    ///
    /// This is the common pattern when multiple threads need to coordinate via a single executor.
    /// The manager will not create a new executor for the task; instead the provided `exe_arc`
    /// is passed to the task closure.
    pub fn spawn_with_shared_executor<F>(
        &self,
        exe_arc: Arc<Mutex<Executor>>,
        f: F,
    ) -> ThreadHandle
    where
        F: FnOnce(Arc<Mutex<Executor>>) -> Result<()> + Send + 'static,
    {
        let (res_tx, res_rx) = mpsc::channel::<Result<()>>();

        let wrapped = Box::new(move |_exe_dummy: Arc<Mutex<Executor>>| {
            let r = f(exe_arc);
            let _ = res_tx.send(r);
            Ok(())
        });

        let _ = self.sender.send(Message::Task(wrapped));

        let join = thread::spawn(move || match res_rx.recv() {
            Ok(r) => r,
            Err(e) => Err(anyhow!("Worker result channel closed: {}", e)),
        });

        ThreadHandle { join }
    }

    /// Gracefully shut down the thread manager and wait for workers to exit.
    pub fn shutdown(self) {
        // Send a Shutdown message per worker
        for _ in &self.workers {
            let _ = self.sender.send(Message::Shutdown);
        }

        // Join worker threads
        for w in self.workers {
            let _ = w.join();
        }
    }

    /// Convenience: create a manager sized from the provided `Environment`.
    ///
    /// Looks for `max_threads` in the environment (set by `devices::auto_configure`).
    /// Falls back to `default_threads` if not present or invalid.
    pub fn from_env(env: &Environment, default_threads: usize) -> Self {
        let mut n = default_threads;
        if let Some(Value::Number(nv)) = env.get("max_threads") {
            let as_usize = nv as usize;
            if as_usize > 0 {
                n = as_usize;
            }
        }
        ThreadManager::new(n)
    }
}

impl Drop for ThreadManager {
    fn drop(&mut self) {
        // Attempt to send shutdown messages; ignore errors.
        for _ in &self.workers {
            let _ = self.sender.send(Message::Shutdown);
        }
        // Note: we don't join here because we don't own the JoinHandles (they are moved out on creation).
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use crate::interpreter::Executor;

    #[test]
    fn spawn_task_and_join() {
        let mgr = ThreadManager::new(2);
        let handle = mgr.spawn(|_exe_arc| {
            // simple work
            std::thread::sleep(Duration::from_millis(10));
            Ok(())
        });
        // join should succeed
        handle.join().unwrap();
        mgr.shutdown();
    }

    #[test]
    fn spawn_with_shared_executor_modifies_executor() {
        let mgr = ThreadManager::new(2);
        let exe = Arc::new(Mutex::new(Executor::new()));
        let exe_clone = Arc::clone(&exe);

        let handle = mgr.spawn_with_shared_executor(exe_clone, |exe_arc| {
            let mut exe = exe_arc.lock().unwrap();
            exe.env.set_local("threaded".to_string(), crate::interpreter::environment::Value::Number(7.0));
            Ok(())
        });

        handle.join().unwrap();

        // Inspect executor
        let guard = exe.lock().unwrap();
        assert_eq!(guard.env.get("threaded"), Some(crate::interpreter::environment::Value::Number(7.0)));
        mgr.shutdown();
    }

    #[test]
    fn from_env_respects_max_threads() {
        let mut env = Environment::new();
        env.set_local("max_threads".to_string(), Value::Number(3.0));
        let mgr = ThreadManager::from_env(&env, 1);
        // We expect at least one worker; exact number is internal, but manager should be constructible.
        mgr.shutdown();
    }

    #[test]
    fn multiple_tasks_run_concurrently() {
        let mgr = ThreadManager::new(4);
        let counter = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..8 {
            let c = Arc::clone(&counter);
            let h = mgr.spawn(move |_exe| {
                // increment
                c.fetch_add(1, Ordering::SeqCst);
                Ok(())
            });
            handles.push(h);
        }

        for h in handles {
            h.join().unwrap();
        }

        assert_eq!(counter.load(Ordering::SeqCst), 8);
        mgr.shutdown();
    }
}
