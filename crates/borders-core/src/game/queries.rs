//! ECS query helper utilities for common game state queries

use glam::U16Vec2;

use crate::game::systems::borders::BorderCache;
use crate::game::{NationId, TerritoryManager};

/// Find any tile owned by a specific nation (useful for camera centering)
/// Returns the tile position if found
#[inline]
pub fn find_nation_tile(border_cache: &BorderCache, nation_id: NationId) -> Option<U16Vec2> {
    border_cache.get(nation_id)?.iter().next().copied()
}

/// Count the total number of conquerable (non-water, owned) tiles on the map
#[inline]
pub fn count_land_tiles(territory_manager: &TerritoryManager) -> u32 {
    territory_manager.as_slice().iter().filter(|ownership| ownership.is_owned()).count() as u32
}
