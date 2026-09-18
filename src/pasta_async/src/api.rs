use serde_json::Value;
use std::path::PathBuf;

pub type TaskId = String;
pub type WorkerId = String;

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct SnapshotMeta {
    pub task_id: TaskId,
    pub process_id: u64,
    pub name: String,
    pub time_ms: u64,
    pub offset_steps: u64,
    pub metadata: Vec<String>,
    pub epoch: u64,
    pub seq: u64,
    pub state: String,
    pub checksum: String,
    pub schema_version: String,
}

pub trait Continuation: Send + Sync {
    fn task_id(&self) -> &TaskId;
    fn epoch(&self) -> u64;
    fn serialize(&self) -> Value;
    fn deserialize(v: &Value) -> Box<dyn Continuation>
    where
        Self: Sized;
}

#[derive(Debug)]
pub struct TaskHandle {
    pub task_id: TaskId,
    pub epoch: u64,
}

#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("other: {0}")]
    Other(String),
}

pub trait AsyncRuntime: Send + Sync {
    fn spawn(&self, cont: Box<dyn Continuation>) -> Result<TaskHandle, Error>;
    fn suspend_in_memory(&self, cont: Box<dyn Continuation>) -> Result<(), Error>;
    fn suspend_to_disk(
        &self,
        cont: Box<dyn Continuation>,
        meta: SnapshotMeta,
    ) -> Result<PathBuf, Error>;
    fn resume_from_disk(&self, path: &std::path::Path) -> Result<TaskHandle, Error>;
    fn cancel(&self, task_id: &TaskId) -> Result<(), Error>;
    fn run(&self) -> Result<(), Error>;
}
