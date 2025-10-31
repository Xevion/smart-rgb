#![allow(dead_code)]
#![allow(unused_imports)]

/// Shared test utilities and helpers
///
/// This module provides infrastructure for testing the Bevy ECS-based game logic:
/// - `builders`: Fluent API for constructing test maps
/// - `assertions`: Fluent assertion API for ECS state verification
/// - `fixtures`: Pre-built test scenarios and GameBuilder extensions
///
/// # Usage Example
/// ```
/// use common::{GameBuilderTestExt, GameTestExt, GameAssertExt};
///
/// let mut game = GameBuilder::with_map_size(50, 50)
///     .with_nation(NationId::ZERO, 100.0)
///     .with_network(NetworkMode::Local)
///     .with_systems(false)
///     .build();
///
/// game.conquer_tile(U16Vec2::new(5, 5), NationId::ZERO);
/// game.assert().player_owns(U16Vec2::new(5, 5), NationId::ZERO);
/// ```
use borders_core::game::Game;
use borders_core::prelude::*;
use extension_traits::extension;
use std::collections::HashMap;
use std::collections::HashSet;
use std::ops::Mul;

mod assertions;
mod builders;
mod fixtures;

// Re-export commonly used items
pub use assertions::*;
pub use builders::*;
pub use fixtures::*;

/// Internal extension trait providing action methods for World
///
/// This trait is used internally by GameTestExt. Tests should use GameTestExt instead.
#[extension(pub(crate) trait WorldTestExt)]
impl World {
    /// Conquer a tile for a player
    fn conquer_tile(&mut self, tile: U16Vec2, player: NationId) {
        self.resource_mut::<TerritoryManager>().conquer(tile, player);
    }

    /// Conquer multiple tiles for a player
    fn conquer_tiles(&mut self, tiles: &[U16Vec2], player: NationId) {
        let mut territory_manager = self.resource_mut::<TerritoryManager>();
        for &tile in tiles {
            territory_manager.conquer(tile, player);
        }
    }

    /// Conquer a square region of tiles centered at a position
    fn conquer_region(&mut self, center: U16Vec2, radius: u32, player: NationId) {
        let mut territory_manager = self.resource_mut::<TerritoryManager>();
        let range = -1.mul(radius as i32)..=(radius as i32);
        for dy in range.clone() {
            for dx in range.clone() {
                let tile = center.as_ivec2() + glam::IVec2::new(dx, dy);
                if tile.x >= 0 && tile.y >= 0 {
                    let tile = tile.as_u16vec2();
                    if tile.x < territory_manager.width() && tile.y < territory_manager.height() {
                        territory_manager.conquer(tile, player);
                    }
                }
            }
        }
    }

    /// Conquer all 4-directional neighbors of a tile
    fn conquer_neighbors(&mut self, center: U16Vec2, player: NationId) {
        let map_size = {
            let mgr = self.resource::<TerritoryManager>();
            U16Vec2::new(mgr.width(), mgr.height())
        };

        let mut territory_manager = self.resource_mut::<TerritoryManager>();
        for neighbor in neighbors(center, map_size) {
            territory_manager.conquer(neighbor, player);
        }
    }

    /// Clear ownership of a tile
    fn clear_tile(&mut self, tile: U16Vec2) -> Option<NationId> {
        self.resource_mut::<TerritoryManager>().clear(tile)
    }

    /// Clear all territory changes
    fn clear_territory_changes(&mut self) {
        self.resource_mut::<TerritoryManager>().clear_changes();
    }

    /// Deactivate the spawn phase
    fn deactivate_spawn_phase(&mut self) {
        self.resource_mut::<SpawnPhase>().active = false;
    }

    /// Activate the spawn phase
    fn activate_spawn_phase(&mut self) {
        self.resource_mut::<SpawnPhase>().active = true;
    }

    /// Get the number of territory changes
    fn get_change_count(&self) -> usize {
        self.resource::<TerritoryManager>().iter_changes().count()
    }

    /// Get the entity for a nation
    fn get_nation_entity(&self, nation_id: NationId) -> Entity {
        let entity_map = self.resource::<NationEntityMap>();
        entity_map.get_entity(nation_id)
    }

    /// Run the border update logic
    fn update_borders(&mut self) {
        if !self.resource::<TerritoryManager>().has_changes() {
            return;
        }

        let (changed_tiles, map_size, tiles_by_owner) = {
            let territory_manager = self.resource::<TerritoryManager>();
            let changed_tiles: HashSet<U16Vec2> = territory_manager.iter_changes().collect();
            let map_size = U16Vec2::new(territory_manager.width(), territory_manager.height());

            let mut affected_tiles = HashSet::with_capacity(changed_tiles.len() * 5);
            for &tile in &changed_tiles {
                affected_tiles.insert(tile);
                affected_tiles.extend(neighbors(tile, map_size));
            }

            let mut tiles_by_owner: HashMap<NationId, HashSet<U16Vec2>> = HashMap::new();
            for &tile in &affected_tiles {
                if let Some(nation_id) = territory_manager.get_nation_id(tile) {
                    tiles_by_owner.entry(nation_id).or_default().insert(tile);
                }
            }

            (changed_tiles, map_size, tiles_by_owner)
        };

        let ownership_snapshot: HashMap<U16Vec2, Option<NationId>> = {
            let territory_manager = self.resource::<TerritoryManager>();
            let mut snapshot = HashMap::new();
            for &tile in changed_tiles.iter() {
                for neighbor in neighbors(tile, map_size) {
                    snapshot.entry(neighbor).or_insert_with(|| territory_manager.get_nation_id(neighbor));
                }
                snapshot.insert(tile, territory_manager.get_nation_id(tile));
            }
            snapshot
        };

        let mut players_query = self.query::<(&NationId, &mut BorderTiles)>();
        for (nation_id, mut component_borders) in players_query.iter_mut(self) {
            let empty_set = HashSet::new();
            let player_tiles = tiles_by_owner.get(nation_id).unwrap_or(&empty_set);

            for &tile in player_tiles {
                let is_border = neighbors(tile, map_size).any(|neighbor| ownership_snapshot.get(&neighbor).and_then(|&owner| owner) != Some(*nation_id));

                if is_border {
                    component_borders.0.insert(tile);
                } else {
                    component_borders.0.remove(&tile);
                }
            }

            for &tile in changed_tiles.iter() {
                if ownership_snapshot.get(&tile).and_then(|&owner| owner) != Some(*nation_id) {
                    component_borders.0.remove(&tile);
                }
            }
        }
    }

