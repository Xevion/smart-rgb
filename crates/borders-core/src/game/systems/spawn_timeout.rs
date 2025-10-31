use bevy_ecs::prelude::*;
use tracing::trace;

use crate::{game::SpawnPhase, time, ui};

/// Tracks spawn phase timeout state on the client side
///
/// This resource is used to:
/// - Show countdown timer in UI
/// - Know when spawn phase is active
/// - Calculate remaining time for display
#[derive(Resource)]
pub struct SpawnTimeout {
    /// Whether spawn phase is currently active
    pub active: bool,

    /// Accumulated time since start (seconds)
    pub elapsed_secs: f32,

    /// Total timeout duration in seconds
    pub duration_secs: f32,

    /// Remaining time in seconds (updated each frame)
    pub remaining_secs: f32,

    /// Whether the initial spawn phase message has been sent
    pub initial_message_sent: bool,
}

impl Default for SpawnTimeout {
    fn default() -> Self {
        Self {
            active: false,
            elapsed_secs: 0.0,
            duration_secs: 5.0, // Local mode: 5 seconds
            remaining_secs: 5.0,
            initial_message_sent: false,
        }
    }
}

impl SpawnTimeout {
    /// Create a new spawn timeout with specified duration
    pub fn new(duration_secs: f32) -> Self {
        Self { active: false, elapsed_secs: 0.0, duration_secs, remaining_secs: duration_secs, initial_message_sent: false }
    }

    /// Start the timeout countdown
    pub fn start(&mut self) {
        if self.elapsed_secs == 0.0 {
            self.active = true;
            self.elapsed_secs = 0.0;
            self.remaining_secs = self.duration_secs;
        }
    }

    /// Update remaining time (call each frame with delta time)
    pub fn update(&mut self, delta_secs: f32) {
        if !self.active {
            return;
        }

        self.elapsed_secs += delta_secs;
        self.remaining_secs = (self.duration_secs - self.elapsed_secs).max(0.0);

        if self.remaining_secs <= 0.0 {
            self.active = false;
        }
    }

    /// Stop the timeout
    pub fn stop(&mut self) {
        self.active = false;
        self.elapsed_secs = 0.0;
    }

    /// Check if timeout has expired
    #[inline]
    pub fn has_expired(&self) -> bool {
        !self.active && self.remaining_secs <= 0.0
    }
}

/// System to manage spawn timeout and emit countdown updates
pub fn manage_spawn_phase_system(mut spawn_timeout: If<ResMut<SpawnTimeout>>, spawn_phase: Res<SpawnPhase>, time: Res<time::Time>, mut backend_messages: MessageWriter<ui::BackendMessage>) {
    if !spawn_phase.active {
        return;
    }

    // Emit initial message if not sent yet
    if !spawn_timeout.initial_message_sent {
        backend_messages.write(ui::BackendMessage::SpawnPhaseUpdate { countdown: None });
        spawn_timeout.initial_message_sent = true;
        trace!("Emitted initial SpawnPhaseUpdate (no countdown)");
    }

    // Emit countdown updates once the timer is active
    if spawn_timeout.active {
        spawn_timeout.update(time.delta_secs());

        let started_at_ms = time.epoch_millis() - (spawn_timeout.elapsed_secs * 1000.0) as u64;

        backend_messages.write(ui::BackendMessage::SpawnPhaseUpdate { countdown: Some(ui::SpawnCountdown { started_at_ms, duration_secs: spawn_timeout.duration_secs }) });

        trace!("SpawnPhaseUpdate: remaining {:.1}s", spawn_timeout.remaining_secs);
    }
}
