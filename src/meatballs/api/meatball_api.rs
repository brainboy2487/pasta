// meatball_api.rs
// Auto-generated skeleton for the Meatball Runtime Abstraction (MRA).
// Fill in implementation details as you iterate.

pub mod types {
    /// Resource specification for a meatball.
    #[derive(Debug, Clone)]
    pub struct Resources {
        pub memory_mib: u64,
        pub vcpus: u8,
        pub disk_mib: u64,
        pub network: bool,
    }

    #[derive(Debug, Clone)]
    pub enum MeatballStatus {
        Starting,
        Running,
        Exited(i32),
        Crashed(String),
    }
}

use types::*;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use uuid::Uuid;

lazy_static::lazy_static! {
    static ref MEATBALL_REGISTRY: Mutex<HashMap<String, MeatballInfo>> = Mutex::new(HashMap::new());
}

pub struct MeatballInfo {
    pub id: String,
    pub resources: Resources,
    pub status: MeatballStatus,
    pub created_at: Instant,
    pub logs: Vec<String>,
    pub message_queue: Vec<Vec<u8>>,
}

pub trait Backend {
    fn spawn(&self, resources: Resources, rootfs: Option<&str>, flags: Option<&str>) -> Result<String, String>;
    fn exec(&self, id: &str, cmd: &str, args: &[&str]) -> Result<i32, String>;
    fn send(&self, id: &str, payload: &[u8]) -> Result<(), String>;
    fn recv(&self, id: &str) -> Result<Vec<u8>, String>;
    fn status(&self, id: &str) -> Result<MeatballStatus, String>;
    fn kill(&self, id: &str) -> Result<(), String>;
    fn logs(&self, id: &str, tail: Option<usize>) -> Result<String, String>;
}

/// A simple in-memory stub backend for early testing.
/// This simulates meatball lifecycle without actual containerization.
/// For production use, implement a DockerBackend or FirecrackerBackend.
pub struct LocalBackend;

impl Backend for LocalBackend {
    fn spawn(&self, resources: Resources, _rootfs: Option<&str>, _flags: Option<&str>) -> Result<String, String> {
        let id = Uuid::new_v4().to_string();
        let info = MeatballInfo {
            id: id.clone(),
            resources,
            status: MeatballStatus::Starting,
            created_at: Instant::now(),
            logs: vec![format!("[{}] Meatball spawned", chrono_lite())],
            message_queue: Vec::new(),
        };
        MEATBALL_REGISTRY.lock().unwrap().insert(id.clone(), info);
        Ok(id)
    }

    fn exec(&self, id: &str, cmd: &str, args: &[&str]) -> Result<i32, String> {
        // Simulated execution - in production, wire to container exec or agent protocol
        let mut reg = MEATBALL_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(id) {
            m.status = MeatballStatus::Running;
            m.logs.push(format!("[{}] exec: {} {:?}", chrono_lite(), cmd, args));
            
            // Simulate command execution based on cmd
            let exit_code = match cmd {
                "exit" | "quit" => {
                    let code = args.first()
                        .and_then(|s| s.parse::<i32>().ok())
                        .unwrap_or(0);
                    m.status = MeatballStatus::Exited(code);
                    code
                }
                "crash" => {
                    m.status = MeatballStatus::Crashed("Simulated crash".into());
                    1
                }
                "echo" => {
                    m.logs.push(format!("[{}] output: {}", chrono_lite(), args.join(" ")));
                    0
                }
                "sleep" => {
                    let ms = args.first()
                        .and_then(|s| s.parse::<u64>().ok())
                        .unwrap_or(1000);
                    std::thread::sleep(std::time::Duration::from_millis(ms));
                    0
                }
                _ => {
                    m.logs.push(format!("[{}] Unknown command: {}", chrono_lite(), cmd));
                    127 // Command not found
                }
            };
            Ok(exit_code)
        } else {
            Err(format!("meatball not found: {}", id))
        }
    }

    fn send(&self, id: &str, payload: &[u8]) -> Result<(), String> {
        let mut reg = MEATBALL_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(id) {
            m.message_queue.push(payload.to_vec());
            m.logs.push(format!("[{}] recv: {} bytes", chrono_lite(), payload.len()));
            Ok(())
        } else {
            Err(format!("meatball not found: {}", id))
        }
    }

    fn recv(&self, id: &str) -> Result<Vec<u8>, String> {
        let mut reg = MEATBALL_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(id) {
            let msg = m.message_queue.pop().unwrap_or_default();
            if !msg.is_empty() {
                m.logs.push(format!("[{}] sent: {} bytes", chrono_lite(), msg.len()));
            }
            Ok(msg)
        } else {
            Err(format!("meatball not found: {}", id))
        }
    }

    fn status(&self, id: &str) -> Result<MeatballStatus, String> {
        let reg = MEATBALL_REGISTRY.lock().unwrap();
        reg.get(id).map(|m| m.status.clone()).ok_or_else(|| format!("meatball not found: {}", id))
    }

    fn kill(&self, id: &str) -> Result<(), String> {
        let mut reg = MEATBALL_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(id) {
            m.status = MeatballStatus::Exited(-9); // SIGKILL
            m.logs.push(format!("[{}] killed", chrono_lite()));
        }
        reg.remove(id);
        Ok(())
    }

    fn logs(&self, id: &str, tail: Option<usize>) -> Result<String, String> {
        let reg = MEATBALL_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get(id) {
            let logs = match tail {
                Some(n) => m.logs.iter().rev().take(n).rev().cloned().collect::<Vec<_>>(),
                None => m.logs.clone(),
            };
            Ok(logs.join("\n"))
        } else {
            Err(format!("meatball not found: {}", id))
        }
    }
}

/// Simple timestamp helper (avoids chrono dependency)
fn chrono_lite() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    format!("{}.{:03}", dur.as_secs(), dur.subsec_millis())
}
