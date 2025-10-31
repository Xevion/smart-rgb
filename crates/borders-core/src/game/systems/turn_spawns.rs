use bevy_ecs::prelude::*;
use tracing::info;

use crate::game::TerritoryManager;
use crate::game::combat::ActiveAttacks;
use crate::game::core::rng::DeterministicRng;
use crate::game::core::turn_execution::handle_spawn;
use crate::game::entities::{TerritorySize, Troops};
use crate::game::input::handlers::SpawnPhase;
use crate::game::systems::GameResources;
use crate::game::systems::spawn::SpawnManager;
use crate::game::systems::turn::ActiveTurn;
use crate::networking::server::LocalTurnServerHandle;

use crate::ui::protocol::BackendMessage;

/// Handle spawns and end spawn phase
/// Only processes on Turn 0
/// Uses If<Res<ActiveTurn>> to skip when no active turn
#[allow(clippy::too_many_arguments)]
pub fn handle_spawns_system(active_turn: If<Res<ActiveTurn>>, spawn_manager: Option<Res<SpawnManager>>, mut spawn_phase: ResMut<SpawnPhase>, mut backend_messages: MessageWriter<BackendMessage>, server_handle: Option<Res<LocalTurnServerHandle>>, mut territory_manager: ResMut<TerritoryManager>, mut active_attacks: ResMut<ActiveAttacks>, rng: Res<DeterministicRng>, resources: GameResources, mut players: Query<(&mut Troops, &mut TerritorySize)>) {
    // Only process on Turn 0
    if active_turn.turn_number != 0 {
        return;
    }

    let _guard = tracing::trace_span!("handle_spawns").entered();

    // Apply ALL spawns (both human player and bots) to game state on Turn(0)
    if let Some(ref spawn_mgr) = spawn_manager {
        let all_spawns = spawn_mgr.get_all_spawns();
        tracing::debug!("Applying {} spawns to game state on Turn(0)", all_spawns.len());
        for spawn in all_spawns {
            handle_spawn(spawn.nation, spawn.tile, &mut territory_manager, &resources.terrain, &mut active_attacks, &rng, &resources.nation_entity_map, &mut players);
        }
    }

    // End spawn phase
    if spawn_phase.active {
        spawn_phase.active = false;

        backend_messages.write(BackendMessage::SpawnPhaseEnded);

        info!("Spawn phase ended after Turn(0) execution");

        if let Some(ref handle) = server_handle {
            handle.resume();
            info!("Local turn server resumed - game started");
        }
    }
}
