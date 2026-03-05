// meatballs/runtime/runtime.rs
// Lightweight runtime skeleton for Meatball lifecycle, scheduling, and observability.
// Drop this file into meatballs/runtime/runtime.rs and wire it into your crate root.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Condvar};
use std::thread;
use std::time::{Duration, Instant};

/// Basic meatball status enum used by the runtime manager.
#[derive(Debug, Clone)]
pub enum RuntimeStatus {
    Starting,
    Running,
    Exited(i32),
    Crashed(String),
    Unknown,
}

/// Minimal resource descriptor used by the runtime manager.
/// Keep this small; map to backend resource schema when wiring real backends.
#[derive(Debug, Clone)]
pub struct Resources {
    pub memory_mib: u64,
    pub vcpus: u8,
    pub disk_mib: u64,
    pub network: bool,
}

/// Metadata stored for each meatball.
#[derive(Debug)]
pub struct MeatballMeta {
    pub id: String,
    pub resources: Resources,
    pub status: RuntimeStatus,
    pub last_heartbeat: Instant,
    pub created_at: Instant,
}

/// Trait for backend implementations. When you wire the real backend,
/// implement this trait in your backend module and register it with the manager.
pub trait Backend: Send + Sync + 'static {
    fn spawn(&self, resources: Resources, rootfs: Option<&str>, flags: Option<&str>) -> Result<String, String>;
    fn exec(&self, id: &str, cmd: &str, args: &[&str]) -> Result<i32, String>;
    fn kill(&self, id: &str) -> Result<(), String>;
    fn status(&self, id: &str) -> Result<RuntimeStatus, String>;
    fn logs(&self, id: &str, tail: Option<usize>) -> Result<String, String>;
}

/// Core manager that tracks meatballs and delegates to a backend.
pub struct MeatballManager {
    backend: Arc<dyn Backend>,
    registry: Arc<Mutex<HashMap<String, MeatballMeta>>>,
    // Simple task queue for async operations (spawn/kill/exec can be queued)
    task_queue: Arc<(Mutex<Vec<RuntimeTask>>, Condvar)>,
    // Monitor thread handle (kept for graceful shutdown)
    monitor_handle: Option<thread::JoinHandle<()>>,
    // Flag to request shutdown
    shutdown_flag: Arc<Mutex<bool>>,
}

/// Simple runtime tasks for the queue.
enum RuntimeTask {
    Spawn { resources: Resources, rootfs: Option<String>, flags: Option<String> },
    Exec { id: String, cmd: String, args: Vec<String> },
    Kill { id: String },
}

impl MeatballManager {
    /// Create a new manager with the provided backend.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        let registry = Arc::new(Mutex::new(HashMap::new()));
        let task_queue = Arc::new((Mutex::new(Vec::new()), Condvar::new()));
        let shutdown_flag = Arc::new(Mutex::new(false));

        // Start monitor thread
        let registry_clone = Arc::clone(&registry);
        let shutdown_clone = Arc::clone(&shutdown_flag);
        let monitor_handle = {
            let tq = Arc::clone(&task_queue);
            thread::spawn(move || {
                Self::monitor_loop(registry_clone, tq, shutdown_clone);
            })
        };

