// src/scheduler.rs
//! Simple scheduler for PASTA runtime
//!
//! Responsibilities:
//! - Maintain a set of long-lived "threads" (logical tasks) registered with the runtime.
//! - Use the `PriorityGraph` to compute ordering and numeric weights for threads.
//! - Provide a simple weighted round-robin scheduler that executes registered tasks.
//! - Integrate with the `Environment` so tasks can read/write runtime state.
//!
//! This scheduler is intentionally small and deterministic. It is not a full OS
//! scheduler — it's a cooperative, single-threaded scheduler suitable for driving
//! DO-block style tasks in the interpreter. Tasks are closures that receive a
//! mutable reference to the `Executor` and return `Ok(true)` when they are done.
//!
//! The scheduler uses the `PriorityGraph` to compute weights. If no priorities
//! are present, threads are scheduled fairly (equal weight). We convert floating
//! weights into integer "slices" by scaling, which yields a simple and stable
//! weighted round-robin behavior.

use std::collections::{HashMap, VecDeque};
use anyhow::{Result, anyhow};

use crate::interpreter::Executor;
use crate::interpreter::environment::{Environment, Value};
use crate::semantics::priority::PriorityGraph;

/// A task callback executed by the scheduler.
///
/// The callback receives a mutable reference to the `Executor`.
/// It should return:
/// - `Ok(true)` if the task has completed and should be removed.
/// - `Ok(false)` if the task remains active.
/// - `Err(_)` on error (scheduler will remove the task and propagate the error).
pub type TaskFn = Box<dyn FnMut(&mut Executor) -> Result<bool> + Send>;

/// A registered thread/task entry.
struct TaskEntry {
    /// Logical thread id (assigned into Environment when spawned).
    id: u64,
    /// Optional human name for debugging.
    name: Option<String>,
    /// The task closure.
    task: TaskFn,
}

/// Scheduler that manages tasks and executes them according to priorities.
pub struct Scheduler {
    /// Owned executor (contains environment, priorities, constraints, etc.)
    pub executor: Executor,
    /// Registered tasks keyed by thread id.
    tasks: HashMap<u64, TaskEntry>,
    /// Insertion order queue for fair fallback ordering.
    insertion_order: Vec<u64>,
    /// Priority graph used to compute ordering and weights.
    pub priorities: PriorityGraph,
    /// Scale factor used to convert floating weights into integer slices.
    weight_scale: f64,
}

impl Scheduler {
    /// Create a new scheduler with a fresh executor.
    pub fn new() -> Self {
        Self {
            executor: Executor::new(),
            tasks: HashMap::new(),
            insertion_order: Vec::new(),
            priorities: PriorityGraph::new(),
            weight_scale: 100.0, // default scale (tunable)
        }
    }

    /// Spawn a new logical thread/task.
    ///
    /// `name` is optional and will be stored in the environment metadata.
    /// Returns the assigned thread id.
    pub fn spawn<F>(&mut self, name: Option<String>, mut f: F) -> u64
    where
        F: FnMut(&mut Executor) -> Result<bool> + Send + 'static,
    {
        // Define a thread in the environment to get an id and metadata.
        // Default weight is 1.0; scheduler will override weights after computing priorities.
        let tid = self.executor.env.define_thread(name.clone(), 1.0);

        let entry = TaskEntry {
            id: tid,
            name: name.clone(),
            task: Box::new(f),
        };

        self.insertion_order.push(tid);
        self.tasks.insert(tid, entry);
        tid
    }

    /// Remove a task by id. Returns true if removed.
    pub fn remove(&mut self, id: u64) -> bool {
        if self.tasks.remove(&id).is_some() {
            // Remove from insertion_order
            self.insertion_order.retain(|&x| x != id);
            // Remove thread metadata from environment
            self.executor.env.remove_thread(id);
            true
        } else {
            false
        }
    }