    /// Clear territory changes
    fn clear_borders_changes(&mut self) {
        self.resource_mut::<TerritoryManager>().clear_changes();
    }

    /// Get border tiles for a nation from ECS
    fn get_nation_borders(&self, nation_id: NationId) -> HashSet<U16Vec2> {
        let entity = self.get_nation_entity(nation_id);
        self.get::<BorderTiles>(entity).expect("Nation entity missing BorderTiles component").0.clone()
    }

    /// Get border tiles from BorderCache
    fn get_border_cache(&self, nation_id: NationId) -> Option<HashSet<U16Vec2>> {
        self.resource::<BorderCache>().get(nation_id).cloned()
    }
}

/// Extension trait providing convenient action methods for Game in tests
///
/// Provides test helpers that forward to World extension methods.
#[extension(pub trait GameTestExt)]
impl Game {
    /// Manually spawn a nation entity with components (for tests that need specific nation IDs)
    ///
    /// Use this when GameBuilder doesn't create the nations you need.
    fn spawn_test_nation(&mut self, id: NationId, troops: f32) {
        let entity = self.world_mut().spawn((id, borders_core::game::NationName(format!("Player {}", id.get())), borders_core::game::NationColor(borders_core::game::entities::HSLColor::new((id.get() as f32 * 137.5) % 360.0, 0.6, 0.5)), BorderTiles::default(), borders_core::game::Troops(troops), borders_core::game::TerritorySize(0), borders_core::game::ships::ShipCount::default())).id();

        self.world_mut().resource_mut::<NationEntityMap>().0.insert(id, entity);
    }

    /// Conquer a tile for a nation
    fn conquer_tile(&mut self, tile: U16Vec2, nation: NationId) {
        self.world_mut().conquer_tile(tile, nation);
    }

    /// Conquer multiple tiles for a nation
    fn conquer_tiles(&mut self, tiles: &[U16Vec2], nation: NationId) {
        self.world_mut().conquer_tiles(tiles, nation);
    }

    /// Conquer a square region of tiles centered at a position
    ///
    /// Conquers all tiles within `radius` steps of `center` (using taxicab distance).
    /// For radius=1, conquers a 3x3 square. For radius=2, conquers a 5x5 square, etc.
    fn conquer_region(&mut self, center: U16Vec2, radius: u32, nation: NationId) {
        self.world_mut().conquer_region(center, radius, nation);
    }

    /// Conquer all 4-directional neighbors of a tile
    fn conquer_neighbors(&mut self, center: U16Vec2, nation: NationId) {
        self.world_mut().conquer_neighbors(center, nation);
    }

    /// Clear ownership of a tile, returning the previous owner
    fn clear_tile(&mut self, tile: U16Vec2) -> Option<NationId> {
        self.world_mut().clear_tile(tile)
    }

    /// Clear all territory changes from the change buffer
    fn clear_territory_changes(&mut self) {
        self.world_mut().clear_territory_changes();
    }

    /// Deactivate the spawn phase
    fn deactivate_spawn_phase(&mut self) {
        self.world_mut().deactivate_spawn_phase();
    }

    /// Activate the spawn phase
    fn activate_spawn_phase(&mut self) {
        self.world_mut().activate_spawn_phase();
    }

    /// Get the number of territory changes in the ChangeBuffer
    fn get_change_count(&self) -> usize {
        self.world().get_change_count()
    }

    /// Get the entity associated with a nation
    fn get_nation_entity(&self, nation_id: NationId) -> Entity {
        self.world().get_nation_entity(nation_id)
    }

    /// Run the border update logic (inline implementation for testing)
    fn update_borders(&mut self) {
        self.world_mut().update_borders();
    }

    /// Clear territory changes
    fn clear_borders_changes(&mut self) {
        self.world_mut().clear_borders_changes();
    }

    /// Get border tiles for a nation from ECS component
    fn get_nation_borders(&self, nation_id: NationId) -> HashSet<U16Vec2> {
        self.world().get_nation_borders(nation_id)
    }

    /// Get border tiles for a nation from BorderCache
    fn get_border_cache(&self, nation_id: NationId) -> Option<HashSet<U16Vec2>> {
        self.world().get_border_cache(nation_id)
    }
}
