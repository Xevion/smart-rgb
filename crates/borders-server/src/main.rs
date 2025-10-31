use borders_core::networking::server::{SharedTurnGenerator, TurnOutput};
use borders_core::networking::{NetMessage, SourcedIntent};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{Duration, interval};
use tracing::{error, info};

use crate::{connections::start_server, registry::ServerRegistry};

mod connections;
mod registry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    #[cfg(debug_assertions)]
    let default_filter = "borders_core=debug,borders_server=debug,borders_protocol=debug,info";

    #[cfg(not(debug_assertions))]
    let default_filter = "borders_core=warn,borders_server=warn,borders_protocol=warn,error";

    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter))).init();

    // Initialize log-to-tracing bridge for dependencies using log crate
    tracing_log::LogTracer::init().expect("Failed to set logger");

    // Parse command line arguments
    let args: Vec<String> = std::env::args().collect();
    let bind_address = if args.len() > 1 { args[1].clone() } else { "127.0.0.1:4433".to_string() };

    info!(bind_address = %bind_address, "Starting borders relay server");

    // Create channels for communication
    let (intent_tx, intent_rx) = flume::unbounded::<SourcedIntent>();

    // Create server registry
    let registry = Arc::new(RwLock::new(ServerRegistry::new()));

    // Spawn network listener task
    let intent_tx_clone = intent_tx.clone();
    let registry_clone = registry.clone();
    let bind_addr = bind_address.clone();
    info!("Spawning server listener task...");
    tokio::spawn(async move {
        info!("Server listener task started");
        if let Err(e) = start_server(&bind_addr, intent_tx_clone, registry_clone).await {
            error!(error = %e, "Server listener failed");
        }
    });

    // Create shared turn generator
    let mut turn_generator = SharedTurnGenerator::new();
    let mut tick_interval = interval(Duration::from_millis(100)); // 100ms tick rate

    info!("Server running - spawn phase active");

    loop {
        let _guard = tracing::debug_span!("server_loop").entered();
        tick_interval.tick().await;

        // Process all pending sourced intents
        let mut sourced_intents = Vec::new();
        while let Ok(sourced_intent) = intent_rx.try_recv() {
            let output = turn_generator.process_intent(sourced_intent.clone());

            // Broadcast spawn updates to all clients
            if let TurnOutput::SpawnUpdate(spawn_config) = output {
                let spawn_message = NetMessage::SpawnConfiguration { spawns: spawn_config };
                registry.write().await.broadcast(spawn_message);
            }

            // Collect sourced intents for turn generation
            sourced_intents.push(sourced_intent);
        }

        // Tick the generator with 100ms delta
        let output = turn_generator.tick(100.0, sourced_intents);

        // Handle turn output
        match output {
            TurnOutput::Turn(turn) => {
                let turn_number = turn.turn_number;
                let intent_count = turn.intents.len();
                let client_count = { registry.read().await.client_count() };

                let turn_message = NetMessage::Turn { turn: turn.turn_number, intents: turn.intents.clone() };

                if turn_number == 0 {
                    info!("Broadcasting Turn(0) to start game (spawns already configured)");
                } else if !turn.intents.is_empty() || turn_number.is_multiple_of(100) {
                    let _guard = tracing::trace_span!("turn_broadcast", turn_number = turn_number, intent_count = intent_count, client_count = client_count).entered();

                    info!(turn_number = turn_number, intent_count = intent_count, client_count = client_count, "Broadcasting turn");
                }

                registry.write().await.broadcast(turn_message);
            }
            TurnOutput::None | TurnOutput::SpawnUpdate(_) => {
                // No turn to broadcast this tick
            }
        }
    }
}
