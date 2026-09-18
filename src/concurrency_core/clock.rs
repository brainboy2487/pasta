// src/concurrency_core/clock.rs
//! Deterministic logical clock for concurrency_core.
//! This clock is NOT wall clock time. It advances only at deterministic points.
//! The implementation is intentionally small and serializable for snapshots.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Logical monotonic clock for deterministic scheduling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LogicalClock(pub u64);

impl LogicalClock {
    /// Create a zero clock.
    pub fn zero() -> Self {
        LogicalClock(0)
    }

    /// Advance the clock by one tick.
    pub fn tick(&mut self) {
        self.0 = self.0.wrapping_add(1);
    }

    /// Advance the clock by n ticks.
    pub fn advance_by(&mut self, n: u64) {
        self.0 = self.0.wrapping_add(n);
    }

    /// Set the clock to an explicit value.
    pub fn set(&mut self, v: u64) {
        self.0 = v;
    }

    /// Read the current clock value.
    pub fn get(&self) -> u64 {
        self.0
    }

    /// Compare two clocks.
    pub fn cmp(&self, other: &LogicalClock) -> Ordering {
        self.0.cmp(&other.0)
    }

    /// Merge semantics for spawn.
    /// When a parent spawns a child, initialize the child clock to the parent's clock.
    /// This keeps child logical time synchronized with parent at creation.
    pub fn init_child_from_parent(parent: &LogicalClock) -> LogicalClock {
        LogicalClock(parent.0)
    }

    /// Merge semantics for resume from snapshot.
    /// If the resumed clock is behind the current registry clock for that task id,
    /// choose the max to avoid moving time backwards.
    pub fn merge_resume(current: &LogicalClock, resumed: &LogicalClock) -> LogicalClock {
        LogicalClock(std::cmp::max(current.0, resumed.0))
    }

    /// Safe increment that returns the previous value then ticks.
    /// Useful for generating deterministic sequence numbers tied to logical time.
    pub fn fetch_and_tick(&mut self) -> u64 {
        let prev = self.0;
        self.tick();
        prev
    }
}

impl Default for LogicalClock {
    fn default() -> Self {
        LogicalClock::zero()
    }
}
