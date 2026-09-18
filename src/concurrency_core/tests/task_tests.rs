#[cfg(test)]
mod tests {
    use crate::concurrency_core::task::{TaskHandle, TaskStatus};

    #[test]
    fn handle_basic() {
        let mut h = TaskHandle::new(42, 0);
        assert_eq!(h.id, 42);
        assert_eq!(h.epoch, 0);
        h.bump_epoch();
        assert_eq!(h.epoch, 1);
        assert_eq!(h.id_string(), "42");
    }

    #[test]
    fn status_terminal() {
        assert!(TaskStatus::Finished.is_terminal());
        assert!(TaskStatus::Errored("x".into()).is_terminal());
        assert!(!TaskStatus::Running.is_terminal());
    }
}
