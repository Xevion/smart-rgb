use std::collections::HashSet;

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;
use glam::U16Vec2;
use tracing::debug;

use crate::game::terrain::TerrainData;
use crate::game::{
    ActiveAttacks, CoastalTiles, DeterministicRng, NationEntityMap, TerritoryManager, TerritorySize, Troops,
    entities::remove_troops,
    ships::{MAX_SHIPS_PER_NATION, Ship, ShipCount, ShipIdCounter, TICKS_PER_TILE, TROOP_PERCENT},
    world::NationId,
};
use crate::game::{ActiveTurn, BorderCache, CurrentTurn};

/// Event for requesting a ship launch
#[derive(Debug, Clone, Message)]
pub struct LaunchShipMessage {
    pub nation_id: NationId,
    pub target_tile: U16Vec2,
    pub troops: u32,
}

/// Event for ship arrivals at their destination
#[derive(Debug, Clone, Message)]
pub struct ShipArrivalMessage {
    pub owner_id: NationId,
    pub target_tile: U16Vec2,
    pub troops: u32,
}

/// System to handle ship launch requests
/// Validates launch conditions and spawns ship entities as children of nations
/// Uses If<Res<ActiveTurn>> to skip when no active turn
#[allow(clippy::too_many_arguments)]
pub fn launch_ship_system(active_turn: If<Res<ActiveTurn>>, mut launch_events: MessageReader<LaunchShipMessage>, mut commands: Commands, mut ship_id_counter: ResMut<ShipIdCounter>, mut players: Query<(&NationId, &mut Troops, &mut ShipCount)>, terrain: Res<TerrainData>, coastal_tiles: Res<CoastalTiles>, territory_manager: Res<TerritoryManager>, border_cache: Res<BorderCache>, entity_map: Res<NationEntityMap>) {
    let turn_number = active_turn.turn_number;
    let size = territory_manager.size();
    let territory_slice = territory_manager.as_slice();

    for event in launch_events.read() {
        let _guard = tracing::trace_span!(
            "launch_ship",
            nation_id = ?event.nation_id,
            ?event.target_tile
        )
        .entered();

        // Get nation entity
        let Some(&player_entity) = entity_map.0.get(&event.nation_id) else {
            debug!(?event.nation_id, "Nation not found");
            continue;
        };

        // Get nation components
        let Ok((_, mut troops, mut ship_count)) = players.get_mut(player_entity) else {
            debug!(?event.nation_id, "Dead nation cannot launch ships");
            continue;
        };

        // Check ship limit
        if ship_count.0 >= MAX_SHIPS_PER_NATION {
            debug!(
                ?event.nation_id,
                "Nation cannot launch ship: already has {}/{} ships",
                ship_count.0,
                MAX_SHIPS_PER_NATION
            );
            continue;
        }

        // Check troops
        if troops.0 <= 0.0 {
            debug!(?event.nation_id, "Nation has no troops to launch ship");
            continue;
        }

        // Clamp troops to available, use default 20% if 0 requested
        let troops_to_send = if event.troops > 0 {
            let available = troops.0 as u32;
            let clamped = event.troops.min(available);

            if event.troops > available {
                debug!(
                    ?event.nation_id,
                    requested = event.troops,
                    available = available,
                    "Ship launch requested more troops than available, clamping"
                );
            }

            clamped
        } else {
            // Default to 20% of troops if 0 requested
            (troops.0 * TROOP_PERCENT).floor() as u32
        };

        if troops_to_send == 0 {
            debug!(?event.nation_id, "Not enough troops to launch ship");
            continue;
        }

        // Find target's nearest coastal tile
        let target_coastal_tile = crate::game::connectivity::find_coastal_tile_in_region(territory_slice, &terrain, event.target_tile, size);

        let target_coastal_tile = match target_coastal_tile {
            Some(tile) => tile,
            None => {
                debug!(
                    ?event.nation_id,
                    ?event.target_tile,
                    "No coastal tile found in target region"
                );
                continue;
            }
        };

        // Find nation's nearest coastal tile
        let nation_border_tiles = border_cache.get(event.nation_id);
        let launch_tile = nation_border_tiles.and_then(|tiles| crate::game::ships::pathfinding::find_nearest_player_coastal_tile(coastal_tiles.tiles(), tiles, target_coastal_tile));

        let launch_tile = match launch_tile {
            Some(tile) => tile,
            None => {
                debug!(
                    ?event.nation_id,
                    ?event.target_tile,
                    "Nation has no coastal tiles to launch from"
                );
                continue;
            }
        };

        // Calculate water path from launch tile to target coastal tile
        let path = {
            let _guard = tracing::trace_span!("ship_pathfinding", ?launch_tile, ?target_coastal_tile).entered();

            crate::game::ships::pathfinding::find_water_path(&terrain, launch_tile, target_coastal_tile, crate::game::ships::MAX_PATH_LENGTH)
        };

        let path = match path {
            Some(p) => p,
            None => {
                debug!(
                    ?event.nation_id,
                    ?event.target_tile,
                    ?launch_tile,
                    "No water path found"
                );
                continue;
            }
        };

        // Generate ship ID
        let ship_id = ship_id_counter.generate_id();

        // Deduct troops from nation
        troops.0 = remove_troops(troops.0, troops_to_send as f32);

        // Create ship as child of nation entity
        let ship = Ship::new(ship_id, troops_to_send, path, TICKS_PER_TILE, turn_number);

        let ship_entity = commands.spawn(ship).id();
        commands.entity(player_entity).add_child(ship_entity);

        // Increment ship count
        ship_count.0 += 1;

        debug!(
            ?event.nation_id,
            ?event.target_tile,
            troops_to_send,
            ?launch_tile,
            ship_id,
            "Ship launched successfully"
        );
    }
}

