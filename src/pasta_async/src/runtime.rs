use crate::api::{Continuation, Error, TaskHandle};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub struct Task {
    pub id: String,
    pub epoch: u64,
    pub cont: Box<dyn Continuation>,
}

pub struct Scheduler {
    queue: Arc<Mutex<VecDeque<Task>>>,
}

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            queue: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub fn enqueue(&self, t: Task) {
        let mut q = self.queue.lock().unwrap();
        q.push_back(t);
    }

    pub fn run(&self) -> Result<(), Error> {
        // Minimal single-threaded run loop stub
        loop {
            let task_opt = { self.queue.lock().unwrap().pop_front() };
            match task_opt {
                Some(task) => {
                    // In a real runtime, we'd poll the continuation here.
                    println!("Running task {}", task.id);
                    // For now, break to avoid infinite loop in template.
                    break;
                }
                None => break,
            }
        }
        Ok(())
    }
}
