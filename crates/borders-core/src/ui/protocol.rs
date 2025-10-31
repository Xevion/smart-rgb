//! Protocol for frontend-backend communication
//!
//! This module defines the bidirectional message protocol used for communication
//! between the game core (Bevy/Rust) and the frontend (PixiJS/TypeScript).

use bevy_ecs::message::Message;
use bevy_ecs::prelude::*;
use glam::Vec2;
use serde::{Deserialize, Serialize};

use crate::game::{NationId, input::AttackControls};

// Re-export input types for TypeScript generation
pub use crate::game::input::{
    events::InputEvent,
    types::{ButtonState, KeyCode, MouseButton},
};

/// All messages sent from backend to frontend
/// Binary data (terrain, territory, nation palette, deltas) are sent via separate binary channels
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[serde(tag = "msg_type")]
pub enum BackendMessage {
    /// Complete leaderboard snapshot (includes names, colors, and stats)
    LeaderboardSnapshot(LeaderboardSnapshot),
    /// Dynamic attacks updates
    AttacksUpdate(AttacksUpdatePayload),
    /// Active ships on the map
    ShipsUpdate(ShipsUpdatePayload),
    /// Game has ended with the specified outcome
    GameEnded { outcome: GameOutcome },
    /// Spawn phase update
    /// - countdown: None = phase active, waiting for first spawn
    /// - countdown: Some = countdown in progress with epoch timestamp
    SpawnPhaseUpdate { countdown: Option<SpawnCountdown> },
    /// Spawn phase has ended, game is now active
    SpawnPhaseEnded,
    /// Highlight a specific nation (None to clear)
    HighlightNation { nation_id: Option<NationId> },
}

/// All messages sent from frontend to backend
#[derive(Debug, Clone, Serialize, Deserialize, Message)]
#[serde(tag = "msg_type")]
pub enum FrontendMessage {
    /// Start a new game
    StartGame,
    /// Quit the current game and return to menu
    QuitGame,
    /// Set attack ratio (percentage of troops to use when attacking)
    SetAttackRatio { ratio: f32 },
}

/// Terrain types for map tiles
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[repr(u8)]
pub enum TerrainType {
    Water = 0,
    Land = 1,
    Mountain = 2,
}

/// Binary message types for unified binary channel
///
/// Uses #[repr(u8)] to provide idiomatic conversion to/from bytes.
/// Constants are defined only once here and accessed via `as u8` cast.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryMessageType {
    /// Initial game state (terrain + territory + nation palette)
    Init = 0,
    /// Territory delta (changed tiles only)
    Delta = 1,
}

impl BinaryMessageType {
    /// Convert a byte to a BinaryMessageType
    ///
    /// Returns None if the byte doesn't correspond to a known message type.
    #[inline]
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Init),
            1 => Some(Self::Delta),
            _ => None,
        }
    }
}

/// Encode a binary message with type tag envelope
///
/// Format: [type:1][payload:N]
///
/// The type byte discriminates between Init and Delta messages,
/// allowing both to be sent through a unified binary channel.
pub fn encode_binary_message(msg_type: BinaryMessageType, payload: Vec<u8>) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + payload.len());
    data.push(msg_type as u8);
    data.extend_from_slice(&payload);
    data
}

/// Decode a binary message envelope
///
/// Returns the message type and payload slice, or None if the envelope is invalid.
pub fn decode_binary_envelope(data: &[u8]) -> Option<(BinaryMessageType, &[u8])> {
    if data.is_empty() {
        return None;
    }
    let msg_type = BinaryMessageType::from_u8(data[0])?;
    Some((msg_type, &data[1..]))
}

