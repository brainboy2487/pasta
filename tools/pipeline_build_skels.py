#!/usr/bin/env python3
"""
Bootstrapper for src/pipelines/ module.

Usage:
    python3 scripts/bootstrap_pipelines.py

Behavior:
- Creates src/pipelines/ if missing.
- Writes mod.rs, pipes.rs, pipe_api.rs, single_p.rs, double_p.rs, async_p.rs, shared_p.rs.
- If a target file already exists, writes a .new file instead of overwriting.
- Prints a summary at the end.
"""

import os
import sys
from pathlib import Path
from textwrap import dedent

ROOT = Path(__file__).resolve().parents[1]  # repo root (assumes scripts/ at repo root)
TARGET_DIR = ROOT / "src" / "pipelines"

FILES = {
    "mod.rs": r'''
// pipelines/mod.rs
//! Public entry for the pipelines module.
//! Re-export the public API and implementations.

pub mod pipes;
pub mod pipe_api;
pub mod single_p;
pub mod double_p;
pub mod async_p;
pub mod shared_p;

pub use pipes::Pipeline;
pub use pipe_api::{Channel, ChannelSender, ChannelReceiver, PipeError, PipeResult};
''',

    "pipe_api.rs": r'''
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
''',

    "pipes.rs": r'''
// pipelines/pipes.rs
//! High-level pipeline orchestration and public API.
//! This file exposes a Pipeline type and helpers that wire stages together.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};
use std::sync::Arc;

/// Public pipeline handle. For the bootstrap this is a thin wrapper.
pub struct Pipeline {
    // TODO: store AST or compiled stages
    pub name: String,
}

impl Pipeline {
    pub fn new(name: &str) -> Self {
        Pipeline { name: name.to_string() }
    }

    /// Example helper to create a simple single-threaded pipeline.
    /// Real implementation will accept stage descriptors and operator tokens.
    pub fn run_single_threaded<F, T>(&self, producer: F) -> PipeResult<()>
    where
        F: Fn(ChannelSender<T>) -> PipeResult<()> {
        // Example: create a channel and call producer with sender.
        let (s, r) = channel::<T>(64);
        // In a real pipeline we'd spawn consumer(s) and wire stages.
        producer(s)?;
        // close and drain
        s.close();
        let _ = r.recv()?;
        Ok(())
    }
}
''',

    "single_p.rs": r'''
// pipelines/single_p.rs
//! Single-threaded streaming pipeline implementation for `|`.
//! Cooperative execution: producer yields values, consumer pulls them immediately.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};
use std::thread;
use std::time::Duration;

/// Example single-threaded pipeline runner.
/// In the real interpreter this will be integrated with the VM's cooperative scheduler.
pub fn run_pipe_single_threaded<T, P, C>(producer: P, consumer: C) -> PipeResult<()>
where
    P: Fn(ChannelSender<T>) -> PipeResult<()>,
    C: Fn(ChannelReceiver<T>) -> PipeResult<()>,
{
    let (s, r) = channel::<T>(1); // capacity 1 for direct handoff
    // Run producer and consumer sequentially but interleave via small sleeps to simulate yielding.
    // In the interpreter, replace sleeps with VM yields.
    producer(s.clone())?;
    // After producer finishes, close sender so consumer sees EOF.
    s.close();
    consumer(r)?;
    Ok(())
}

// TODO: Replace the above with an interpreter-integrated cooperative loop that alternates
// between producer and consumer, avoiding OS threads and sleeps.
''',

    "double_p.rs": r'''
// pipelines/double_p.rs
//! Double-pipe (`||`) implementation: isolated interpreter contexts.
//! Each side runs in its own VM context with separate globals/heaps.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};
use std::sync::Arc;
use std::thread;

/// Spawn an isolated context for the right-hand stage and wire a channel between them.
/// For the bootstrap we simulate isolation by running closures on separate OS threads.
/// Replace with lightweight VM context spawn in the real runtime.
pub fn run_double_pipe<T, L, R>(left: L, right: R) -> PipeResult<()>
where
    L: Fn(ChannelSender<T>) -> PipeResult<()> + Send + 'static,
    R: Fn(ChannelReceiver<T>) -> PipeResult<()> + Send + 'static,
    T: Send + 'static,
{
    let (s, r) = channel::<T>(64);

    // Spawn right-hand isolated context
    let right_handle = thread::spawn(move || {
        // In real implementation, create a new VM context and execute 'right' inside it.
        right(r).map_err(|e| e)
    });

    // Run left in current thread (or spawn as needed)
    let left_res = left(s.clone());

    // Close sender and wait for right
    s.close();
    let right_res = right_handle.join().map_err(|_| PipeError::Other("thread join failed".into()))?;

    left_res?;
    right_res?;
    Ok(())
}
''',

    "async_p.rs": r'''
// pipelines/async_p.rs
//! Async-pipe (`|&|`) implementation: schedule stages on pasta_async worker pool.
//! This file contains the integration points; the actual worker pool lives in pasta_async.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};
use std::sync::Arc;

/// Placeholder async integration API.
/// The real implementation should call into pasta_async to spawn tasks and return handles.
pub fn run_async_pipe<T, L, R>(_left: L, _right: R, _workers: usize) -> PipeResult<()>
where
    L: Fn(ChannelSender<T>) -> PipeResult<()> + Send + 'static,
    R: Fn(ChannelReceiver<T>) -> PipeResult<()> + Send + 'static,
    T: Send + 'static,
{
    // TODO:
    // 1. Create bounded channels.
    // 2. Submit left and right StageFns to pasta_async worker pool.
    // 3. Support preserve/unordered merging semantics.
    // 4. Ensure thread-safe runtime primitives for shared resources.
    Err(PipeError::Other("async_pipe not yet implemented".into()))
}
''',

    "shared_p.rs": r'''
// pipelines/shared_p.rs
//! Shared-pipe (`|:|`) implementation: bind two scripts/modules into a single shared thread.
//! Useful when two modules must share interpreter state but still communicate via channels.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};

/// For the bootstrap, this function demonstrates wiring two entry points into a single context.
/// In the real runtime, load both modules into the same VM context and wire their entry points.
pub fn run_shared_pipe<T, A, B>(_left_entry: A, _right_entry: B) -> PipeResult<()>
where
    A: Fn(ChannelSender<T>) -> PipeResult<()>,
    B: Fn(ChannelReceiver<T>) -> PipeResult<()>,
{
    // TODO: implement module loading into a single interpreter context and channel wiring.
    Err(PipeError::Other("shared_pipe not yet implemented".into()))
}
''',

    "README.md": r'''
# pipelines

This module implements the new pipeline subsystem for Pasta.

Files:
- `pipe_api.rs` — channel primitives and error types.
- `pipes.rs` — high-level Pipeline API and helpers.
- `single_p.rs` — single-threaded `|` implementation (cooperative streaming).
- `double_p.rs` — `||` implementation (isolated interpreter contexts).
- `async_p.rs` — `|&|` implementation (async worker pool integration).
- `shared_p.rs` — `|:|` implementation (shared-thread binding for scripts/modules).

Bootstrap notes:
- The bootstrap provides safe, well-documented skeletons and a mutex+condvar channel implementation.
- Replace TODOs with interpreter-specific VM context creation, pasta_async integration, and canvas safety primitives.

Suggested next steps:
1. Wire `single_p::run_pipe_single_threaded` into the parser for `|`.
2. Implement a small interpreter-level cooperative scheduler to avoid OS sleeps.
3. Implement VM context spawn for `double_p`.
4. Integrate `async_p` with `pasta_async` worker pool and add thread-safe canvas APIs.
'''
}

def write_file(path: Path, content: str):
    content = dedent(content).lstrip("\n")
    if path.exists():
        new_path = path.with_suffix(path.suffix + ".new")
        new_path.write_text(content)
        print(f"Skipped existing {path.name}; wrote {new_path.name} for review.")
    else:
        path.write_text(content)
        print(f"Created {path}")

def main():
    print("Bootstrapping pipelines module...")
    TARGET_DIR.mkdir(parents=True, exist_ok=True)
    for name, body in FILES.items():
        p = TARGET_DIR / name
        write_file(p, body)
    print("\nBootstrap complete.")
    print("Review .new files if any existed; integrate them manually when ready.")
    print("Next: wire the parser to produce Pipeline AST nodes and call single_p::run_pipe_single_threaded for `|`.")

if __name__ == "__main__":
    main()