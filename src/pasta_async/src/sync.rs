use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

// Minimal async channel stub for iteration
pub struct Channel<T> {
    inner: Arc<Mutex<VecDeque<T>>>,
}

impl<T> Channel<T> {
    pub fn bounded(_cap: usize) -> Self {
        Channel {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }
    pub fn send(&self, v: T) {
        self.inner.lock().unwrap().push_back(v);
    }
    pub fn try_recv(&self) -> Option<T> {
        self.inner.lock().unwrap().pop_front()
    }
}
