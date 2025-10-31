use bevy_ecs::prelude::*;
use glam::U16Vec2;

use super::changes::ChangeBuffer;
use super::tilemap::TileMap;
use super::{NationId, TileOwnership};
use crate::game::utils::neighbors;

/// Manages territory ownership for all tiles
#[derive(Resource)]
pub struct TerritoryManager {
    tile_owners: TileMap<TileOwnership>,
    changes: ChangeBuffer,
    /// Cached u16 representation for efficient serialization to frontend
    u16_cache: Vec<u16>,
    cache_dirty: bool,
}

impl TerritoryManager {
    /// Creates a new territory manager
    pub fn new(map_size: U16Vec2) -> Self {
        let size = (map_size.x as usize) * (map_size.y as usize);
        Self { tile_owners: TileMap::with_default(map_size, TileOwnership::Unclaimed), changes: ChangeBuffer::with_capacity(size / 100), u16_cache: vec![0; size], cache_dirty: true }
    }

    /// Resets the territory manager
    pub fn reset(&mut self, map_size: U16Vec2, _conquerable_tiles: &[bool]) {
        self.tile_owners = TileMap::with_default(map_size, TileOwnership::Unclaimed);
        self.changes.clear();

        let size = (map_size.x as usize) * (map_size.y as usize);
        self.u16_cache.resize(size, 0);
        self.cache_dirty = true;
    }

    /// Checks if a tile is a border tile of the territory of its owner
    /// A tile is a border tile if it is adjacent to a tile that is not owned by the same player
    pub fn is_border(&self, tile: U16Vec2) -> bool {
        let owner = self.tile_owners[tile];

        // Border if on map edge
        if tile.x == 0 || tile.x == self.tile_owners.width() - 1 || tile.y == 0 || tile.y == self.tile_owners.height() - 1 {
            return true;
        }

        // Border if any neighbor has different owner
        for neighbor_pos in self.tile_owners.neighbors(tile) {
            if self.tile_owners[neighbor_pos] != owner {
                return true;
            }
        }

        false
    }

    /// Checks if a tile has an owner
    pub fn has_owner(&self, tile: U16Vec2) -> bool {
        self.tile_owners[tile].is_owned()
    }

    /// Checks if a tile is owned by a specific nation
    pub fn is_owner(&self, tile: U16Vec2, owner: NationId) -> bool {
        self.tile_owners[tile].is_owned_by(owner)
    }

    /// Gets the nation ID of the tile owner, if any
    pub fn get_nation_id(&self, tile: U16Vec2) -> Option<NationId> {
        self.tile_owners[tile].nation_id()
    }

    /// Gets the ownership enum for a tile
    pub fn get_ownership(&self, tile: U16Vec2) -> TileOwnership {
        self.tile_owners[tile]
    }

    /// Conquers a tile for a nation
    /// Returns the previous owner, if any
    pub fn conquer(&mut self, tile: U16Vec2, owner: NationId) -> Option<NationId> {
        let previous_owner = self.tile_owners[tile];
        let new_ownership = TileOwnership::Owned(owner);

        if previous_owner != new_ownership {
            self.tile_owners[tile] = new_ownership;
            self.changes.push(tile);
            self.cache_dirty = true;
        }

        previous_owner.nation_id()
    }

    /// Clears a tile (removes ownership)
    pub fn clear(&mut self, tile: U16Vec2) -> Option<NationId> {
        let ownership = self.tile_owners[tile];
        if ownership.is_owned() {
            self.tile_owners[tile] = TileOwnership::Unclaimed;
            self.changes.push(tile);
            self.cache_dirty = true;
            ownership.nation_id()
        } else {
            None
        }
    }

    /// Get the size of the map as U16Vec2
    #[inline]
    pub fn size(&self) -> U16Vec2 {
        self.tile_owners.size()
    }

    /// Get width of the map
    #[inline]
    pub fn width(&self) -> u16 {
        self.tile_owners.width()
    }

    /// Get height of the map
    #[inline]
    pub fn height(&self) -> u16 {
        self.tile_owners.height()
    }

    /// Returns a reference to the underlying tile ownership data as a slice of enums
    #[inline]
    pub fn as_slice(&self) -> &[TileOwnership] {
        self.tile_owners.as_slice()
    }

    /// Returns the tile ownership data as u16 values for frontend serialization
    /// This is cached and only recomputed when ownership changes
    pub fn as_u16_slice(&mut self) -> &[u16] {
        if self.cache_dirty {
            let tile_count = self.tile_owners.len();
            let _guard = tracing::trace_span!("rebuild_u16_cache", tile_count).entered();

            for (i, ownership) in self.tile_owners.as_slice().iter().enumerate() {
                self.u16_cache[i] = (*ownership).into();
            }
            self.cache_dirty = false;
        }
        &self.u16_cache
    }

    /// Returns the number of tiles in the map
    #[inline]
    pub fn len(&self) -> usize {
        self.tile_owners.len()
    }

    /// Returns true if the map has no tiles
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.tile_owners.len() == 0
    }

    /// Returns an iterator over changed tile positions without consuming them
    /// Use this to read changes without clearing the buffer
    #[inline]
    pub fn iter_changes(&self) -> impl Iterator<Item = U16Vec2> + '_ {
        self.changes.iter()
    }

    /// Drains all changed tile positions, returning an iterator and clearing the change buffer
    #[inline]
    pub fn drain_changes(&mut self) -> impl Iterator<Item = U16Vec2> + '_ {
        self.changes.drain()
    }

    /// Returns true if any territory changes have been recorded since last drain
    #[inline]
    pub fn has_changes(&self) -> bool {
        self.changes.has_changes()
    }

    /// Clears all tracked changes without returning them
    #[inline]
    pub fn clear_changes(&mut self) {
        self.changes.clear()
    }

    /// Calls a closure for each neighbor using tile indices (legacy compatibility)
    #[inline]
    pub fn on_neighbor_indices<F>(&self, index: u32, closure: F)
    where
        F: FnMut(u32),
    {
        self.tile_owners.on_neighbor_indices(index, closure)
    }

    /// Checks if any neighbor has a different owner than the specified owner
    pub fn any_neighbor_has_different_owner(&self, tile: U16Vec2, owner: NationId) -> bool {
        let owner_enum = TileOwnership::Owned(owner);
        neighbors(tile, self.size()).any(|neighbor| self.tile_owners[neighbor] != owner_enum)
    }

    /// Converts position to flat u32 index for JavaScript/IPC boundary
    #[inline]
    pub fn pos_to_index<P: Into<U16Vec2>>(&self, pos: P) -> u32 {
        self.tile_owners.pos_to_index(pos)
    }
}
