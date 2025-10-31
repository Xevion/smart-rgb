//! Async input queue for platform-to-game communication

use flume::{Receiver, Sender};

use super::events::InputEvent;

/// Input queue using flume channel for lock-free, async-friendly input handling
///
/// Platforms send InputEvent via the sender, and Game drains them each frame
/// before running systems. This decouples platform input from ECS execution.
pub struct InputQueue {
    sender: Sender<InputEvent>,
    receiver: Receiver<InputEvent>,
}

impl InputQueue {
    /// Create a new unbounded input queue
    pub fn new() -> Self {
        let (sender, receiver) = flume::unbounded();
        Self { sender, receiver }
    }

    /// Get a sender for platforms to send input events
    pub fn sender(&self) -> Sender<InputEvent> {
        self.sender.clone()
    }

    /// Drain all pending input events from the queue
    ///
    /// Called internally by Game::update() to convert queued events into Bevy messages
    pub(crate) fn drain(&self) -> impl Iterator<Item = InputEvent> + '_ {
        self.receiver.try_iter()
    }
}

impl Default for InputQueue {
    fn default() -> Self {
        Self::new()
    }
}
