// pipelines/async_p.rs
//! Async-pipe (`|&|`) implementation: schedule stages on separate threads.
//! This file contains the integration points for asynchronous pipeline execution.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};
use std::sync::Arc;
use std::thread;

/// Run an async pipe by spawning left and right stages on separate threads.
/// The left stage produces values sent through a channel to the right stage.
/// Returns when both stages complete.
pub fn run_async_pipe<T, L, R>(left: L, right: R, workers: usize) -> PipeResult<()>
where
    L: Fn(ChannelSender<T>) -> PipeResult<()> + Send + 'static,
    R: Fn(ChannelReceiver<T>) -> PipeResult<()> + Send + 'static,
    T: Send + 'static,
{
    // Use workers count to determine channel buffer size
    let buffer_size = workers.max(1) * 16;
    let (tx, rx) = channel::<T>(buffer_size);

    // Spawn left (producer) stage
    let left_handle = thread::Builder::new()
        .name("async-pipe-left".into())
        .spawn(move || {
            left(tx)
        })
        .map_err(|e| PipeError::Other(format!("Failed to spawn left stage: {}", e)))?;

    // Spawn right (consumer) stage
    let right_handle = thread::Builder::new()
        .name("async-pipe-right".into())
        .spawn(move || {
            right(rx)
        })
        .map_err(|e| PipeError::Other(format!("Failed to spawn right stage: {}", e)))?;

    // Wait for both stages to complete
    let left_result = left_handle
        .join()
        .map_err(|_| PipeError::Other("Left stage panicked".into()))?;

    let right_result = right_handle
        .join()
        .map_err(|_| PipeError::Other("Right stage panicked".into()))?;

    // Return first error if any
    left_result?;
    right_result?;

    Ok(())
}

/// Async pipe with multiple workers for parallel processing.
/// Spawns N worker threads that all consume from the same receiver.
pub fn run_async_pipe_parallel<T, L, W>(
    left: L,
    worker: W,
    num_workers: usize,
) -> PipeResult<()>
where
    L: Fn(ChannelSender<T>) -> PipeResult<()> + Send + 'static,
    W: Fn(T) -> PipeResult<()> + Send + Sync + Clone + 'static,
    T: Send + 'static,
{
    let num_workers = num_workers.max(1);
    let buffer_size = num_workers * 16;
    let (tx, rx) = channel::<T>(buffer_size);
    let rx = Arc::new(std::sync::Mutex::new(rx));

    // Spawn producer
    let left_handle = thread::Builder::new()
        .name("async-pipe-producer".into())
        .spawn(move || left(tx))
        .map_err(|e| PipeError::Other(format!("Failed to spawn producer: {}", e)))?;

    // Spawn worker pool
    let mut worker_handles = Vec::with_capacity(num_workers);
    for i in 0..num_workers {
        let rx_clone = Arc::clone(&rx);
        let worker_clone = worker.clone();
        let handle = thread::Builder::new()
            .name(format!("async-pipe-worker-{}", i))
            .spawn(move || -> PipeResult<()> {
                loop {
                    let item = {
                        let rx = rx_clone.lock().map_err(|_| PipeError::Other("Lock poisoned".into()))?;
                        rx.recv().ok()
                    };
                    match item {
                        Some(val) => worker_clone(val)?,
                        None => break, // Channel closed
                    }
                }
                Ok(())
            })
            .map_err(|e| PipeError::Other(format!("Failed to spawn worker {}: {}", i, e)))?;
        worker_handles.push(handle);
    }

    // Wait for producer
    let left_result = left_handle
        .join()
        .map_err(|_| PipeError::Other("Producer panicked".into()))?;
    left_result?;

    // Wait for all workers
    for (i, handle) in worker_handles.into_iter().enumerate() {
        handle
            .join()
            .map_err(|_| PipeError::Other(format!("Worker {} panicked", i)))?
            .map_err(|e| PipeError::Other(format!("Worker {} error: {}", i, e)))?;
    }

    Ok(())
}