    /// Compute scheduling order and integer slices for each active task using the priority graph.
    ///
    /// Returns a Vec of (thread_id, slices) in scheduling order (highest priority first).
    fn compute_slices(&self) -> Vec<(u64, usize)> {
        // Collect active thread names for mapping to priority graph nodes.
        // PriorityGraph uses String node names; we will map thread names -> ids.
        // If a thread has no name, we fall back to its numeric id string.
        let mut name_to_id: HashMap<String, u64> = HashMap::new();
        for (&id, entry) in &self.tasks {
            let key = entry
                .name
                .as_ref()
                .map(|s| s.clone())
                .unwrap_or_else(|| id.to_string());
            name_to_id.insert(key, id);
        }

        // Ask the priority graph for an order. If it fails or is empty, fall back to insertion order.
        let mut ordered_ids: Vec<u64> = Vec::new();
        if let Ok(order) = self.priorities.resolve_order() {
            // Map node names to ids when possible; otherwise ignore unknown nodes.
            for node in order {
                if let Some(&id) = name_to_id.get(&node) {
                    ordered_ids.push(id);
                }
            }
            // Append any remaining tasks not present in the priority graph in insertion order.
            for &id in &self.insertion_order {
                if !ordered_ids.contains(&id) && self.tasks.contains_key(&id) {
                    ordered_ids.push(id);
                }
            }
        } else {
            // Fallback: insertion order
            for &id in &self.insertion_order {
                if self.tasks.contains_key(&id) {
                    ordered_ids.push(id);
                }
            }
        }

        // Compute numeric weights. If the priority graph can compute weights by node name, use them.
        // Otherwise, default to 1.0 for each task.
        let mut weights: HashMap<u64, f64> = HashMap::new();
        if let Ok(wmap) = self.priorities.compute_weights() {
            // wmap: node name -> weight
            for (name, w) in wmap {
                if let Some(&id) = name_to_id.get(&name) {
                    weights.insert(id, w);
                }
            }
        }

        // For any task without a weight, assign 1.0
        for &id in &ordered_ids {
            weights.entry(id).or_insert(1.0);
        }

        // Convert weights to integer slices by scaling and rounding.
        // Ensure at least 1 slice per active task.
        let mut slices_vec: Vec<(u64, usize)> = Vec::new();
        for &id in &ordered_ids {
            let w = *weights.get(&id).unwrap_or(&1.0);
            let scaled = (w * self.weight_scale).round() as isize;
            let slices = if scaled <= 0 { 1usize } else { scaled as usize };
            slices_vec.push((id, slices));
        }

        slices_vec
    }

    /// Run the scheduler for up to `max_steps` task executions (each execution is one task "slice").
    ///
    /// Returns Ok(()) on normal completion. If a task returns an error, it is removed and the error is returned.
    pub fn run_rounds(&mut self, max_steps: usize) -> Result<()> {
        if self.tasks.is_empty() || max_steps == 0 {
            return Ok(());
        }

        // Compute slices and build a queue of task ids repeated by slices.
        let slices = self.compute_slices();
        let mut queue: VecDeque<u64> = VecDeque::new();
        for (id, s) in slices {
            for _ in 0..s {
                queue.push_back(id);
            }
        }

        // If queue is empty (shouldn't be), fall back to insertion order single pass.
        if queue.is_empty() {
            for &id in &self.insertion_order {
                queue.push_back(id);
            }
        }

        let mut steps = 0usize;
        while steps < max_steps && !queue.is_empty() && !self.tasks.is_empty() {
            // Pop next task id
            let id = queue.pop_front().unwrap();

            // If task no longer exists, skip
            if !self.tasks.contains_key(&id) {
                continue;
            }

            // Execute one slice of the task
            // We borrow the task mutably by temporarily removing and reinserting the entry.
            // This avoids double-borrow issues.
            let mut remove_after = false;
            {
                // Extract the task entry
                let mut entry = self.tasks.remove(&id).expect("task must exist");
                // Execute the task closure
                match (entry.task)(&mut self.executor) {
                    Ok(done) => {
                        if done {
                            // Task finished; mark for removal (do not reinsert)
                            remove_after = true;
                        } else {
                            // Task remains active; reinsert
                            self.tasks.insert(id, entry);
                        }
                    }
                    Err(e) => {
                        // On error, do not reinsert; return error after cleanup
                        // Ensure thread metadata removed from environment
                        self.executor.env.remove_thread(id);
                        // Reinsert remaining tasks back into tasks map is not necessary here
                        return Err(anyhow!("Task {} error: {}", id, e));
                    }
                }
            }

            // If task finished, ensure it's removed from insertion order and environment
            if remove_after {
                self.insertion_order.retain(|&x| x != id);
                self.executor.env.remove_thread(id);
            } else {
                // If still active, push id back to the end of the queue to allow other tasks to run
                queue.push_back(id);
            }

            steps += 1;
        }

        Ok(())
    }

