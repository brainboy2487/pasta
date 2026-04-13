// pipelines/shared_p.rs
//! Shared-pipe (`|:|`) implementation: bind two entry points into a single shared thread.
//! Useful when two stages must share interpreter state but still communicate via channels.

use crate::pipelines::pipe_api::{channel, ChannelSender, ChannelReceiver, PipeResult, PipeError};
use std::sync::Arc;

/// Run a shared pipe where both stages execute in a cooperative manner on the current thread.
/// The left stage runs first, producing items into a buffer, then the right stage consumes them.
/// This is useful when stages need to share state without thread synchronization.
pub fn run_shared_pipe<T, A, B>(left_entry: A, right_entry: B) -> PipeResult<()>
where
    A: Fn(ChannelSender<T>) -> PipeResult<()>,
    B: Fn(ChannelReceiver<T>) -> PipeResult<()>,
{
    // Create a bounded channel for communication
    let buffer_size = 256;
    let (tx, rx) = channel::<T>(buffer_size);

    // Run left stage to completion (produces all items)
    left_entry(tx)?;

    // Run right stage to completion (consumes all items)
    right_entry(rx)?;

    Ok(())
}

/// Run a shared pipe with interleaved execution.
/// Uses an iterator-style approach where the right stage pulls from left on demand.
pub fn run_shared_pipe_interleaved<T, P, C>(
    producer: P,
    consumer: C,
) -> PipeResult<()>
where
    P: FnMut() -> Option<T>,
    C: FnMut(T) -> PipeResult<()>,
{
    let mut producer = producer;
    let mut consumer = consumer;

    // Pull items from producer and feed to consumer until exhausted
    while let Some(item) = producer() {
        consumer(item)?;
    }

    Ok(())
}

/// Run a shared pipe with a shared context object.
/// Both stages have mutable access to the context (sequentially, not concurrently).
pub fn run_shared_pipe_with_context<T, Ctx, A, B>(
    context: &mut Ctx,
    left_entry: A,
    right_entry: B,
) -> PipeResult<()>
where
    A: Fn(&mut Ctx, ChannelSender<T>) -> PipeResult<()>,
    B: Fn(&mut Ctx, ChannelReceiver<T>) -> PipeResult<()>,
{
    let buffer_size = 256;
    let (tx, rx) = channel::<T>(buffer_size);

    // Run left stage with context access
    left_entry(context, tx)?;

    // Run right stage with context access
    right_entry(context, rx)?;

    Ok(())
}

/// Shared pipe that wraps both stages in an executor context.
/// This is the primary integration point for PASTA script execution.
pub fn run_shared_script_pipe<T>(
    left_script: &str,
    right_script: &str,
    buffer_size: usize,
) -> PipeResult<Vec<T>>
where
    T: Clone + Send + 'static,
{
    // This is a stub that would integrate with the PASTA executor
    // In practice, this would:
    // 1. Create a single Executor instance
    // 2. Load both scripts into the same environment
    // 3. Wire their entry points through a channel
    // 4. Execute them cooperatively

    let _ = (left_script, right_script, buffer_size);
    Err(PipeError::Other(
        "Script-level shared pipe requires executor integration".into()
    ))
}
