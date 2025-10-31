//! Spawn territory claiming logic
//!
//! Provides utilities for claiming 5x5 territories around spawn points.

use glam::U16Vec2;
use std::collections::HashSet;

use crate::game::terrain::data::TerrainData;
use crate::game::world::{NationId, TileOwnership};

/// Claims a 5x5 territory around a spawn point
///
/// Claims all unclaimed, conquerable tiles within 2 tiles of the spawn center
/// and returns the set of tiles that were successfully claimed.
#[inline]
pub fn claim_spawn_territory(spawn_center: U16Vec2, nation: NationId, territories: &mut [TileOwnership], terrain: &TerrainData, map_size: U16Vec2) -> HashSet<U16Vec2> {
    let width = map_size.x as usize;

    (-2..=2)
        .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
        .filter_map(|(dx, dy)| {
            let x = (spawn_center.x as i32 + dx).clamp(0, map_size.x as i32 - 1) as usize;
            let y = (spawn_center.y as i32 + dy).clamp(0, map_size.y as i32 - 1) as usize;
            let tile_pos = U16Vec2::new(x as u16, y as u16);
            let idx = y * width + x;

            if territories[idx].is_unclaimed() && terrain.is_conquerable(tile_pos) {
                territories[idx] = TileOwnership::Owned(nation);
                Some(tile_pos)
            } else {
                None
            }
        })
        .collect()
}

/// Clears spawn territory for a specific nation within a 5x5 area
///
/// Reverts all tiles owned by the given nation within the 5x5 area back to unclaimed
/// and returns the set of tiles that were cleared.
#[inline]
pub fn clear_spawn_territory(spawn_center: U16Vec2, nation: NationId, territories: &mut [TileOwnership], map_size: U16Vec2) -> HashSet<U16Vec2> {
    let width = map_size.x as usize;

    (-2..=2)
        .flat_map(|dy| (-2..=2).map(move |dx| (dx, dy)))
        .filter_map(|(dx, dy)| {
            let x = (spawn_center.x as i32 + dx).clamp(0, map_size.x as i32 - 1) as usize;
            let y = (spawn_center.y as i32 + dy).clamp(0, map_size.y as i32 - 1) as usize;
            let tile_pos = U16Vec2::new(x as u16, y as u16);
            let idx = y * width + x;

            if territories[idx].is_owned_by(nation) {
                territories[idx] = TileOwnership::Unclaimed;
                Some(tile_pos)
            } else {
                None
            }
        })
        .collect()
}