        MeatballManager {
            backend,
            registry,
            task_queue,
            monitor_handle: Some(monitor_handle),
            shutdown_flag,
        }
    }

    /// Enqueue a spawn task (non-blocking). Returns an operation id (meatball id on success).
    pub fn spawn_meatball(&self, resources: Resources, rootfs: Option<&str>, flags: Option<&str>) -> Result<String, String> {
        // For ergonomics, attempt synchronous spawn via backend first; fallback to queued spawn on transient errors.
        match self.backend.spawn(resources.clone(), rootfs, flags) {
            Ok(id) => {
                let meta = MeatballMeta {
                    id: id.clone(),
                    resources,
                    status: RuntimeStatus::Starting,
                    last_heartbeat: Instant::now(),
                    created_at: Instant::now(),
                };
                self.registry.lock().unwrap().insert(id.clone(), meta);
                Ok(id)
            }
            Err(e) => Err(e),
        }
    }

    /// Execute a command inside a meatball (synchronous wrapper).
    pub fn exec_in_meatball(&self, id: &str, cmd: &str, args: &[&str]) -> Result<i32, String> {
        self.backend.exec(id, cmd, args)
    }

    /// Kill a meatball and remove it from registry.
    pub fn kill_meatball(&self, id: &str) -> Result<(), String> {
        self.backend.kill(id)?;
        self.registry.lock().unwrap().remove(id);
        Ok(())
    }

    /// Query status from the backend and update registry.
    pub fn status(&self, id: &str) -> Result<RuntimeStatus, String> {
        let s = self.backend.status(id)?;
        if let Ok(mut reg) = self.registry.lock() {
            if let Some(meta) = reg.get_mut(id) {
                meta.status = s.clone();
                meta.last_heartbeat = Instant::now();
            }
        }
        Ok(s)
    }

    /// Collect logs via backend.
    pub fn logs(&self, id: &str, tail: Option<usize>) -> Result<String, String> {
        self.backend.logs(id, tail)
    }

    /// Return a snapshot of metrics for all meatballs.
    pub fn collect_metrics(&self) -> Vec<(String, RuntimeStatus, u64)> {
        let reg = self.registry.lock().unwrap();
        reg.iter()
            .map(|(id, meta)| {
                // Example metric: uptime seconds
                let uptime = meta.created_at.elapsed().as_secs();
                (id.clone(), meta.status.clone(), uptime)
            })
            .collect()
    }

    /// Graceful shutdown: signal monitor thread and wait for it to exit.
    pub fn shutdown(&mut self) {
        {
            let mut flag = self.shutdown_flag.lock().unwrap();
            *flag = true;
        }
        // Wake monitor if waiting
        let (lock, cvar) = &*self.task_queue;
        cvar.notify_all();
        if let Some(handle) = self.monitor_handle.take() {
            let _ = handle.join();
        }
        // Attempt to kill remaining meatballs (best-effort)
        let ids: Vec<String> = self.registry.lock().unwrap().keys().cloned().collect();
        for id in ids {
            let _ = self.backend.kill(&id);
        }
    }

    /// Internal monitor loop: checks heartbeats, processes queued tasks, and performs periodic reconciliation.
    fn monitor_loop(registry: Arc<Mutex<HashMap<String, MeatballMeta>>>, task_queue: Arc<(Mutex<Vec<RuntimeTask>>, Condvar)>, shutdown_flag: Arc<Mutex<bool>>) {
        let heartbeat_interval = Duration::from_secs(5);
        let heartbeat_timeout = Duration::from_secs(15);

        loop {
            // Check shutdown
            if *shutdown_flag.lock().unwrap() {
                eprintln!("runtime monitor: shutdown requested");
                break;
            }

            // Process queued tasks (if any)
            {
                let (lock, cvar) = &*task_queue;
                let mut q = lock.lock().unwrap();
                while q.is_empty() {
                    // Wait with timeout so we still run periodic checks
                    let (guard, _res) = cvar.wait_timeout(q, heartbeat_interval).unwrap();
                    q = guard;
                    if *shutdown_flag.lock().unwrap() {
                        break;
                    }
                    // break to run heartbeat checks
                    break;
                }
                // Drain tasks
                let tasks: Vec<RuntimeTask> = q.drain(..).collect();
                drop(q);
                for t in tasks {
                    match t {
                        RuntimeTask::Spawn { resources, rootfs, flags } => {
                            // In this skeleton we don't have access to backend here; real implementation would call backend.
                            eprintln!("monitor: spawn task requested (resources={:?})", resources);
                            // TODO: call backend.spawn and update registry
                        }
                        RuntimeTask::Exec { id, cmd, args } => {
                            eprintln!("monitor: exec task for {}: {} {:?}", id, cmd, args);
                            // TODO: call backend.exec
                        }
                        RuntimeTask::Kill { id } => {
                            eprintln!("monitor: kill task for {}", id);
                            // TODO: call backend.kill and cleanup registry
                        }
                    }
                }
            }

            // Heartbeat / health checks
            {
                let mut reg = registry.lock().unwrap();
                let now = Instant::now();
                let mut to_remove = Vec::new();
                for (id, meta) in reg.iter_mut() {
                    let elapsed = now.duration_since(meta.last_heartbeat);
                    if elapsed > heartbeat_timeout {
                        eprintln!("runtime monitor: meatball {} missed heartbeat ({}s)", id, elapsed.as_secs());
                        // Mark as crashed for now; real implementation should query backend for status
                        meta.status = RuntimeStatus::Crashed("heartbeat timeout".into());
                        // Optionally schedule cleanup
                        to_remove.push(id.clone());
                    }
                }
                // Best-effort cleanup of timed-out meatballs from registry (do not call backend here)
                for id in to_remove {
                    eprintln!("runtime monitor: removing stale meatball {}", id);
                    reg.remove(&id);
                }
            }

            // Sleep until next iteration
            thread::sleep(heartbeat_interval);
        }

        eprintln!("runtime monitor: exiting");
    }
}

