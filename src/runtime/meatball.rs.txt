// src/runtime/meatball.rs
//! Lightweight "Meatball" micro‑VMs for isolated Pasta processes.
//!
//! This module contains the initial scaffolding for the meatball system
//! described in the design document (`Untitled-1`). A Meatball is essentially
//! an independent interpreter instance with its own heap, scheduler, and
//! communication channels. The current implementation is intentionally minimal
//! and will be expanded in subsequent iterations.
//!
//! At this stage we provide:
//! * `Meatball` struct representing an isolated VM instance.
//! * `MeatballConfig` placeholder for configuration options.
//! * `MeatballHandle` which wraps the running thread and allows sending
//!   messages and joining.
//! * `Saucepan` supervisor which can spawn and track multiple meatballs.
//!
//! The internals are currently backed by an `Executor` instance and simple
//! mpsc channels; future work will introduce proper GC isolation, resource
//! quotas, and richer message types.

use std::collections::HashMap;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::{self, JoinHandle};

use anyhow::Result;

use crate::interpreter::Executor;
use crate::interpreter::environment::Value;

/// Unique identifier for a meatball instance.
pub type MeatballID = u64;

/// Simple messages that can be exchanged with a meatball.
/// For now we just support passing `Value` objects; the enum can be
/// extended later for control messages, interrupts, etc.
#[derive(Debug, Clone)]
pub enum Message {
    Value(Value),
}

/// Configuration options for a meatball. Currently empty; added for
/// forward-compatibility.
#[derive(Debug, Clone, Default)]
pub struct MeatballConfig {
    /// Optional human-readable name for debugging.
    pub name: Option<String>,
    // TODO: resource limits, GC settings, device affinity, etc.
}

/// A handle to a running meatball. Allows sending messages and joining the
/// underlying thread.
pub struct MeatballHandle {
    pub id: MeatballID,
    sender: Sender<Message>,
    /// Receiver for messages produced by the meatball.
    pub receiver: Receiver<Message>,
    join: Option<JoinHandle<Result<()>>>,
}

impl MeatballHandle {
    /// Send a message to the meatball. Returns error if the meatball has
    /// already shut down.
    pub fn send(&self, msg: Message) -> Result<()> {
        self.sender
            .send(msg)
            .map_err(|e| anyhow::anyhow!("send failed: {}", e))
    }

    /// Attempt to receive a message from the meatball. This call will block
    /// until a message is available or the meatball has shut down.
    pub fn recv(&self) -> Result<Message> {
        self.receiver
            .recv()
            .map_err(|e| anyhow::anyhow!("receive failed: {}", e))
    }

    /// Wait for the meatball thread to finish. Returns any error produced
    /// during execution.
    pub fn join(self) -> Result<()> {
        if let Some(join) = self.join {
            // ensure we propagate any error, but drop the returned unit
            let _ = join.join().map_err(|e| anyhow::anyhow!("thread panicked: {:?}", e))?;
        } else {
            // nothing to join
        }
        Ok(())
    }
}

/// An isolated Meatball instance containing its own executor and message
/// queues. Clients rarely manipulate this struct directly; they typically
/// interact via a `MeatballHandle` returned by `Saucepan::spawn`.
pub struct Meatball {
    pub id: MeatballID,
    pub executor: Executor,
    inbox: Receiver<Message>,
    outbox: Sender<Message>,
    config: MeatballConfig,
}

impl Meatball {
    /// Create a new meatball with the given configuration.
    pub fn new(
        id: MeatballID,
        config: MeatballConfig,
    ) -> (Self, Sender<Message>, Receiver<Message>) {
        let (tx_in, rx_in) = mpsc::channel();
        let (tx_out, rx_out) = mpsc::channel();
        let mb = Meatball {
            id,
            executor: Executor::new(),
            inbox: rx_in,
            outbox: tx_out.clone(),
            config,
        };
        // Return the meatball, the sender for its inbox, and the receiver for its outbox.
        (mb, tx_in, rx_out)
    }
}

