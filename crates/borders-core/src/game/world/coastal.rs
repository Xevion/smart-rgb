use std::collections::HashSet;

use bevy_ecs::prelude::*;
use glam::U16Vec2;

use crate::game::core::utils::neighbors;
use crate::game::terrain::TerrainData;

/// Resource containing precomputed coastal tile positions
///
/// A coastal tile is defined as a land tile (not water) that is adjacent
/// to at least one water tile in 4-directional connectivity.
///
/// This is computed once during game initialization and never changes,
/// providing O(1) lookups for systems that need to check if a tile is coastal.
#[derive(Resource)]
pub struct CoastalTiles {
    tiles: HashSet<U16Vec2>,
}

impl CoastalTiles {
    /// Compute all coastal tile positions from terrain data
    ///
    /// This scans the entire map once to find all land tiles adjacent to water.
    /// The result is cached in a HashSet for fast lookups.
    pub fn compute(terrain: &TerrainData, size: U16Vec2) -> Self {
        let mut coastal_tiles = HashSet::new();
        let width = size.x as usize;
        let height = size.y as usize;

        for y in 0..height {
            for x in 0..width {
                let tile_pos = U16Vec2::new(x as u16, y as u16);

                // Skip water tiles
                if terrain.is_navigable(tile_pos) {
                    continue;
                }

                // Check if any neighbor is water using the neighbors utility
                if neighbors(tile_pos, size).any(|neighbor| terrain.is_navigable(neighbor)) {
                    coastal_tiles.insert(tile_pos);
                }
            }
        }

        Self { tiles: coastal_tiles }
    }

    /// Check if a tile is coastal
    #[inline]
    pub fn contains(&self, tile: U16Vec2) -> bool {
        self.tiles.contains(&tile)
    }

    /// Get a reference to the set of all coastal tiles
    #[inline]
    pub fn tiles(&self) -> &HashSet<U16Vec2> {
        &self.tiles
    }

    /// Get the number of coastal tiles
    #[inline]
    pub fn len(&self) -> usize {
        self.tiles.len()
    }

    /// Check if there are no coastal tiles
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tiles.is_empty()
    }
}
