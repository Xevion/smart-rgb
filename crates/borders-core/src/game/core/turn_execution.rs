use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use glam::U16Vec2;

use crate::game::ai::bot::Bot;
use crate::game::combat::ActiveAttacks;
use crate::game::core::action::{GameAction, TroopCount};
use crate::game::core::rng::DeterministicRng;
use crate::game::entities::{Dead, NationEntityMap, TerritorySize, Troops, remove_troops};
use crate::game::ships::LaunchShipMessage;
use crate::game::terrain::data::TerrainData;
use crate::game::world::{NationId, TerritoryManager};
use crate::networking::{Intent, Turn};

/// Execute bot AI to generate actions
/// This must be called before execute_turn to avoid query conflicts
/// Returns (nation_id, action) pairs
pub fn process_bot_actions(turn_number: u64, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng_seed: u64, bots: &mut Query<(&NationId, &Troops, &mut Bot), Without<Dead>>) -> Vec<(NationId, GameAction)> {
    let mut bot_actions = Vec::new();

    for (nation_id, troops, mut bot) in &mut *bots {
        if let Some(action) = bot.tick(turn_number, *nation_id, troops, territory_manager, terrain, nation_borders, rng_seed) {
            bot_actions.push((*nation_id, action));
        }
    }

    bot_actions
}

/// Execute a full game turn
#[allow(clippy::too_many_arguments)]
pub fn execute_turn(turn: &Turn, turn_number: u64, bot_actions: Vec<(NationId, GameAction)>, territory_manager: &mut TerritoryManager, terrain: &TerrainData, active_attacks: &mut ActiveAttacks, rng: &mut DeterministicRng, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>, is_bot_query: &Query<Has<Bot>>, launch_ship_writer: &mut MessageWriter<LaunchShipMessage>) {
    let _guard = tracing::trace_span!("execute_turn", turn_number, intent_count = turn.intents.len(), bot_action_count = bot_actions.len()).entered();

    // Update RNG for this turn
    rng.update_turn(turn_number);

    // PHASE 1: Process bot actions (deterministic, based on turn N-1 state)
    {
        let _guard = tracing::trace_span!("apply_bot_actions", count = bot_actions.len()).entered();

        for (nation_id, action) in bot_actions {
            apply_action(nation_id, action, turn_number, territory_manager, terrain, active_attacks, rng, nation_borders, entity_map, nations, launch_ship_writer);
        }
    }

    // PHASE 2: Process player intents (from network)
    for sourced_intent in &turn.intents {
        match &sourced_intent.intent {
            Intent::Action(action) => {
                apply_action(sourced_intent.source, action.clone(), turn_number, territory_manager, terrain, active_attacks, rng, nation_borders, entity_map, nations, launch_ship_writer);
            }
            Intent::SetSpawn { .. } => {}
        }
    }

    // PHASE 3: Tick game systems (attacks, etc.)
    active_attacks.tick(entity_map, nations, territory_manager, terrain, nation_borders, rng, is_bot_query);
}

/// Apply a game action (attack or ship launch)
#[allow(clippy::too_many_arguments)]
pub fn apply_action(nation_id: NationId, action: GameAction, turn_number: u64, territory_manager: &TerritoryManager, terrain: &TerrainData, active_attacks: &mut ActiveAttacks, rng: &DeterministicRng, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>, launch_ship_writer: &mut MessageWriter<LaunchShipMessage>) {
    match action {
        GameAction::Attack { target, troops } => {
            handle_attack(nation_id, target, troops, turn_number, territory_manager, terrain, active_attacks, rng, nation_borders, entity_map, nations);
        }
        GameAction::LaunchShip { target_tile, troops } => {
            launch_ship_writer.write(LaunchShipMessage { nation_id, target_tile, troops });
        }
    }
}

