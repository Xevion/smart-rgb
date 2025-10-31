//! Tauri-Bevy integration setup
//!
//! This module provides the integration setup between Tauri and Bevy,
//! configuring shared resources and systems.

use borders_core::game::NationId;
use borders_core::game::input::InputQueue;
use borders_core::ui::protocol::LeaderboardSnapshot;
use borders_core::{
    game::{Game, GameBuilder, TerrainData, Update},
    networking::NetworkMode,
};
use std::sync::{Arc, Mutex};
use tauri::Manager;
use tracing::{error, info};

use crate::transport::{TauriTransport, cache_leaderboard_snapshot_system};

pub fn generate_tauri_context() -> tauri::Context {
    tauri::generate_context!()
}

/// Resources needed before Game creation
#[allow(dead_code)]
pub struct TauriResources {
    pub transport: TauriTransport,
    pub shared_leaderboard_state: Arc<Mutex<Option<LeaderboardSnapshot>>>,
    pub input_queue: Arc<InputQueue>,
}

/// Phase 1: Setup Tauri resources and register message queue
///
/// This happens at startup, BEFORE the Game/World is created.
/// It creates the message queue so frontend can send messages.
pub fn setup_tauri_resources(tauri_app: &tauri::App) -> TauriResources {
    let _guard = tracing::debug_span!("setup_tauri_resources").entered();
    tracing::debug!("Setting up Tauri resources (no World yet)");

    // Create shared state for game state recovery (leaderboard only)
    let shared_leaderboard_state = Arc::new(Mutex::new(None::<LeaderboardSnapshot>));

    // Create transport for Tauri frontend (handles both render and UI communication)
    let transport = TauriTransport::new(tauri_app.handle().clone());

    // Create input queue once - will be reused across game instances
    let input_queue = Arc::new(InputQueue::new());
    let input_sender = input_queue.sender();

    // Register message queue, binary channel, and input sender with Tauri
    tauri_app.manage(transport.inbound_messages());
    tauri_app.manage(transport.binary_channel());
    tauri_app.manage(input_sender);
    tauri_app.manage(shared_leaderboard_state.clone());

    tracing::debug!("Tauri resources registered, ready to receive messages");

    TauriResources { transport, shared_leaderboard_state, input_queue }
}

/// Phase 2: Create Game with all plugins and resources
///
/// This happens when StartGame arrives. Creates the full Game/World in one shot.
pub fn create_game(tauri_resources: &TauriResources) -> Game {
    let _guard = tracing::debug_span!("create_game").entered();

    // Load terrain FIRST, before creating the Game
    info!("Loading terrain data...");
    let terrain_data = match TerrainData::load_world_map() {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to load World map: {}", e);
            panic!("Cannot start game without terrain data");
        }
    };

    let terrain_arc = Arc::new(terrain_data);

    // Create the Game with all configuration via GameBuilder
    info!("Creating Game with GameBuilder...");
    let mut game = GameBuilder::new().with_map(terrain_arc).with_bots(500).with_local_player(NationId::ZERO).with_network(NetworkMode::Local).with_frontend(tauri_resources.transport.clone()).with_input_queue(tauri_resources.input_queue.clone()).build();

    // Insert shared resources into ECS (Tauri-specific resources)
    game.insert_non_send_resource(tauri_resources.shared_leaderboard_state.clone());

    // Add the leaderboard caching system (Tauri-specific)
    game.add_systems(Update, cache_leaderboard_snapshot_system);

    info!("Game created and initialized successfully");
    game
}
