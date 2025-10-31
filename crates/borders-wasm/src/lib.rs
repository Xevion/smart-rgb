pub mod bridge;
pub mod render_bridge;

use borders_core::time::Time;
use borders_core::ui::protocol::FrontendMessage;
use render_bridge::INBOUND_MESSAGES;
use std::sync::atomic::{AtomicBool, Ordering};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use wasm_bindgen::prelude::*;

/// Synchronization flag to ensure callbacks are registered before game starts
static CALLBACKS_READY: AtomicBool = AtomicBool::new(false);

/// Called by frontend worker after both callbacks are registered
#[wasm_bindgen]
pub fn signal_callbacks_ready() {
    CALLBACKS_READY.store(true, Ordering::SeqCst);
    tracing::debug!("Frontend callbacks registered, game can start");
}

#[wasm_bindgen(start)]
pub fn main() {
    // Set up panic hook for better error messages in the browser
    console_error_panic_hook::set_once();

    // Initialize tracing for WASM (outputs to browser console)
    // Debug builds: debug level for our crates, info for dependencies
    // Release builds: warn level for our crates, error for dependencies
    #[cfg(debug_assertions)]
    let level_filter = "borders_core=debug,borders_protocol=debug,borders_wasm=debug,info";

    #[cfg(not(debug_assertions))]
    let level_filter = "borders_core=warn,borders_protocol=warn,borders_wasm=warn,error";

    if let Err(e) = tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(level_filter))
        .with(wasm_tracing::WasmLayer::new(
            wasm_tracing::WasmLayerConfig::new()
                .set_show_fields(true)
                .set_report_logs_in_timings(true)
                .set_console_config(wasm_tracing::ConsoleConfig::ReportWithConsoleColor)
                // Only show origin (filename, line number) in debug builds
                .set_show_origin(true)
                .clone(),
        ))
        .try_init()
    {
        eprintln!("Failed to initialize tracing: {}", e);
    }

    // Log build information
    tracing::info!("Iron Borders v{}", borders_core::build_info::VERSION);
    tracing::info!("Git: {} | Built: {}", borders_core::build_info::git_commit_short(), borders_core::build_info::BUILD_TIME);
    tracing::info!("© 2025 Ryan Walters. All Rights Reserved.");

    // Initialize telemetry in background (non-blocking)
    wasm_bindgen_futures::spawn_local(async {
        borders_core::telemetry::init(borders_core::telemetry::TelemetryConfig::default()).await;
        borders_core::telemetry::track_session_start().await;
        tracing::info!("Telemetry initialized");
    });

    // Start the game immediately (don't wait for telemetry)
    wasm_bindgen_futures::spawn_local(async {
        run().await;
    });
}

/// Wait for StartGame message from frontend
async fn wait_for_start_game() {
    use std::time::Duration;
    use tracing::info;

    info!("Waiting for StartGame message from frontend...");

    loop {
        // Check if StartGame message has arrived
        let should_start = INBOUND_MESSAGES.with(|messages| {
            let mut msgs = messages.borrow_mut();
            if let Some(idx) = msgs.iter().position(|msg| matches!(msg, FrontendMessage::StartGame)) {
                // Remove StartGame from queue so it's not processed again
                msgs.remove(idx);
                true
            } else {
                false
            }
        });

        if should_start {
            info!("StartGame received, creating game...");
            return;
        }

        // Poll every 10ms
        gloo_timers::future::sleep(Duration::from_millis(10)).await;
    }
}

/// Create and run the game until QuitGame is received
async fn run_game_until_quit() {
    use borders_core::game::GameBuilder;
    use borders_core::game::NationId;
    use std::sync::Arc;
    use std::time::Duration;
    use tracing::{error, info};

    // Load terrain first
    info!("Loading terrain data...");
    let terrain_data = match borders_core::game::TerrainData::load_world_map() {
        Ok(data) => data,
        Err(e) => {
            error!("Failed to load World map: {}", e);
            panic!("Cannot start game without terrain data");
        }
    };

    let terrain_arc = Arc::new(terrain_data);

    // Create the Game with all configuration via GameBuilder
    // GameBuilder handles all initialization automatically: plugins, resources, and lifecycle
    info!("Creating Game with GameBuilder...");
    let builder = GameBuilder::new();
    let input_sender = builder.input_sender();

    // Store input sender globally for WASM bindings to access
    bridge::set_input_sender(input_sender);

    let mut game = builder.with_map(terrain_arc).with_bots(500).with_local_player(NationId::ZERO).with_network(borders_core::networking::NetworkMode::Local).with_frontend(render_bridge::WasmTransport).build();

    info!("Game created and initialized successfully");

    // Manual update loop at 60 FPS (worker-compatible)
    let frame_time = Duration::from_millis(16); // ~60 FPS

    loop {
        // Check for QuitGame message (matches desktop's pattern)
        let should_quit = INBOUND_MESSAGES.with(|messages| {
            let mut msgs = messages.borrow_mut();
            if let Some(idx) = msgs.iter().position(|msg| matches!(msg, FrontendMessage::QuitGame)) {
                msgs.remove(idx);
                true
            } else {
                false
            }
        });

        if should_quit {
            info!("QuitGame received, shutting down game...");
            drop(game);
            return;
        }

        // Tick time to measure frame delta
        if let Some(mut time) = game.world_mut().get_resource_mut::<Time>() {
            time.tick();
        }

        game.update();

        gloo_timers::future::sleep(frame_time).await;
    }
}

async fn run() {
    use std::time::Duration;
    use tracing::info;

    // Wait for frontend to register callbacks before game can start
    info!("Waiting for frontend callbacks to be registered...");
    while !CALLBACKS_READY.load(Ordering::SeqCst) {
        gloo_timers::future::sleep(Duration::from_millis(10)).await;
    }
    info!("Frontend callbacks ready");

    // Main loop: wait for StartGame, run game until QuitGame, repeat
    // This allows multiple game sessions (desktop does the same)
    loop {
        // Phase 1: Wait for StartGame message
        wait_for_start_game().await;

        // Phase 2: Run game until QuitGame is received
        run_game_until_quit().await;

        // After QuitGame, loop back to wait for next StartGame
        info!("Game session ended, ready for next StartGame");
    }
}