/// Encode complete initialization data into binary format for channel streaming
///
/// This combines terrain, territory, and nation palette data into a single atomic payload
/// to avoid synchronization issues with multiple messages.
///
/// Format: [terrain_len:4][terrain_data][territory_len:4][territory_data][nation_palette_count:2][nation_palette_rgb:N*3]
///
/// Terrain data format: [width:2][height:2][tile_ids:N][palette_count:2][palette_rgb:N*3]
/// Territory data format: [count:4][tiles...] where tiles = [index:4][owner:2]
///
/// All integers are little-endian
pub fn encode_init_binary(size: glam::U16Vec2, tile_ids: &[u8], terrain_palette: &[RgbColor], territories: &[crate::game::TileOwnership], nation_palette: &[RgbColor]) -> Vec<u8> {
    let tile_count = (size.x as usize) * (size.y as usize);
    assert_eq!(tile_ids.len(), tile_count, "Tile ID count mismatch");

    let terrain_palette_count = terrain_palette.len();
    assert!(terrain_palette_count <= u16::MAX as usize, "Terrain palette too large");

    // Build terrain data
    let terrain_size = 2 + 2 + tile_count + 2 + (terrain_palette_count * 3);
    let mut terrain_data = Vec::with_capacity(terrain_size);

    terrain_data.extend_from_slice(&size.x.to_le_bytes());
    terrain_data.extend_from_slice(&size.y.to_le_bytes());
    terrain_data.extend_from_slice(tile_ids);
    terrain_data.extend_from_slice(&(terrain_palette_count as u16).to_le_bytes());
    for color in terrain_palette {
        terrain_data.extend_from_slice(&[color.r, color.g, color.b]);
    }

    // Build territory data (only nation-owned tiles, filter out unclaimed)
    let claimed_tiles: Vec<(u32, u16)> = territories.iter().enumerate().filter_map(|(index, &ownership)| ownership.nation_id().map(|nation_id| (index as u32, nation_id.get()))).collect();

    let territory_count = claimed_tiles.len() as u32;
    let territory_size = 4 + (claimed_tiles.len() * 6);
    let mut territory_data = Vec::with_capacity(territory_size);

    territory_data.extend_from_slice(&territory_count.to_le_bytes());
    for (index, owner) in claimed_tiles {
        territory_data.extend_from_slice(&index.to_le_bytes());
        territory_data.extend_from_slice(&owner.to_le_bytes());
    }

    // Build nation palette data
    let nation_palette_count = nation_palette.len();
    assert!(nation_palette_count <= u16::MAX as usize, "Nation palette too large");

    // Combine into single payload with length prefixes
    let total_size = 4 + terrain_data.len() + 4 + territory_data.len() + 2 + (nation_palette_count * 3);
    let mut data = Vec::with_capacity(total_size);

    data.extend_from_slice(&(terrain_data.len() as u32).to_le_bytes());
    data.extend_from_slice(&terrain_data);
    data.extend_from_slice(&(territory_data.len() as u32).to_le_bytes());
    data.extend_from_slice(&territory_data);

    // Append nation palette
    data.extend_from_slice(&(nation_palette_count as u16).to_le_bytes());
    for color in nation_palette {
        data.extend_from_slice(&[color.r, color.g, color.b]);
    }

    data
}

/// A single tile change in the territory map
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TileChange {
    /// Tile index (row * width + col)
    pub index: u32,
    /// New owner ID (0-65534 for nations, 65535 for unclaimed)
    pub owner_id: u16,
}

/// Binary format for efficient territory delta streaming
/// This is for the pixel streaming channel, separate from JSON messages
#[derive(Debug)]
pub struct BinaryTerritoryDelta {
    /// Raw bytes: [turn:8][count:4][changes...]
    /// Each change: [index:4][owner:2] = 6 bytes
    pub data: Vec<u8>,
}

impl BinaryTerritoryDelta {
    /// Create binary delta from territory changes
    pub fn encode(turn: u64, changes: &[TileChange]) -> Vec<u8> {
        let count = changes.len() as u32;
        let mut data = Vec::with_capacity(12 + changes.len() * 6);

        // Header: turn (8 bytes) + count (4 bytes)
        data.extend_from_slice(&turn.to_le_bytes());
        data.extend_from_slice(&count.to_le_bytes());

        // Changes: each is index (4 bytes) + owner (2 bytes)
        for change in changes {
            data.extend_from_slice(&change.index.to_le_bytes());
            data.extend_from_slice(&change.owner_id.to_le_bytes());
        }

        data
    }

