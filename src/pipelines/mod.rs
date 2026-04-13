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
