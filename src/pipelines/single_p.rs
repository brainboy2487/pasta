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
