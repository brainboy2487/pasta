// src/concurrency_core/tests/clock_tests.rs
#[cfg(test)]
mod tests {
    use crate::concurrency_core::clock::LogicalClock;

    #[test]
    fn zero_and_tick() {
        let mut c = LogicalClock::zero();
        assert_eq!(c.get(), 0);
        c.tick();
        assert_eq!(c.get(), 1);
        c.advance_by(4);
        assert_eq!(c.get(), 5);
    }

    #[test]
    fn init_child_from_parent() {
        let mut parent = LogicalClock::zero();
        parent.advance_by(10);
        let child = LogicalClock::init_child_from_parent(&parent);
        assert_eq!(child.get(), parent.get());
    }

    #[test]
    fn merge_resume_chooses_max() {
        let current = LogicalClock(20);
        let resumed = LogicalClock(15);
        let merged = LogicalClock::merge_resume(&current, &resumed);
        assert_eq!(merged.get(), 20);

        let current2 = LogicalClock(5);
        let resumed2 = LogicalClock(12);
        let merged2 = LogicalClock::merge_resume(&current2, &resumed2);
        assert_eq!(merged2.get(), 12);
    }

    #[test]
    fn fetch_and_tick_returns_previous() {
        let mut c = LogicalClock::zero();
        let prev = c.fetch_and_tick();
        assert_eq!(prev, 0);
        assert_eq!(c.get(), 1);
    }
}