/// Convenience init function for the runtime. Returns a manager instance.
/// Caller should keep the manager alive for the runtime lifetime and call shutdown() on drop.
pub fn init_runtime_with_backend(backend: Arc<dyn Backend>) -> MeatballManager {
    let manager = MeatballManager::new(backend);
    eprintln!("runtime: initialized");
    manager
}

/// Minimal shutdown helper for convenience.
pub fn shutdown_runtime(manager: &mut MeatballManager) {
    manager.shutdown();
    eprintln!("runtime: shutdown complete");
}

/// --- Unit tests using a MockBackend so you can validate runtime behavior locally ---
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// A tiny mock backend that simulates spawn/exec/kill behavior for tests.
    struct MockBackend {
        counter: AtomicUsize,
    }

    impl MockBackend {
        fn new() -> Self {
            MockBackend { counter: AtomicUsize::new(1) }
        }
    }

    impl Backend for MockBackend {
        fn spawn(&self, _resources: Resources, _rootfs: Option<&str>, _flags: Option<&str>) -> Result<String, String> {
            let id = format!("mock-{}", self.counter.fetch_add(1, Ordering::SeqCst));
            Ok(id)
        }
        fn exec(&self, _id: &str, _cmd: &str, _args: &[&str]) -> Result<i32, String> {
            Ok(0)
        }
        fn kill(&self, _id: &str) -> Result<(), String> {
            Ok(())
        }
        fn status(&self, _id: &str) -> Result<RuntimeStatus, String> {
            Ok(RuntimeStatus::Running)
        }
        fn logs(&self, _id: &str, _tail: Option<usize>) -> Result<String, String> {
            Ok("mock logs".into())
        }
    }

    #[test]
    fn test_spawn_and_status() {
        let backend = Arc::new(MockBackend::new());
        let manager = init_runtime_with_backend(backend);
        let res = Resources { memory_mib: 64, vcpus: 1, disk_mib: 16, network: false };
        let id = manager.spawn_meatball(res.clone(), None, None).expect("spawn failed");
        // status should be available via backend; manager.status will call backend.status
        let s = manager.status(&id).expect("status failed");
        match s {
            RuntimeStatus::Running => {}
            _ => panic!("expected Running"),
        }
    }

    #[test]
    fn test_exec_and_logs() {
        let backend = Arc::new(MockBackend::new());
        let manager = init_runtime_with_backend(backend);
        let res = Resources { memory_mib: 64, vcpus: 1, disk_mib: 16, network: false };
        let id = manager.spawn_meatball(res.clone(), None, None).expect("spawn failed");
        let rc = manager.exec_in_meatball(&id, "/bin/true", &[]).expect("exec failed");
        assert_eq!(rc, 0);
        let logs = manager.logs(&id, Some(10)).expect("logs failed");
        assert!(logs.contains("mock"));
    }
}