    /// Convenience: run until all tasks complete or until `max_steps` reached.
    pub fn run_until_idle(&mut self, max_steps: usize) -> Result<()> {
        self.run_rounds(max_steps)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interpreter::environment::Value;

    #[test]
    fn spawn_and_run_simple_tasks() {
        let mut sched = Scheduler::new();

        // Task A increments "a" in environment
        let _a = sched.spawn(Some("A".into()), |exe| {
            let cur = exe.env.get("a").and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            }).unwrap_or(0.0);
            exe.env.assign("a", Value::Number(cur + 1.0));
            // never finish
            Ok(false)
        });

        // Task B increments "b"
        let _b = sched.spawn(Some("B".into()), |exe| {
            let cur = exe.env.get("b").and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            }).unwrap_or(0.0);
            exe.env.assign("b", Value::Number(cur + 1.0));
            Ok(false)
        });

        // Run 10 slices; with equal weights both should run roughly equally.
        sched.run_rounds(10).unwrap();

        let a = sched.executor.env.get("a").unwrap();
        let b = sched.executor.env.get("b").unwrap();

        // Both counters should be > 0
        assert!(matches!(a, Value::Number(_)));
        assert!(matches!(b, Value::Number(_)));
    }

    #[test]
    fn priority_affects_scheduling() {
        let mut sched = Scheduler::new();

        // Spawn two tasks named "High" and "Low"
        let high_id = sched.spawn(Some("High".into()), |exe| {
            let cur = exe.env.get("h").and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            }).unwrap_or(0.0);
            exe.env.assign("h", Value::Number(cur + 1.0));
            Ok(false)
        });

        let low_id = sched.spawn(Some("Low".into()), |exe| {
            let cur = exe.env.get("l").and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            }).unwrap_or(0.0);
            exe.env.assign("l", Value::Number(cur + 1.0));
            Ok(false)
        });

        // Add priority: High OVER Low
        sched.priorities.add_edge("High", "Low");

        // Run 40 slices. With the default decay rule (1.0, 0.75, ...), High should get more slices.
        sched.run_rounds(40).unwrap();

        let h = match sched.executor.env.get("h").unwrap() {
            Value::Number(n) => n,
            _ => 0.0,
        };
        let l = match sched.executor.env.get("l").unwrap() {
            Value::Number(n) => n,
            _ => 0.0,
        };

        // High should have run at least as many times as Low, and likely more.
        assert!(h >= l);
    }

    #[test]
    fn task_completion_removes_task() {
        let mut sched = Scheduler::new();

        // Spawn a task that completes after 3 increments
        let mut counter = 0usize;
        let tid = sched.spawn(Some("OneOff".into()), move |exe| {
            let cur = exe.env.get("x").and_then(|v| match v {
                Value::Number(n) => Some(n),
                _ => None,
            }).unwrap_or(0.0);
            exe
