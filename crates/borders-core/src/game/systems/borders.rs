/// Border tile management
///
/// This module manages border tiles for all nations. A border tile is a tile
/// adjacent to a tile with a different owner. Borders are used for:
/// - Attack targeting (attacks expand from border tiles)
/// - UI rendering (show nation borders on the map)
/// - Ship launching (find coastal borders for naval operations)
use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use glam::U16Vec2;

use crate::game::{
    CurrentTurn,
    entities::BorderTiles,
    utils::neighbors,
    world::{NationId, TerritoryManager},
};

/// Cached border data for efficient non-ECS lookups
///
/// This resource caches border tiles per nation to avoid reconstructing
/// HashMaps every turn. It is updated by `update_player_borders_system`
/// only when borders actually change.
#[derive(Resource, Default)]
pub struct BorderCache {
    borders: HashMap<NationId, HashSet<U16Vec2>>,
}

impl BorderCache {
    /// Get border tiles for a specific nation
    #[inline]
    pub fn get(&self, nation_id: NationId) -> Option<&HashSet<U16Vec2>> {
        self.borders.get(&nation_id)
    }

    /// Update the border cache with current border data
    fn update(&mut self, nation_id: NationId, borders: &HashSet<U16Vec2>) {
        self.borders.insert(nation_id, borders.clone());
    }

    /// Get all nation borders as a HashMap (for compatibility)
    pub fn as_map(&self) -> HashMap<NationId, &HashSet<U16Vec2>> {
        self.borders.iter().map(|(id, borders)| (*id, borders)).collect()
    }
}

/// Result of a border transition
#[derive(Debug)]
pub struct BorderTransitionResult {
    /// Tiles that became interior (not borders anymore)
    pub territory: Vec<U16Vec2>,
    /// Tiles that are now attacker borders
    pub attacker: Vec<U16Vec2>,
    /// Tiles that are now defender borders
    pub defender: Vec<U16Vec2>,
}

/// Group affected tiles by their owner for efficient per-nation processing
///
/// Instead of checking every tile for every nation (O(nations * tiles)),
/// we group tiles by owner once (O(tiles)) and then process each group.
fn group_tiles_by_owner(affected_tiles: &HashSet<U16Vec2>, territory: &TerritoryManager) -> HashMap<NationId, HashSet<U16Vec2>> {
    let _guard = tracing::trace_span!("group_tiles_by_owner", tile_count = affected_tiles.len()).entered();

    let mut grouped: HashMap<NationId, HashSet<U16Vec2>> = HashMap::new();
    for &tile in affected_tiles {
        if let Some(nation_id) = territory.get_ownership(tile).nation_id() {
            grouped.entry(nation_id).or_default().insert(tile);
        }
    }

    grouped
}

/// System to clear territory changes
pub fn clear_territory_changes_system(current_turn: Res<CurrentTurn>, mut territory_manager: ResMut<TerritoryManager>) {
    if current_turn.active && territory_manager.has_changes() {
        tracing::trace!(count = territory_manager.iter_changes().count(), "Clearing territory changes");
        territory_manager.clear_changes();
    }
}

/// Update all nation borders based on territory changes (batched system)
///
/// This system runs once per turn AFTER all territory changes (conquests, spawns, ships).
/// It drains the TerritoryManager's change buffer and updates borders for all affected nations.
/// It also updates the BorderCache for efficient non-ECS lookups.
pub fn update_nation_borders_system(mut nations: Query<(&NationId, &mut BorderTiles)>, territory_manager: Res<TerritoryManager>, mut border_cache: ResMut<BorderCache>) {
    if !territory_manager.has_changes() {
        return; // Early exit - no work needed
    }

    let _guard = tracing::trace_span!("update_player_borders").entered();

    let (changed_tiles, raw_change_count): (HashSet<U16Vec2>, usize) = {
        let _guard = tracing::trace_span!("collect_changed_tiles").entered();
        let changes_vec: Vec<U16Vec2> = territory_manager.iter_changes().collect();
        let raw_count = changes_vec.len();
        let unique_set: HashSet<U16Vec2> = changes_vec.into_iter().collect();
        (unique_set, raw_count)
    };

    if raw_change_count != changed_tiles.len() {
        tracing::warn!(raw_changes = raw_change_count, unique_changes = changed_tiles.len(), duplicates = raw_change_count - changed_tiles.len(), "Duplicate tile changes detected in ChangeBuffer - this causes performance degradation");
    }

    // Build affected tiles (changed + all neighbors)
    let affected_tiles = {
        let _guard = tracing::trace_span!("build_affected_tiles", changed_count = changed_tiles.len()).entered();

        let mut affected_tiles = HashSet::with_capacity(changed_tiles.len() * 5);
        let size = territory_manager.size();
        for &tile in &changed_tiles {
            affected_tiles.insert(tile);
            affected_tiles.extend(crate::game::core::utils::neighbors(tile, size));
        }
        affected_tiles
    };

    // Group tiles by owner for efficient per-nation processing
    let tiles_by_owner = group_tiles_by_owner(&affected_tiles, &territory_manager);

    tracing::trace!(nation_count = nations.iter().len(), changed_tile_count = changed_tiles.len(), affected_tile_count = affected_tiles.len(), unique_owners = tiles_by_owner.len(), "Border update statistics");

    // Update each nation's borders (pure ECS) and BorderCache
    {
        let _guard = tracing::trace_span!("update_all_nation_borders", nation_count = nations.iter().len()).entered();

        for (nation_id, mut component_borders) in &mut nations {
            // Only process tiles owned by this nation (or empty set if none)
            let empty_set = HashSet::new();
            let nation_tiles = tiles_by_owner.get(nation_id).unwrap_or(&empty_set);

            update_borders_for_player(&mut component_borders, *nation_id, nation_tiles, &territory_manager);

            // Update the cache with the new border data
            border_cache.update(*nation_id, &component_borders);
        }
    }
}

/// Update borders for a single nation based on their owned tiles
///
/// Only processes tiles owned by this nation, significantly reducing
/// redundant work when multiple nations exist.
fn update_borders_for_player(borders: &mut HashSet<U16Vec2>, nation_id: NationId, nation_tiles: &HashSet<U16Vec2>, territory: &TerritoryManager) {
    let _guard = tracing::trace_span!(
        "update_borders_for_nation",
        nation_id = %nation_id,
        nation_tile_count = nation_tiles.len(),
        current_border_count = borders.len()
    )
    .entered();

    for &tile in nation_tiles {
        // Check if it's a border (has at least one neighbor with different owner)
        let is_border = neighbors(tile, territory.size()).any(|neighbor| !territory.is_owner(neighbor, nation_id));

        if is_border {
            borders.insert(tile);
        } else {
            borders.remove(&tile);
        }
    }
}
