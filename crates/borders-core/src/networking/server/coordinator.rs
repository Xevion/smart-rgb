use crate::game::terrain::TerrainData;
use crate::game::{SpawnManager, TerritoryManager};
use crate::time::Time;
use bevy_ecs::prelude::*;
use tracing::warn;

use super::turn_generator::{SharedTurnGenerator, TurnOutput};
use crate::game::LocalPlayerContext;
use crate::networking::{Intent, ProcessTurnEvent, SourcedIntent, Turn};
use flume::{Receiver, Sender};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Resource for receiving tracked intents from the client
/// This has replaced the old IntentReceiver that used plain Intent
pub type IntentReceiver = crate::networking::client::TrackedIntentReceiver;

#[derive(Resource)]
pub struct TurnReceiver {
    pub turn_rx: Receiver<Turn>,
}

/// Local turn server control handle
#[derive(Resource, Clone)]
pub struct LocalTurnServerHandle {
    pub paused: Arc<AtomicBool>,
    pub running: Arc<AtomicBool>,
}

impl LocalTurnServerHandle {
    pub fn pause(&self) {
        self.paused.store(true, Ordering::SeqCst);
    }

    pub fn resume(&self) {
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn stop(&self) {
        self.running.store(false, Ordering::SeqCst);
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }
}

/// Resource wrapping the shared turn generator and output channel
#[derive(Resource)]
pub struct TurnGenerator {
    generator: SharedTurnGenerator,
    turn_tx: Sender<Turn>,
}

impl TurnGenerator {
    pub fn new(turn_tx: Sender<Turn>) -> Self {
        Self { generator: SharedTurnGenerator::new(), turn_tx }
    }

    /// Skip spawn phase and start the game immediately
    /// This is useful for tests that don't need the spawn phase
    pub fn start_game_immediately(&mut self) {
        self.generator.skip_spawn_phase();
    }
}

/// System to generate turns using Bevy's Update loop
#[allow(clippy::too_many_arguments)]
pub fn generate_turns_system(mut generator: If<ResMut<TurnGenerator>>, server_handle: If<Res<LocalTurnServerHandle>>, intent_receiver: If<Res<IntentReceiver>>, local_context: Option<Res<LocalPlayerContext>>, mut spawns: Option<ResMut<SpawnManager>>, time: Res<Time>, territory: Res<TerritoryManager>, terrain: Res<TerrainData>) {
    let _guard = tracing::trace_span!("generate_turns").entered();
    if !server_handle.is_running() {
        return;
    }

    let is_paused = server_handle.paused.load(Ordering::SeqCst);

    // Get nation ID for wrapping intents (local single-player)
    let Some(local_context) = local_context else {
        return;
    };
    let nation_id = local_context.id;

    // During spawn phase (paused), process intents and update SpawnManager
    if is_paused {
        while let Ok(tracked_intent) = intent_receiver.rx.try_recv() {
            // Wrap tracked intent with nation_id for local single-player
            let sourced_intent = SourcedIntent { source: nation_id, intent_id: tracked_intent.id, intent: tracked_intent.intent.clone() };

            let output = generator.generator.process_intent(sourced_intent.clone());

            // Update SpawnManager for SetSpawn intents (two-pass spawn system)
            if let Intent::SetSpawn { tile_index } = sourced_intent.intent
                && let Some(ref mut spawns) = spawns
            {
                spawns.update_player_spawn(sourced_intent.source, tile_index, &territory, &terrain);
            }

            // SpawnUpdate output is not used here - SpawnManager handles coordination
            let _ = output;
        }

        // Tick the generator to check spawn timeout
        let delta_ms = time.delta().as_secs_f64() * 1000.0;
        let output = generator.generator.tick(delta_ms, vec![]);

        // Handle initial Turn(0) that starts the game
        if let TurnOutput::Turn(turn) = output
            && turn.turn_number == 0
        {
            if let Err(e) = generator.turn_tx.send(turn) {
                warn!("Failed to send Turn(0): {}", e);
            }
            server_handle.resume();
        }

        return;
    }

    // Normal turn generation (after game has started)
    if !generator.generator.game_started() {
        return;
    }

    // Collect all pending intents and wrap them with nation_id
    let mut sourced_intents = Vec::new();
    while let Ok(tracked_intent) = intent_receiver.rx.try_recv() {
        sourced_intents.push(SourcedIntent { source: nation_id, intent_id: tracked_intent.id, intent: tracked_intent.intent });
    }

    // Tick the generator with accumulated time and sourced intents
    let delta_ms = time.delta().as_secs_f64() * 1000.0;
    let output = generator.generator.tick(delta_ms, sourced_intents);

    // Handle Turn output
    match output {
        TurnOutput::Turn(turn) => {
            if let Err(e) = generator.turn_tx.send(turn) {
                warn!("Failed to send turn: {}", e);
            }
        }
        TurnOutput::None | TurnOutput::SpawnUpdate(_) => {}
    }
}

/// System to poll for turns from the local server and emit ProcessTurnEvent
pub fn poll_turns_system(turn_receiver: If<Res<TurnReceiver>>, mut process_turn_writer: MessageWriter<ProcessTurnEvent>) {
    let _guard = tracing::trace_span!("poll_turns").entered();
    while let Ok(turn) = turn_receiver.turn_rx.try_recv() {
        process_turn_writer.write(ProcessTurnEvent(turn));
    }
}