/// System to update all ships and emit arrival events
pub fn update_ships_system(mut ships: Query<(Entity, &mut Ship, &ChildOf)>, mut arrival_events: MessageWriter<ShipArrivalMessage>, mut commands: Commands, mut players: Query<(&NationId, &mut ShipCount)>) {
    let _guard = tracing::trace_span!("update_ships", ship_count = ships.iter().len()).entered();

    for (ship_entity, mut ship, parent) in ships.iter_mut() {
        if ship.update() {
            // Ship has arrived at destination
            arrival_events.write(ShipArrivalMessage {
                owner_id: {
                    if let Ok((nation_id, _)) = players.get(parent.0) {
                        *nation_id
                    } else {
                        debug!(ship_id = ship.id, "Ship parent entity missing NationId");
                        commands.entity(ship_entity).despawn();
                        continue;
                    }
                },
                target_tile: ship.target_tile,
                troops: ship.troops,
            });

            if let Ok((nation_id, mut ship_count)) = players.get_mut(parent.0) {
                ship_count.0 = ship_count.0.saturating_sub(1);
                debug!(ship_id = ship.id, nation_id = nation_id.get(), troops = ship.troops, "Ship arrived at destination");
            }

            // Despawn ship
            commands.entity(ship_entity).despawn();
        }
    }
}

/// System to handle ship arrivals and create beachheads
/// Uses If<Res<ActiveTurn>> to skip when no active turn
#[allow(clippy::too_many_arguments)]
pub fn handle_ship_arrivals_system(_active_turn: If<Res<ActiveTurn>>, mut arrival_events: MessageReader<ShipArrivalMessage>, current_turn: Res<CurrentTurn>, terrain: Res<TerrainData>, mut territory_manager: ResMut<TerritoryManager>, mut active_attacks: ResMut<ActiveAttacks>, rng: Res<DeterministicRng>, entity_map: Res<NationEntityMap>, border_cache: Res<BorderCache>, mut players: Query<(&mut Troops, &mut TerritorySize)>, mut commands: Commands) {
    let arrivals: Vec<_> = arrival_events.read().cloned().collect();

    if arrivals.is_empty() {
        return;
    }

    let _guard = tracing::trace_span!("ship_arrivals", arrival_count = arrivals.len()).entered();

    for arrival in arrivals {
        tracing::debug!(
            ?arrival.owner_id,
            ?arrival.target_tile,
            arrival.troops,
            "Ship arrived at destination, establishing beachhead"
        );

        // Step 1: Force-claim the landing tile as beachhead
        let arrival_nation_id = arrival.owner_id;
        let previous_owner = territory_manager.conquer(arrival.target_tile, arrival_nation_id);

        // Step 2: Update nation stats
        if let Some(nation_id) = previous_owner
            && let Some(&prev_entity) = entity_map.0.get(&nation_id)
            && let Ok((mut troops, mut territory_size)) = players.get_mut(prev_entity)
        {
            territory_size.0 = territory_size.0.saturating_sub(1);
            if territory_size.0 == 0 {
                troops.0 = 0.0;
                commands.entity(prev_entity).insert(crate::game::Dead);
            }
        }
        if let Some(&entity) = entity_map.0.get(&arrival_nation_id)
            && let Ok((_, mut territory_size)) = players.get_mut(entity)
        {
            territory_size.0 += 1;
        }

        let turn_number = current_turn.turn.turn_number;
        let size = territory_manager.size();
        let target_tile = arrival.target_tile;
        let troops = arrival.troops;

        // Step 3: Notify active attacks of territory change
        active_attacks.handle_territory_add(target_tile, arrival_nation_id, &territory_manager, &terrain, &rng);

        // Step 4: Create attack from beachhead to expand
        // Find valid attack targets (not water, not our own tiles)
        let valid_targets: Vec<U16Vec2> = crate::game::utils::neighbors(target_tile, size).filter(|&neighbor| terrain.is_conquerable(neighbor) && territory_manager.get_nation_id(neighbor) != Some(arrival_nation_id)).collect();

        // Pick a deterministic random target from valid targets
        if !valid_targets.is_empty() {
            // Deterministic random selection using turn number and beachhead position
            let seed = turn_number.wrapping_mul(31).wrapping_add(target_tile.x as u64).wrapping_add(target_tile.y as u64);

            let index = (seed % valid_targets.len() as u64) as usize;
            let attack_target_tile = valid_targets[index];

            // Determine the target nation (None if unclaimed)
            let attack_target = territory_manager.get_nation_id(attack_target_tile);

            // Build nation borders map for compatibility
            let nation_borders = border_cache.as_map();
            let beachhead_borders = Some(&HashSet::from([target_tile]));

            crate::game::handle_attack_internal(arrival_nation_id, attack_target, crate::game::TroopCount::Absolute(troops), false, beachhead_borders, turn_number, &territory_manager, &terrain, &mut active_attacks, &rng, &nation_borders, &entity_map, &mut players);
        } else {
            tracing::debug!(?arrival_nation_id, ?target_tile, "Ship landed but no valid attack targets found (all adjacent tiles are water or owned)");
        }
    }
}