/// Supervisor for a collection of meatballs. The Saucepan keeps track of
/// active meatballs, supports spawning new ones, and can query or terminate
/// them.
pub struct Saucepan {
    next_id: MeatballID,
    registry: HashMap<MeatballID, MeatballHandle>,
}

impl Saucepan {
    /// Construct a new, empty Saucepan.
    pub fn new() -> Self {
        Self {
            next_id: 1,
            registry: HashMap::new(),
        }
    }

    /// Spawn a meatball that will execute the provided closure. The closure
    /// receives a mutable reference to the meatball's `Executor`. The
    /// returned `MeatballHandle` can be used to send messages or join the
    /// thread.
    pub fn spawn<F>(&mut self, config: MeatballConfig, f: F) -> MeatballHandle
    where
        F: FnOnce(&mut Executor, Receiver<Message>, Sender<Message>) -> Result<()> + Send + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;

        // create Meatball and obtain handle to its inbox
        let (mut meatball, inbox_sender, outbox_receiver) = Meatball::new(id, config.clone());

        // We'll spawn a thread that runs the provided closure. The closure is
        // responsible for polling the inbox and sending responses on the
        // outbox.
        let outbox = meatball.outbox.clone();
        let handle_thread = thread::spawn(move || {
            // Execute the user-supplied function.
            let res = f(&mut meatball.executor, meatball.inbox, outbox);
            // TODO: optionally perform cleanup, logging, etc.
            res
        });

        let handle = MeatballHandle {
            id,
            sender: inbox_sender,
            receiver: outbox_receiver,
            join: Some(handle_thread),
        };

        // store a copy of the handle in the registry without the join handle
        let mut registry_handle = handle.clone();
        registry_handle.join = None;
        self.registry.insert(id, registry_handle);
        handle
    }

    /// List all currently registered meatball IDs.
    pub fn list(&self) -> Vec<MeatballID> {
        self.registry.keys().cloned().collect()
    }

    /// Attempt to kill a running meatball by id. This simply removes it from
    /// the registry; the underlying thread may continue to run unless the
    /// client cooperates via messages.
    pub fn kill(&mut self, id: MeatballID) -> Option<MeatballHandle> {
        self.registry.remove(&id)
    }
}

// We implement Clone for MeatballHandle in order to store it in the registry
// and also return a copy to the caller. Cloning the handle shares the sender
// (which is cheap) but does not allow multiple joins; only the original
// handle will successfully join the thread.
impl Clone for MeatballHandle {
    fn clone(&self) -> Self {
        // The cloned handle does not share the inbound receiver (there is no
        // way to clone an mpsc::Receiver). The registry never reads from the
        // receiver so we supply a fresh dummy channel that will never yield
        // any messages.
        let (_tx, rx) = mpsc::channel();
        MeatballHandle {
            id: self.id,
            sender: self.sender.clone(),
            receiver: rx,
            // when cloning we intentionally drop the join handle so that only
            // the original owner can `join`. Clones are useful for storing
            // a lightweight reference in registries.
            join: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn meatball_creation_and_basic_message() {
        let mut sauce = Saucepan::new();
        // we'll use the receiver to capture the echoed value
        let mut echo_val: Option<f64> = None;

        let handle = sauce.spawn(MeatballConfig::default(), move |_exe, inbox, outbox| {
            // echo any number messages back to the caller
            while let Ok(msg) = inbox.recv() {
                if let Message::Value(Value::Number(n)) = msg {
                    outbox.send(Message::Value(Value::Number(n)))?;
                    break;
                }
            }
            Ok(())
        });

        // send a number to the meatball and verify we get it back
        handle.send(Message::Value(Value::Number(3.0))).unwrap();
        if let Message::Value(Value::Number(n)) = handle.recv().unwrap() {
            echo_val = Some(n);
        }
        handle.join().unwrap();
        assert_eq!(echo_val, Some(3.0));
    }
}
