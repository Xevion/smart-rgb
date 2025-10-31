use bevy_ecs::prelude::*;

use crate::game::TerritoryManager;
use crate::game::ai::bot::Bot;
use crate::game::combat::ActiveAttacks;
use crate::game::core::rng::DeterministicRng;
use crate::game::entities::{TerritorySize, Troops};
use crate::game::systems::GameResources;

/// Tick active attacks
/// Runs after actions are applied, processes ongoing attacks
#[allow(clippy::too_many_arguments)]
pub fn tick_attacks_system(mut active_attacks: ResMut<ActiveAttacks>, mut territory_manager: ResMut<TerritoryManager>, rng: Res<DeterministicRng>, resources: GameResources, mut nations: Query<(&mut Troops, &mut TerritorySize)>, is_bot_query: Query<Has<Bot>>) {
    let _guard = tracing::trace_span!("tick_attacks").entered();

    // Use BorderCache for border data
    let nation_borders = resources.border_cache.as_map();

    // Tick all active attacks
    active_attacks.tick(&resources.nation_entity_map, &mut nations, &mut territory_manager, &resources.terrain, &nation_borders, &rng, &is_bot_query);
}
