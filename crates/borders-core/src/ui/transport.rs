//! Shared render bridge infrastructure for platform-agnostic rendering
//!
//! This module provides the common logic for rendering bridges across platforms
//! (WASM, Tauri, etc.), with platform-specific transport mechanisms abstracted
//! behind the RenderBridgeTransport trait.

use crate::game::builder::PreviousSpawnState;
use crate::game::entities::{Dead, NationColor};
use crate::game::input::handlers::SpawnPhase;
use crate::game::systems::turn::ActiveTurn;
use crate::game::{NationId, SpawnManager, TerritoryManager, TileOwnership};
use crate::prelude::TerrainData;
use crate::ui::protocol::{BackendMessage, BinaryMessageType, BinaryTerritoryDelta, FrontendMessage, RgbColor, TileChange};
use bevy_ecs::prelude::*;
use tracing::{error, info, trace, warn};

/// Trait for platform-specific frontend communication
///
/// This abstracts the actual mechanism for bidirectional frontend communication,
/// allowing WASM (JS callbacks), Tauri (channels), and other platforms to implement
/// their own transport while sharing the core logic.
///
/// All platforms use binary channels for terrain/territory data to ensure
/// consistent encoding/decoding and optimal performance.
pub trait FrontendTransport: Send + Sync + 'static {
    /// Send a message from backend to frontend (JSON-serialized control messages)
    fn send_backend_message(&self, message: &BackendMessage) -> Result<(), String>;

    /// Send binary data (init or delta) through unified channel
    ///
    /// Format: [type:1][payload:N]
    /// - type = 0: Init (terrain + territory + nation palette)
    /// - type = 1: Delta (territory changes only)
    ///
    /// See `encode_binary_message` for envelope format details.
    fn send_binary(&self, msg_type: BinaryMessageType, payload: Vec<u8>) -> Result<(), String>;

    /// Try to receive a message from the frontend
    ///
    /// Returns `Some(message)` if a message is available, `None` if not.
    /// This should be non-blocking and called frequently (e.g., every frame).
    fn try_recv_frontend_message(&self) -> Option<FrontendMessage>;
}

/// Resource for managing frontend communication state
#[derive(Resource)]
pub struct RenderBridge {
    pub transport: std::sync::Arc<dyn FrontendTransport>,
    /// Track if we've sent initial data
    pub(crate) initialized: bool,
}

impl RenderBridge {
    pub fn new(transport: std::sync::Arc<dyn FrontendTransport>) -> Self {
        Self { transport, initialized: false }
    }

    /// Reset the bridge to allow re-initialization
    /// This should be called when a game is quit to ensure fresh data is sent on next game start
    pub fn reset(&mut self) {
        self.initialized = false;
    }
}

/// System to send initial render data (terrain, palette, initial territories)
pub fn send_initial_render_data(territory_manager: Res<TerritoryManager>, terrain_data: Res<TerrainData>, nations: Query<(&NationId, &NationColor), Without<Dead>>, mut bridge: If<ResMut<RenderBridge>>) {
    // Early return if already initialized - prevents duplicate sends
    if bridge.initialized {
        return;
    }

    // Don't send initial data for empty state
    let nation_count = nations.iter().count();
    if territory_manager.width() == 0 || territory_manager.height() == 0 || nation_count == 0 {
        trace!("send_initial_render_data: Game not yet populated, waiting...");
        return;
    }

    let _guard = tracing::debug_span!(
        "send_initial_render_data",
        size = ?territory_manager.size(),
        nation_count = nation_count
    )
    .entered();

    // Mark as initialized FIRST to prevent re-execution even if send fails
    // This is important because the frontend callback might not be registered yet
    // on the first few frames, causing send to fail but we don't want to rebuild
    // the expensive RenderInit message multiple times
    bridge.initialized = true;

    // Prepare terrain data
    let size = territory_manager.size();
    let tile_ids = terrain_data.get_tile_ids();
    let palette_colors: Vec<RgbColor> = terrain_data.get_terrain_palette_colors().into_iter().map(|[r, g, b]| RgbColor { r, g, b }).collect();

    info!("Terrain palette: {} colors", palette_colors.len());

    // Build nation palette from ECS
    let nation_palette = {
        let _guard = tracing::trace_span!("build_nation_palette").entered();

        // Allocate only enough space for active nations + a small buffer
        let max_nation_id = nations.iter().map(|(id, _)| id.get()).max().unwrap_or(0) as usize;

        // Allocate palette size as: max(256, max_nation_id + 1) to handle typical nation counts
        let palette_size = (max_nation_id + 1).max(256);
        let mut colors = vec![RgbColor { r: 0, g: 0, b: 0 }; palette_size];

        for (nation_id, nation_color) in nations.iter() {
            let rgba = nation_color.0.to_rgba();
            colors[nation_id.get() as usize] = RgbColor { r: (rgba[0] * 255.0) as u8, g: (rgba[1] * 255.0) as u8, b: (rgba[2] * 255.0) as u8 };
        }
        colors
    };

    // Send terrain, territory, and nation palette via binary channel (both WASM and Tauri)
    {
        let _guard = tracing::trace_span!("send_init_binary", terrain_size = tile_ids.len(), territory_size = territory_manager.as_slice().len(), nation_palette_size = nation_palette.len()).entered();

        let binary_init = crate::ui::protocol::encode_init_binary(size, tile_ids, &palette_colors, territory_manager.as_slice(), &nation_palette);

        if let Err(e) = bridge.transport.send_binary(BinaryMessageType::Init, binary_init) {
            error!("Failed to send init binary data: {}", e);
            bridge.initialized = false;
            return;
        }
    }

    info!("Initialization data sent successfully via binary channel (terrain + territory + nation palette)");
}

