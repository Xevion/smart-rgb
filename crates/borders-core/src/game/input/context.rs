use bevy_ecs::prelude::*;
use serde::{Deserialize, Serialize};

use crate::game::NationId;

/// Represents the outcome for a specific player (local, not shared)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerOutcome {
    /// Player has won the game
    Victory,
    /// Player has been eliminated/defeated
    Defeat,
}

/// Local player context - CLIENT-SPECIFIC state, NOT part of deterministic game state
///
/// **Important: This is LOCAL context, not shared/deterministic state!**
///
/// This resource contains information specific to THIS client's perspective:
/// - Which player ID this client controls
/// - Whether this player won/lost (irrelevant to other clients)
/// - Whether this client can send commands or is spectating
///
/// In multiplayer:
/// - Each client has their own LocalPlayerContext with different player IDs
/// - One client may have `my_outcome = Victory` while others have `Defeat`
/// - A spectator would have `can_send_intents = false`
/// - The shared game state continues running regardless
#[derive(Resource)]
pub struct LocalPlayerContext {
    /// The player ID for this client
    pub id: NationId,

    /// The outcome for this specific player (if determined)
    /// None = still playing, Some(Victory/Defeat) = game ended for this player
    pub my_outcome: Option<PlayerOutcome>,

    /// Whether this client can send intents (false when defeated or spectating)
    pub can_send_intents: bool,
}

impl LocalPlayerContext {
    /// Create a new local player context for the given player ID
    pub fn new(id: NationId) -> Self {
        Self { id, my_outcome: None, can_send_intents: true }
    }

    /// Mark the local player as defeated
    pub fn mark_defeated(&mut self) {
        self.my_outcome = Some(PlayerOutcome::Defeat);
        self.can_send_intents = false;
    }

    /// Mark the local player as victorious
    pub fn mark_victorious(&mut self) {
        self.my_outcome = Some(PlayerOutcome::Victory);
        // Player can still send intents after victory (to continue playing if desired)
        // Or set to false if you want to prevent further actions
    }

    /// Check if the local player is still actively playing
    #[inline]
    pub fn is_playing(&self) -> bool {
        self.my_outcome.is_none() && self.can_send_intents
    }
}