    /// Decode binary delta back to structured format
    pub fn decode(data: &[u8]) -> Option<(u64, Vec<TileChange>)> {
        if data.len() < 12 {
            return None; // Not enough data for header
        }

        let turn = u64::from_le_bytes([data[0], data[1], data[2], data[3], data[4], data[5], data[6], data[7]]);

        let count = u32::from_le_bytes([data[8], data[9], data[10], data[11]]) as usize;

        let expected_size = 12 + count * 6;
        if data.len() != expected_size {
            return None; // Invalid size
        }

        let mut changes = Vec::with_capacity(count);
        for i in 0..count {
            let offset = 12 + i * 6;
            let index = u32::from_le_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
            let owner_id = u16::from_le_bytes([data[offset + 4], data[offset + 5]]);
            changes.push(TileChange { index, owner_id });
        }

        Some((turn, changes))
    }
}

/// RGB color for nation palette
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct RgbColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Queries sent from frontend to backend about the map
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MapQuery {
    /// Get owner of tile at world coordinates
    GetOwnerAt { pos: Vec2 },
    /// Get detailed tile info by index
    GetTileInfo { tile_index: u32 },
    /// Find any tile owned by nation (for camera centering)
    FindPlayerTerritory { nation_id: NationId },
    /// Convert screen coordinates to tile index
    ScreenToTile { screen_pos: Vec2 },
}

/// Unified leaderboard entry containing both static and dynamic data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardEntry {
    pub id: NationId,
    pub name: String,
    pub color: String, // Hex color without alpha, e.g. "0A44FF"
    pub tile_count: u32,
    pub troops: u32,
    pub territory_percent: f32,
    pub rank: usize,          // Current rank (1-indexed, updates every tick)
    pub display_order: usize, // Visual position (0-indexed, updates every 3rd tick)
}

/// Complete leaderboard snapshot (replaces separate Init/Update)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaderboardSnapshot {
    pub turn: u64,
    pub total_land_tiles: u32,
    pub entries: Vec<LeaderboardEntry>,
    pub client_nation_id: NationId,
}

/// Outcome of the game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum GameOutcome {
    /// Player won the game
    Victory,
    /// Player lost the game
    Defeat,
}

/// Single attack entry for attacks UI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttackEntry {
    pub id: u64,
    pub attacker_id: NationId,
    pub target_id: Option<NationId>, // None for unclaimed territory
    pub troops: u32,
    pub is_outgoing: bool,
}

/// Attacks update payload
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttacksUpdatePayload {
    pub turn: u64,
    pub entries: Vec<AttackEntry>,
}

/// Ships update payload with lifecycle variants
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipsUpdatePayload {
    pub turn: u64,
    pub updates: Vec<ShipUpdateVariant>,
}

/// Ship update variants for efficient delta updates
/// NOTE: SHIP_TICKS_PER_TILE (1) must be synchronized between backend and frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ShipUpdateVariant {
    /// Ship created - full initial state
    Create {
        id: u32,
        owner_id: NationId,
        path: Vec<u32>,
        troops: u32, // Static value, currently unused for rendering
    },
    /// Ship moved to next tile in path
    Move { id: u32, current_path_index: u32 },
    /// Ship destroyed (arrived or cancelled)
    Destroy { id: u32 },
}

// TODO: On client reconnection/late-join, send Create variants for all active ships

/// Countdown state for spawn phase
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpawnCountdown {
    pub started_at_ms: u64,
    pub duration_secs: f32,
}

/// System to handle FrontendMessage events
///
/// NOTE: StartGame and QuitGame are handled directly in main.rs (desktop) or game.worker.ts (browser)
/// to control World creation/destruction at the application level.
pub fn handle_frontend_messages_system(mut frontend_messages: MessageReader<FrontendMessage>, mut attack_controls: ResMut<AttackControls>) {
    for message in frontend_messages.read() {
        match message {
            FrontendMessage::StartGame | FrontendMessage::QuitGame => {
                // These are handled at the application level (main.rs for desktop, game.worker.ts for browser)
                // They control World creation/destruction and should never reach this system
                tracing::trace!("Ignoring {:?} - handled at application level", message);
            }
            FrontendMessage::SetAttackRatio { ratio } => {
                attack_controls.attack_ratio = ratio.clamp(0.01, 1.0);
                tracing::debug!("Attack ratio set to {:.1}%", attack_controls.attack_ratio * 100.0);
            }
        }
    }
}
