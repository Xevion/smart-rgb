//! Shared networking types and events

use std::collections::HashMap;

use bevy_ecs::prelude::Message;
use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

use super::protocol::SourcedIntent;
use crate::{game::NationId, game::core::action::GameAction};

/// Network mode configuration for the game
pub enum NetworkMode {
    /// Local single-player or hotseat mode
    Local,
    /// Remote multiplayer mode (non-WASM only)
    #[cfg(not(target_arch = "wasm32"))]
    Remote { server_address: String },
}

// Shared event types
#[derive(Message, Debug, Clone, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub struct IntentEvent(pub Intent);

#[derive(Message, Debug, Clone, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub struct ProcessTurnEvent(pub Turn);

/// Event containing spawn configuration update from server (multiplayer)
#[derive(Message, Debug, Clone)]
pub struct SpawnConfigEvent(pub HashMap<NationId, glam::U16Vec2>);

/// Network wrapper for player intents
///
/// Intent is the network-layer representation of player intents.
/// It has two variants:
/// - Action: State-recorded game actions that appear in game history (replays)
/// - SetSpawn: Ephemeral spawn selection that doesn't pollute game history
///
/// Note: Bot actions are NOT sent as intents - they are calculated
/// deterministically on each client during turn execution.
///
/// Player identity is derived from the connection (server-side) and wrapped
/// in SourcedIntent to prevent spoofing.
#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub enum Intent {
    /// State-recorded game action (appears in game history for replays)
    Action(GameAction),
    /// Ephemeral spawn selection (not recorded in history)
    /// Only valid during spawn phase, ignored after game starts
    /// Player ID is derived from connection by server
    SetSpawn {
        #[serde(with = "crate::game::utils::u16vec2_serde")]
        tile_index: glam::U16Vec2,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub struct Turn {
    pub turn_number: u64,
    pub intents: Vec<SourcedIntent>,
}
