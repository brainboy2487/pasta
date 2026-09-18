//! Bridge adapter between `pasta_async` traits and the main PASTA thread registry.
//!
//! This keeps async orchestration and interpreter threading on one backend.

use std::path::{Path, PathBuf};

use anyhow::anyhow;
use pasta_async::api::{AsyncRuntime, Continuation, Error, SnapshotMeta, TaskHandle, TaskId};
use pasta_async::{io, serialize};
use serde_json::{json, Value as JsonValue};

use crate::interpreter::environment::Value;
use crate::threading::thread_api;
use crate::threading::threads::list_threads;

/// Thread-registry-backed implementation of `pasta_async::api::AsyncRuntime`.
#[derive(Debug, Clone)]
pub struct RegistryAsyncRuntime {
    /// Root directory for async snapshot files.
    pub snapshot_root: PathBuf,
}

impl Default for RegistryAsyncRuntime {
    fn default() -> Self {
        Self {
            snapshot_root: PathBuf::from("snapshots"),
        }
    }
}

impl RegistryAsyncRuntime {
    /// Construct with a custom snapshot root directory.
    pub fn new(snapshot_root: PathBuf) -> Self {
        Self { snapshot_root }
    }

    fn target_from_task_id(task_id: &str) -> Value {
        if let Ok(id) = task_id.parse::<u64>() {
            Value::Number(id as f64)
        } else {
            Value::String(task_id.to_string())
        }
    }
}

impl AsyncRuntime for RegistryAsyncRuntime {
    fn spawn(&self, cont: Box<dyn Continuation>) -> Result<TaskHandle, Error> {
        let task_id = cont.task_id().clone();
        let id = thread_api::spawn_statement_thread(
            task_id.clone(),
            vec![],
            std::collections::HashMap::new(),
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        Ok(TaskHandle {
            task_id: id.to_string(),
            epoch: cont.epoch(),
        })
    }

    fn suspend_in_memory(&self, _cont: Box<dyn Continuation>) -> Result<(), Error> {
        // Current integration keeps running tasks in thread registry.
        Ok(())
    }

    fn suspend_to_disk(
        &self,
        cont: Box<dyn Continuation>,
        meta: SnapshotMeta,
    ) -> Result<PathBuf, Error> {
        let cont_json = cont.serialize();
        let payload = json!({
            "meta": meta,
            "continuation": cont_json,
        });
        let bytes = serde_json::to_vec_pretty(&payload)?;
        let checksum = serialize::compute_checksum(&bytes);
        let with_checksum = json!({
            "checksum": checksum,
            "payload": payload,
        });
        let final_bytes = serde_json::to_vec_pretty(&with_checksum)?;
        let tmp = self.snapshot_root.join("tmp");
        let out = self.snapshot_root.join("outbox");
        let path = io::write_snapshot_atomic(
            &tmp,
            &out,
            &final_bytes,
            &meta.task_id,
            meta.epoch,
            meta.seq,
        )?;
        Ok(path)
    }

    fn resume_from_disk(&self, path: &Path) -> Result<TaskHandle, Error> {
        let raw = std::fs::read(path)?;
        let doc: JsonValue = serde_json::from_slice(&raw)?;
        let payload = doc
            .get("payload")
            .ok_or_else(|| Error::Other("snapshot missing payload".to_string()))?;
        let meta = payload
            .get("meta")
            .ok_or_else(|| Error::Other("snapshot missing meta".to_string()))?;
        let task_id = meta
            .get("task_id")
            .and_then(|v| v.as_str())
            .unwrap_or("resumed-task")
            .to_string();
        let epoch = meta
            .get("epoch")
            .and_then(|v| v.as_u64())
            .unwrap_or(1);
        let id = thread_api::spawn_statement_thread(
            task_id.clone(),
            vec![],
            std::collections::HashMap::new(),
        )
        .map_err(|e| Error::Other(e.to_string()))?;
        Ok(TaskHandle {
            task_id: id.to_string(),
            epoch,
        })
    }

    fn cancel(&self, task_id: &TaskId) -> Result<(), Error> {
        let target = Self::target_from_task_id(task_id);
        let id = thread_api::resolve_thread_id(&target)
            .map_err(|e| Error::Other(e.to_string()))?;
        if thread_api::kill_thread_by_id(id) {
            Ok(())
        } else {
            Err(Error::Other(format!("task '{}' not found", task_id)))
        }
    }

    fn run(&self) -> Result<(), Error> {
        Ok(())
    }
}

/// Persist lightweight thread snapshot metadata for an active thread target.
pub fn persist_thread_snapshot(target: &Value) -> anyhow::Result<PathBuf> {
    let id = thread_api::resolve_thread_id(target)?;
    let row = list_threads()
        .into_iter()
        .find(|t| t.id == id)
        .ok_or_else(|| anyhow!("thread target not found"))?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let meta = SnapshotMeta {
        task_id: row.name.clone(),
        process_id: row.id,
        name: row.name.clone(),
        time_ms: now,
        offset_steps: 0,
        metadata: vec!["source=thread-registry".to_string()],
        epoch: 1,
        seq: now,
        state: row.status.to_string(),
        checksum: String::new(),
        schema_version: "phase-e-bridge-v1".to_string(),
    };

    struct EmptyContinuation {
        id: TaskId,
    }
    impl Continuation for EmptyContinuation {
        fn task_id(&self) -> &TaskId { &self.id }
        fn epoch(&self) -> u64 { 1 }
        fn serialize(&self) -> JsonValue {
            json!({"task_id": self.id, "kind": "empty"})
        }
        fn deserialize(v: &JsonValue) -> Box<dyn Continuation> {
            let id = v
                .get("task_id")
                .and_then(|x| x.as_str())
                .unwrap_or("unknown")
                .to_string();
            Box::new(EmptyContinuation { id })
        }
    }

    let rt = RegistryAsyncRuntime::default();
    rt.suspend_to_disk(
        Box::new(EmptyContinuation { id: row.name }),
        meta,
    )
    .map_err(|e| anyhow!(e.to_string()))
}
