// pipelines/pipe_api.rs
//! Channel and token primitives used by pipeline implementations.
//! This file defines the runtime channel abstraction and error/result types.

use std::sync::{Arc, Mutex, Condvar};
use std::collections::VecDeque;
use std::time::Duration;

#[derive(Debug)]
pub enum PipeError {
    Closed,
    SendError(String),
    RecvError(String),
    Terminated,
    Other(String),
}

pub type PipeResult<T> = Result<T, PipeError>;

/// A bounded channel for pipeline stages.
/// Simple mutex+condvar implementation for the bootstrap.
pub struct Channel<T> {
    inner: Arc<(Mutex<ChannelInner<T>>, Condvar)>,
}

struct ChannelInner<T> {
    buf: VecDeque<T>,
    cap: usize,
    closed: bool,
}

impl<T> Channel<T> {
    pub fn bounded(cap: usize) -> Self {
        let inner = ChannelInner {
            buf: VecDeque::with_capacity(cap),
            cap,
            closed: false,
        };
        Channel { inner: Arc::new((Mutex::new(inner), Condvar::new())) }
    }

    /// Blocking push (will wait until space available or closed).
    pub fn push(&self, item: T) -> PipeResult<()> {
        let (lock, cvar) = &*self.inner;
        let mut inner = lock.lock().unwrap();
        while inner.buf.len() >= inner.cap && !inner.closed {
            inner = cvar.wait(inner).unwrap();
        }
        if inner.closed {
            return Err(PipeError::Closed);
        }
        inner.buf.push_back(item);
        cvar.notify_all();
        Ok(())
    }

    /// Blocking pop (waits until item available or closed and empty).
    pub fn pop(&self) -> PipeResult<Option<T>> {
        let (lock, cvar) = &*self.inner;
        let mut inner = lock.lock().unwrap();
        while inner.buf.is_empty() && !inner.closed {
            inner = cvar.wait(inner).unwrap();
        }
        if inner.buf.is_empty() && inner.closed {
            return Ok(None);
        }
        let v = inner.buf.pop_front();
        cvar.notify_all();
        Ok(v)
    }

    pub fn close(&self) {
        let (lock, cvar) = &*self.inner;
        let mut inner = lock.lock().unwrap();
        inner.closed = true;
        cvar.notify_all();
    }
}

/// Convenience sender/receiver wrappers for type clarity.
#[derive(Clone)]
pub struct ChannelSender<T> {
    ch: Channel<T>,
}
#[derive(Clone)]
pub struct ChannelReceiver<T> {
    ch: Channel<T>,
}

impl<T> ChannelSender<T> {
    pub fn send(&self, v: T) -> PipeResult<()> { self.ch.push(v) }
    pub fn close(&self) { self.ch.close() }
}

impl<T> ChannelReceiver<T> {
    pub fn recv(&self) -> PipeResult<Option<T>> { self.ch.pop() }
}

pub fn channel<T>(cap: usize) -> (ChannelSender<T>, ChannelReceiver<T>) {
    let ch = Channel::bounded(cap);
    (ChannelSender { ch: ch.clone() }, ChannelReceiver { ch })
}
