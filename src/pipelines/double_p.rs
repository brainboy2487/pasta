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