/// Handle nation spawn at a given tile
#[allow(clippy::too_many_arguments)]
pub fn handle_spawn(nation_id: NationId, tile: U16Vec2, territory_manager: &mut TerritoryManager, terrain: &TerrainData, active_attacks: &mut ActiveAttacks, rng: &DeterministicRng, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>) {
    if territory_manager.has_owner(tile) || !terrain.is_conquerable(tile) {
        tracing::debug!(
            nation_id = %nation_id,
            ?tile,
            "Spawn on occupied/water tile ignored"
        );
        return;
    }

    // Claim 5x5 territory around spawn point
    let size = territory_manager.size();

    // We need to work with territory data directly to use the helper function
    // Convert TerritoryManager slice to Vec for modification
    let mut territories: Vec<_> = territory_manager.as_slice().to_vec();
    let changed = crate::game::systems::spawn_territory::claim_spawn_territory(tile, nation_id, &mut territories, terrain, size);

    // Apply changes back to TerritoryManager
    if !changed.is_empty() {
        for &tile_pos in &changed {
            territory_manager.conquer(tile_pos, nation_id);
        }

        // Update nation stats
        if let Some(&entity) = entity_map.0.get(&nation_id)
            && let Ok((_, mut territory_size)) = nations.get_mut(entity)
        {
            territory_size.0 += changed.len() as u32;
        }

        // Notify active attacks that territory changed
        for &t in &changed {
            active_attacks.handle_territory_add(t, nation_id, territory_manager, terrain, rng);
        }
    }
}

/// Handle an attack action
#[allow(clippy::too_many_arguments)]
pub fn handle_attack(nation_id: NationId, target: Option<NationId>, troops: u32, turn_number: u64, territory_manager: &TerritoryManager, terrain: &TerrainData, active_attacks: &mut ActiveAttacks, rng: &DeterministicRng, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>) {
    handle_attack_internal(nation_id, target, TroopCount::Absolute(troops), true, None, turn_number, territory_manager, terrain, active_attacks, rng, nation_borders, entity_map, nations);
}

/// Handle attack with specific border tiles and troop allocation
#[allow(clippy::too_many_arguments)]
pub fn handle_attack_internal(nation_id: NationId, target: Option<NationId>, troop_count: TroopCount, deduct_from_nation: bool, border_tiles: Option<&HashSet<U16Vec2>>, turn_number: u64, territory_manager: &TerritoryManager, terrain: &TerrainData, active_attacks: &mut ActiveAttacks, rng: &DeterministicRng, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>) {
    // Validate not attacking self
    if target == Some(nation_id) {
        tracing::debug!(
            nation_id = ?nation_id,
            "Attack on own nation ignored"
        );
        return;
    }

    let troops = match troop_count {
        TroopCount::Ratio(ratio) => {
            let Some(&entity) = entity_map.0.get(&nation_id) else {
                return;
            };
            if let Ok((troops, _)) = nations.get(entity) {
                troops.0 * ratio
            } else {
                return;
            }
        }
        TroopCount::Absolute(count) => count as f32,
    };

    // Clamp troops to available and deduct from nation's pool when creating the attack (if requested)
    if deduct_from_nation {
        let Some(&entity) = entity_map.0.get(&nation_id) else {
            return;
        };
        if let Ok((mut troops_comp, _)) = nations.get_mut(entity) {
            let available = troops_comp.0;
            let clamped_troops = troops.min(available);

            if troops > available {
                tracing::warn!(
                    nation_id = ?nation_id,
                    requested = troops,
                    available = available,
                    "Attack requested more troops than available, clamping to available"
                );
            }

            troops_comp.0 = remove_troops(troops_comp.0, clamped_troops);
        } else {
            return;
        }
    }

    let border_tiles_to_use = border_tiles.or_else(|| nation_borders.get(&nation_id).copied());

    match target {
        None => {
            // Attack unclaimed territory
            if entity_map.0.contains_key(&nation_id) {
                active_attacks.schedule_unclaimed(nation_id, troops, border_tiles_to_use, territory_manager, terrain, nation_borders, turn_number, rng);
            }
        }
        Some(target_id) => {
            // Attack specific nation
            let attacker_exists = entity_map.0.contains_key(&nation_id);
            let target_exists = entity_map.0.contains_key(&target_id);

            if attacker_exists && target_exists {
                active_attacks.schedule_attack(nation_id, target_id, troops, border_tiles_to_use, territory_manager, terrain, nation_borders, turn_number, rng);
            }
        }
    }
}
