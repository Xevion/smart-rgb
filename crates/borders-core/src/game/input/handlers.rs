//! Action handler systems - process semantic input actions

use bevy_ecs::prelude::*;
use tracing::{debug, info, trace};

use crate::game::core::constants::input::*;
use crate::game::systems::borders::BorderCache;
use crate::game::terrain::TerrainData;
use crate::game::{BorderTiles, CoastalTiles, GameAction, LocalPlayerContext, NationEntityMap, SpawnManager, SpawnTimeout, TerritoryManager, Troops};
use crate::networking::{Intent, IntentEvent};

use super::events::{CameraAction, TileClickedAction, UiAction};

/// Resource tracking whether spawn phase is active
#[derive(Resource, Default)]
pub struct SpawnPhase {
    pub active: bool,
}

/// Resource for attack control settings
#[derive(Resource)]
pub struct AttackControls {
    pub attack_ratio: f32,
}

impl Default for AttackControls {
    fn default() -> Self {
        Self { attack_ratio: DEFAULT_ATTACK_RATIO }
    }
}

/// Handle tile clicks during spawn and gameplay phases
#[allow(clippy::too_many_arguments)]
pub fn handle_tile_clicked_system(mut tile_clicked: MessageReader<TileClickedAction>, spawn_phase: Res<SpawnPhase>, local_context: If<Res<LocalPlayerContext>>, mut spawn_manager: Option<ResMut<SpawnManager>>, mut spawn_timeout: Option<ResMut<SpawnTimeout>>, mut intent_writer: MessageWriter<IntentEvent>, territory_manager: Res<TerritoryManager>, terrain: Res<TerrainData>, coastal_tiles: Res<CoastalTiles>, attack_controls: Res<AttackControls>, entity_map: Res<NationEntityMap>, border_query: Query<&BorderTiles>, troops_query: Query<&Troops>) {
    // Can't interact if not allowed to send intents
    if !local_context.can_send_intents {
        return;
    };

    for action in tile_clicked.read() {
        if spawn_phase.active {
            // Spawn phase logic
            handle_spawn_click(action.tile, local_context.as_ref(), &mut spawn_manager, &mut spawn_timeout, &mut intent_writer, &territory_manager, &terrain);
        } else {
            // Gameplay phase logic
            handle_attack_click(action.tile, local_context.as_ref(), &territory_manager, &terrain, &coastal_tiles, &attack_controls, &mut intent_writer, &entity_map, &border_query, &troops_query);
        }
    }
}

/// Handle spawn click logic
#[allow(clippy::too_many_arguments)]
fn handle_spawn_click(tile_coord: glam::U16Vec2, local_context: &LocalPlayerContext, spawn_manager: &mut Option<ResMut<SpawnManager>>, spawn_timeout: &mut Option<ResMut<SpawnTimeout>>, intent_writer: &mut MessageWriter<IntentEvent>, territory_manager: &TerritoryManager, terrain: &TerrainData) {
    let _guard = tracing::trace_span!("spawn_click").entered();

    let tile_ownership = territory_manager.get_ownership(tile_coord);
    if tile_ownership.is_owned() {
        debug!("Spawn click on tile {:?} ignored - occupied", tile_coord);
        return;
    }

    // Check if tile is water/unconquerable
    if !terrain.is_conquerable(tile_coord) {
        debug!("Spawn click on tile {:?} ignored - water or unconquerable", tile_coord);
        return;
    }

    // Player has chosen a spawn location - send to server
    info!("Player {} setting spawn at tile {:?}", local_context.id.get(), tile_coord);

    // Check if this is the first spawn (timer not started yet)
    let is_first_spawn = if let Some(spawn_mgr) = spawn_manager { spawn_mgr.get_player_spawns().is_empty() } else { true };

    // Send SetSpawn intent to server (not Action - this won't be in game history)
    // Server will validate, track, and eventually send Turn(0) when timeout expires
    intent_writer.write(IntentEvent(Intent::SetSpawn { tile_index: tile_coord }));

    // Start spawn timeout on first spawn (spawn_phase plugin will emit countdown updates)
    if is_first_spawn && let Some(timeout) = spawn_timeout {
        timeout.start();
        info!("Spawn timeout started ({:.1}s)", timeout.duration_secs);
    }

    // Update local spawn manager for preview/bot recalculation
    // Note: This only updates the spawn manager, not the game instance
    // The actual game state is updated when Turn(0) is processed
    if let Some(spawn_mgr) = spawn_manager {
        // Update spawn manager (triggers bot spawn recalculation)
        spawn_mgr.update_player_spawn(local_context.id, tile_coord, territory_manager, terrain);

        info!("Spawn manager updated with player {} spawn at tile {:?}", local_context.id.get(), tile_coord);
        info!("Total spawns in manager: {}", spawn_mgr.get_all_spawns().len());
    }
}

