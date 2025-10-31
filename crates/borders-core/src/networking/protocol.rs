//! Network protocol for multiplayer client-server communication

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};

use crate::{game::NationId, networking::Intent};
use glam::U16Vec2;
use std::collections::HashMap;

/// Intent wrapper with source nation ID assigned by server
///
/// The server wraps all intents with the authenticated source nation ID
/// to prevent client spoofing. The intent_id is echoed back from the
/// client for round-trip tracking.
#[derive(Debug, Clone, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SourcedIntent {
    /// Authenticated nation ID (assigned by server)
    pub source: NationId,
    /// Client-assigned ID for tracking (echoed back by server)
    pub intent_id: u64,
    /// The actual intent payload
    pub intent: Intent,
}

/// Network message protocol for client-server communication
#[derive(Debug, Clone, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub enum NetMessage {
    /// Server assigns nation ID to client
    ServerConfig { nation_id: NationId },
    /// Client sends intent to server with tracking ID
    Intent { id: u64, intent: Intent },
    /// Server broadcasts turn to all clients with sourced intents
    Turn { turn: u64, intents: Vec<SourcedIntent> },
    /// Server broadcasts current spawn configuration during spawn phase
    /// Maps nation_id -> tile_position for all nations who have chosen spawns
    SpawnConfiguration { spawns: HashMap<NationId, U16Vec2> },
}

/// Shared constants across all binaries for deterministic behavior
pub const NETWORK_SEED: u64 = 0xC0FFEE;
pub const TICK_MS: u64 = 100;