/// System to detect and stream territory changes
/// Uses If<Res<ActiveTurn>> to skip when no active turn
pub fn stream_territory_deltas(current_turn: If<Res<ActiveTurn>>, territory_manager: Res<TerritoryManager>, bridge: If<Res<RenderBridge>>) {
    // Gate: Don't send deltas until initial render data has been sent
    if !bridge.initialized {
        return;
    }

    // Skip if TerritoryManager has no changes in its internal buffer
    if !territory_manager.has_changes() {
        return;
    }

    let _guard = tracing::debug_span!("stream_territory_deltas").entered();

    // Build delta from the pre-tracked changes in TerritoryManager
    // Include ALL changed tiles, both owned and unclaimed (65535)
    let changes: Vec<TileChange> = territory_manager
        .iter_changes()
        .map(|pos| {
            let index = territory_manager.pos_to_index(pos);
            let ownership = territory_manager.get_ownership(pos);
            let owner_id: u16 = ownership.into();
            TileChange { index, owner_id }
        })
        .collect();

    if !changes.is_empty() {
        let turn = current_turn.turn_number;

        // Send binary delta through unified channel
        let binary_data = BinaryTerritoryDelta::encode(turn, &changes);
        if let Err(e) = bridge.transport.send_binary(BinaryMessageType::Delta, binary_data) {
            error!("Failed to send binary territory delta: {}", e);
        }
    }
}

/// System to stream spawn preview during spawn phase
///
/// Sends spawn territories (5x5 areas) as temporary territory ownership so players can see
/// their spawn and bot spawns before Turn(0) processes. Uses incremental updates
/// by comparing with previous state to send only changed tiles.
pub fn stream_spawn_preview_deltas(spawn_phase: Res<SpawnPhase>, spawn_manager: Option<Res<SpawnManager>>, territory_manager: Res<TerritoryManager>, terrain_data: Res<TerrainData>, mut previous_state: If<ResMut<PreviousSpawnState>>, bridge: If<Res<RenderBridge>>) {
    // Only run during spawn phase
    if !spawn_phase.active {
        // Clear previous state when spawn phase ends
        if !previous_state.spawns.is_empty() {
            previous_state.spawns.clear();
        }
        return;
    }

    // Don't send until initial render data has been sent
    if !bridge.initialized {
        return;
    }

    let Some(spawn_manager) = spawn_manager else {
        return;
    };

    let _guard = tracing::trace_span!("stream_spawn_preview").entered();

    // Get current spawns (player + bot spawns)
    let current_spawns = spawn_manager.get_all_spawns();

    // Check if spawns have changed by comparing with previous state
    let spawns_changed = current_spawns.len() != previous_state.spawns.len() || current_spawns.iter().zip(previous_state.spawns.iter()).any(|(a, b)| a.nation != b.nation || a.tile != b.tile);

    if !spawns_changed {
        return;
    }

    let map_size = territory_manager.size();

    // Build territories for previous spawns
    let mut old_territories: Vec<TileOwnership> = vec![crate::game::TileOwnership::Unclaimed; (map_size.x as usize) * (map_size.y as usize)];
    for spawn in &previous_state.spawns {
        crate::game::systems::spawn_territory::claim_spawn_territory(spawn.tile, spawn.nation, &mut old_territories, &terrain_data, map_size);
    }

    // Build territories for current spawns
    let mut new_territories: Vec<TileOwnership> = vec![crate::game::TileOwnership::Unclaimed; (map_size.x as usize) * (map_size.y as usize)];
    for spawn in &current_spawns {
        crate::game::systems::spawn_territory::claim_spawn_territory(spawn.tile, spawn.nation, &mut new_territories, &terrain_data, map_size);
    }

    // Compute changed tiles by comparing old vs new territories
    let changes: Vec<TileChange> = old_territories
        .iter()
        .zip(new_territories.iter())
        .enumerate()
        .filter_map(|(idx, (old, new))| {
            if old != new {
                let owner_id: u16 = (*new).into();
                Some(TileChange { index: idx as u32, owner_id })
            } else {
                None
            }
        })
        .collect();

    if !changes.is_empty() {
        // Use turn 0 for spawn preview (before game actually starts)
        let binary_data = BinaryTerritoryDelta::encode(0, &changes);
        if let Err(e) = bridge.transport.send_binary(BinaryMessageType::Delta, binary_data) {
            error!("Failed to send spawn preview delta: {}", e);
        } else {
            trace!("Sent spawn preview with {} changed tile(s) (sample: {:?})", changes.len(), &changes[..changes.len().min(10)]);
        }
    }

    // Update previous state
    previous_state.spawns = current_spawns;
}

/// System that reads BackendMessage events and sends them through the transport
pub(crate) fn emit_backend_messages_system(mut events: MessageReader<BackendMessage>, bridge: Res<RenderBridge>) {
    for event in events.read() {
        if let Err(e) = bridge.transport.send_backend_message(event) {
            warn!("Failed to send backend message through transport: {}", e);
        }
    }
}

/// System that polls the transport for incoming frontend messages and emits them as events
pub(crate) fn ingest_frontend_messages_system(mut messages: MessageWriter<FrontendMessage>, bridge: Res<RenderBridge>) {
    while let Some(message) = bridge.transport.try_recv_frontend_message() {
        messages.write(message);
    }
}
