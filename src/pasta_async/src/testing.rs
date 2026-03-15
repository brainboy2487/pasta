#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn smoke() {
        // Basic smoke test to ensure crate compiles and IO helpers work
        let tmp = Path::new("snapshots/tmp");
        let out = Path::new("snapshots/outbox");
        let data = br#"{"task_id":"task-0001"}"#;
        let p = crate::io::write_snapshot_atomic(tmp, out, data, "task-0001", 1, 1).expect("write");
        assert!(p.exists());
    }
}
