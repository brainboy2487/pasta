// meatball_api.rs
// Auto-generated skeleton for the Meatball Runtime Abstraction (MRA).
// Fill in implementation details as you iterate.

pub mod types {{
    /// Resource specification for a meatball.
    #[derive(Debug, Clone)]
    pub struct Resources {{
        pub memory_mib: u64,
        pub vcpus: u8,
        pub disk_mib: u64,
        pub network: bool,
    }}

    #[derive(Debug)]
    pub enum MeatballStatus {{
        Starting,
        Running,
        Exited(i32),
        Crashed(String),
    }}
}}

use types::*;
use std::collections::HashMap;
use std::sync::Mutex;
use uuid::Uuid;

lazy_static::lazy_static! {{
    static ref MEATBALL_REGISTRY: Mutex<HashMap<String, MeatballInfo>> = Mutex::new(HashMap::new());
}}

pub struct MeatballInfo {{
    pub id: String,
    pub resources: Resources,
    pub status: MeatballStatus,
}}

pub trait Backend {{
    fn spawn(&self, resources: Resources, rootfs: Option<&str>, flags: Option<&str>) -> Result<String, String>;
    fn exec(&self, id: &str, cmd: &str, args: &[&str]) -> Result<i32, String>;
    fn send(&self, id: &str, payload: &[u8]) -> Result<(), String>;
    fn recv(&self, id: &str) -> Result<Vec<u8>, String>;
    fn status(&self, id: &str) -> Result<MeatballStatus, String>;
    fn kill(&self, id: &str) -> Result<(), String>;
    fn logs(&self, id: &str, tail: Option<usize>) -> Result<String, String>;
}}

// A simple in-memory stub backend for early testing.
pub struct LocalBackend;

impl Backend for LocalBackend {{
    fn spawn(&self, resources: Resources, _rootfs: Option<&str>, _flags: Option<&str>) -> Result<String, String> {{
        let id = Uuid::new_v4().to_string();
        let info = MeatballInfo {{
            id: id.clone(),
            resources,
            status: MeatballStatus::Starting,
        }};
        MEATBALL_REGISTRY.lock().unwrap().insert(id.clone(), info);
        Ok(id)
    }}

    fn exec(&self, id: &str, _cmd: &str, _args: &[&str]) -> Result<i32, String> {{
        // TODO: wire to agent protocol
        let mut reg = MEATBALL_REGISTRY.lock().unwrap();
        if let Some(m) = reg.get_mut(id) {{
            m.status = MeatballStatus::Running;
            Ok(0)
        }} else {{
            Err(format!("meatball not found: {}", id))
        }}
    }}

    fn send(&self, _id: &str, _payload: &[u8]) -> Result<(), String> {{ Ok(()) }}
    fn recv(&self, _id: &str) -> Result<Vec<u8>, String> {{ Ok(vec![]) }}
    fn status(&self, id: &str) -> Result<MeatballStatus, String> {{
        let reg = MEATBALL_REGISTRY.lock().unwrap();
        reg.get(id).map(|m| m.status.clone()).ok_or_else(|| "not found".to_string())
    }}
    fn kill(&self, id: &str) -> Result<(), String> {{
        let mut reg = MEATBALL_REGISTRY.lock().unwrap();
        reg.remove(id);
        Ok(())
    }}
    fn logs(&self, _id: &str, _tail: Option<usize>) -> Result<String, String> {{
        Ok("<no logs yet>".to_string())
    }}
}}
