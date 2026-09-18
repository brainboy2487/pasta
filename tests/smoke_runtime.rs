#[cfg(test)]
mod tests {
    use crate::concurrency_core::bridge::Bridge;

    #[test]
    fn smoke_spawn_status() {
        let mut b = Bridge::new();
        let h = b.spawn("test-task");
        let s = b.status(&h);
        assert!(s.contains("Running") || s.contains("Finished") || s.contains("Errored"));
    }
}
