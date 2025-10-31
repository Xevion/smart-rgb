use super::connection::Connection;
use crate::{
    game::{SpawnManager, SpawnPoint, TerrainData, TerritoryManager, systems::turn::CurrentTurn},
    networking::{IntentEvent, SpawnConfigEvent},
};
use bevy_ecs::prelude::*;
use tracing::{debug, info};

/// Resource for receiving tracked intents from the client
pub type IntentReceiver = super::connection::TrackedIntentReceiver;

/// Unified intent sending system that works for both local and remote
///
/// Uses the generic `Connection` resource to abstract transport mechanism.
pub fn send_intent_system(mut intent_events: MessageReader<IntentEvent>, mut connection: ResMut<Connection>, current_turn: Option<Res<CurrentTurn>>) {
    let turn_number = current_turn.as_ref().map(|ct| ct.turn.turn_number).unwrap_or(0);

    for event in intent_events.read() {
        debug!("Sending intent: {:?}", event.0);
        connection.send_intent(event.0.clone(), turn_number);
    }
}

/// System to handle spawn configuration updates from server
/// Updates local SpawnManager with remote player spawn positions
pub fn handle_spawn_config_system(mut events: MessageReader<SpawnConfigEvent>, mut spawns: If<ResMut<SpawnManager>>, territory: Res<TerritoryManager>, terrain: Res<TerrainData>) {
    for event in events.read() {
        // Update player spawns from server
        spawns.player_spawns.clear();
        for (&player_id, &tile_index) in &event.0 {
            spawns.player_spawns.push(SpawnPoint::new(player_id, tile_index));
        }

        // Recalculate bot spawns based on updated player positions
        spawns.current_bot_spawns = crate::game::ai::bot::recalculate_spawns_with_players(spawns.initial_bot_spawns.clone(), &spawns.player_spawns, &territory, &terrain, spawns.rng_seed);

        info!("Updated spawn manager with {} player spawns from server", spawns.player_spawns.len());
    }
}
