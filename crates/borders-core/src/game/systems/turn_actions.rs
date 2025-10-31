use bevy_ecs::prelude::*;
use tracing::trace;

use crate::game::TerritoryManager;
use crate::game::ai::bot::Bot;
use crate::game::combat::ActiveAttacks;
use crate::game::core::rng::DeterministicRng;
use crate::game::core::turn_execution::{apply_action, process_bot_actions};
use crate::game::entities::{Dead, TerritorySize, Troops};
use crate::game::ships::LaunchShipMessage;
use crate::game::systems::GameResources;
use crate::game::systems::turn::{ActiveTurn, CurrentTurn};
use crate::game::world::NationId;
use crate::networking::Intent;

/// Process bot AI and apply all actions (bot + player)
/// Runs first in turn execution chain
/// Uses If<Res<ActiveTurn>> to skip when no active turn
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn process_and_apply_actions_system(_active_turn: If<Res<ActiveTurn>>, current_turn: Res<CurrentTurn>, territory_manager: Res<TerritoryManager>, mut active_attacks: ResMut<ActiveAttacks>, mut rng: ResMut<DeterministicRng>, resources: GameResources, mut player_queries: ParamSet<(Query<(&mut Troops, &mut TerritorySize)>, Query<(&NationId, &Troops, &mut Bot), Without<Dead>>)>, mut launch_ship_writer: MessageWriter<LaunchShipMessage>) {
    let turn = &current_turn.turn;

    let _guard = tracing::trace_span!("process_and_apply_actions", turn_number = turn.turn_number, intent_count = turn.intents.len()).entered();

    trace!("Processing actions for turn {} with {} intents", turn.turn_number, turn.intents.len());

    // Use BorderCache for border data
    let nation_borders = resources.border_cache.as_map();

    // Update RNG for this turn
    rng.update_turn(turn.turn_number);

    // Process bot AI to generate actions
    let bot_actions = process_bot_actions(turn.turn_number, &territory_manager, &resources.terrain, &nation_borders, rng.turn_number(), &mut player_queries.p1());

    // PHASE 1: Apply bot actions
    {
        let _guard = tracing::trace_span!("apply_bot_actions", count = bot_actions.len()).entered();

        for (nation_id, action) in bot_actions {
            apply_action(nation_id, action, turn.turn_number, &territory_manager, &resources.terrain, &mut active_attacks, &rng, &nation_borders, &resources.nation_entity_map, &mut player_queries.p0(), &mut launch_ship_writer);
        }
    }

    // PHASE 2: Apply player intents
    for sourced_intent in &turn.intents {
        match &sourced_intent.intent {
            Intent::Action(action) => {
                apply_action(sourced_intent.source, action.clone(), turn.turn_number, &territory_manager, &resources.terrain, &mut active_attacks, &rng, &nation_borders, &resources.nation_entity_map, &mut player_queries.p0(), &mut launch_ship_writer);
            }
            Intent::SetSpawn { .. } => {}
        }
    }
}
