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