/// Handle attack click logic
#[allow(clippy::too_many_arguments)]
fn handle_attack_click(tile_coord: glam::U16Vec2, local_context: &LocalPlayerContext, territory_manager: &TerritoryManager, terrain: &TerrainData, coastal_tiles: &CoastalTiles, attack_controls: &AttackControls, intent_writer: &mut MessageWriter<IntentEvent>, entity_map: &NationEntityMap, border_query: &Query<&BorderTiles>, troops_query: &Query<&Troops>) {
    let _guard = tracing::trace_span!("attack_click").entered();

    let tile_ownership = territory_manager.get_ownership(tile_coord);
    let nation_id = local_context.id;

    // Can't attack own tiles
    if tile_ownership.is_owned_by(nation_id) {
        return;
    }

    // Check if target is water - ignore water clicks
    if terrain.is_navigable(tile_coord) {
        return;
    }

    // Check if target is connected to player's territory
    let size = territory_manager.size();
    let is_connected = crate::game::connectivity::is_connected_to_player(territory_manager.as_slice(), terrain, tile_coord, nation_id, size);

    if is_connected {
        // Target is connected to player's territory - use normal attack
        // Calculate absolute troop count from ratio
        let troops = if let Some(&entity) = entity_map.0.get(&nation_id)
            && let Ok(troops_comp) = troops_query.get(entity)
        {
            (troops_comp.0 * attack_controls.attack_ratio).floor() as u32
        } else {
            0
        };

        intent_writer.write(IntentEvent(Intent::Action(GameAction::Attack { target: tile_ownership.nation_id(), troops })));
        return;
    }

    // Target is NOT connected - need to use ship
    debug!("Target {:?} not connected to player territory, attempting ship launch", tile_coord);

    // Find target's nearest coastal tile
    let target_coastal_tile = crate::game::connectivity::find_coastal_tile_in_region(territory_manager.as_slice(), terrain, tile_coord, size);

    let Some(target_coastal_tile) = target_coastal_tile else {
        debug!("No coastal tile found in target's region for tile {:?}", tile_coord);
        return;
    };

    // Find player's nearest coastal tile using O(1) entity lookup
    let player_border_tiles = entity_map.0.get(&nation_id).and_then(|&entity| border_query.get(entity).ok());

    let launch_tile = player_border_tiles.and_then(|tiles| crate::game::ships::pathfinding::find_nearest_player_coastal_tile(coastal_tiles.tiles(), tiles, target_coastal_tile));

    let Some(launch_tile) = launch_tile else {
        debug!("Player has no coastal tiles to launch ship from");
        return;
    };

    debug!("Found launch tile {:?} and target coastal tile {:?} for target {:?}", launch_tile, target_coastal_tile, tile_coord);

    // Try to find a water path from launch tile to target coastal tile
    let path = crate::game::ships::pathfinding::find_water_path(terrain, launch_tile, target_coastal_tile, crate::game::ships::MAX_PATH_LENGTH);

    if let Some(_path) = path {
        // We can reach the target by ship!
        // Calculate absolute troop count from ratio
        let troops = if let Some(&entity) = entity_map.0.get(&nation_id)
            && let Ok(troops_comp) = troops_query.get(entity)
        {
            (troops_comp.0 * attack_controls.attack_ratio).floor() as u32
        } else {
            0
        };

        debug!("Launching ship to target {:?} with {} troops", tile_coord, troops);

        intent_writer.write(IntentEvent(Intent::Action(GameAction::LaunchShip { target_tile: tile_coord, troops })));
    } else {
        debug!("No water path found from {:?} to {:?}", launch_tile, target_coastal_tile);
    }
}

/// Handle camera actions
pub fn handle_camera_action_system(mut camera_actions: MessageReader<CameraAction>, border_cache: Res<BorderCache>, local_context: If<Res<LocalPlayerContext>>) {
    for action in camera_actions.read() {
        match action {
            CameraAction::Center => {
                // Find any owned tile to center on
                if let Some(_tile) = border_cache.get(local_context.id).and_then(|tiles| tiles.iter().next().copied()) {
                    // TODO: Implement camera centering when camera commands are added
                    tracing::debug!("Camera center requested (not implemented)");
                }
            }
            CameraAction::InteractionStarted => {
                trace!("Camera interaction started");
            }
            CameraAction::InteractionEnded => {
                trace!("Camera interaction ended");
            }
        }
    }
}

/// Handle UI actions
pub fn handle_ui_action_system(mut ui_actions: MessageReader<UiAction>, mut attack_controls: ResMut<AttackControls>) {
    for action in ui_actions.read() {
        match action {
            UiAction::UpdateAttackRatio { amount } => {
                attack_controls.attack_ratio = (attack_controls.attack_ratio + amount).clamp(ATTACK_RATIO_MIN, ATTACK_RATIO_MAX);
                debug!("Attack ratio changed to {:.1}", attack_controls.attack_ratio);
            }
            UiAction::TogglePause => {
                // TODO: Implement pause functionality
                debug!("Pause toggle requested (not implemented)");
            }
        }
    }
}
