//! Fluent assertion API for testing Bevy ECS Game state
//!
//! Provides chainable assertion methods that test behavior through public interfaces.
//!
//! Tests should use GameAssertExt on Game instances. WorldAssertExt is internal.
use assert2::assert;
use borders_core::game::Game;
use extension_traits::extension;
use std::collections::HashSet;

use borders_core::prelude::*;

/// Internal fluent assertion builder for World state
///
/// Tests should use GameAssertions instead via `game.assert()`.
pub(crate) struct WorldAssertions<'w> {
    world: &'w World,
}

/// Internal extension trait to add assertion capabilities to World
///
/// Tests should use GameAssertExt instead. This is used internally by GameAssertions.
#[extension(pub(crate) trait WorldAssertExt)]
impl World {
    /// Begin a fluent assertion chain
    fn assert(&self) -> WorldAssertions<'_> {
        WorldAssertions { world: self }
    }
}

impl<'w> WorldAssertions<'w> {
    /// Assert that a nation owns a specific tile
    #[track_caller]
    pub fn player_owns(self, tile: U16Vec2, expected: NationId) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        let actual = mgr.get_nation_id(tile);
        assert!(actual == Some(expected), "Expected nation {} to own tile {:?}, but found {:?}", expected.get(), tile, actual.map(|id| id.get()));
        self
    }

    /// Assert that a tile is unclaimed
    #[track_caller]
    pub fn tile_unclaimed(self, tile: U16Vec2) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        let ownership = mgr.get_ownership(tile);
        assert!(ownership.is_unclaimed(), "Expected tile {:?} to be unclaimed, but it's owned by {:?}", tile, ownership.nation_id());
        self
    }

    /// Assert that an attack exists between attacker and target
    #[track_caller]
    pub fn attack_exists(self, attacker: NationId, target: Option<NationId>) -> Self {
        let attacks = self.world.resource::<ActiveAttacks>();
        let nation_attacks = attacks.get_attacks_for_nation(attacker);

        let exists = nation_attacks.iter().any(|(att, tgt, _, _, is_outgoing)| *att == attacker && *tgt == target && *is_outgoing);

        assert!(exists, "Expected attack from nation {} to {:?}, but none found. Active attacks: {:?}", attacker.get(), target.map(|t| t.get()), nation_attacks);
        self
    }

    /// Assert that no attack exists between attacker and target
    #[track_caller]
    pub fn no_attack(self, attacker: NationId, target: Option<NationId>) -> Self {
        let attacks = self.world.resource::<ActiveAttacks>();
        let nation_attacks = attacks.get_attacks_for_nation(attacker);

        let exists = nation_attacks.iter().any(|(att, tgt, _, _, is_outgoing)| *att == attacker && *tgt == target && *is_outgoing);

        assert!(!exists, "Expected NO attack from nation {} to {:?}, but found one. Active attacks: {:?}", attacker.get(), target.map(|t| t.get()), nation_attacks);
        self
    }

    /// Assert that a nation has specific border tiles
    #[track_caller]
    pub fn border_tiles(self, nation_id: NationId, expected: &HashSet<U16Vec2>) -> Self {
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();
        let entity = entity_map.get_entity(nation_id);

        let border_tiles = self.world.get::<BorderTiles>(entity).expect("BorderTiles component not found");

        assert!(&border_tiles.0 == expected, "Border tiles mismatch for nation {}.\nExpected: {:?}\nActual: {:?}", nation_id.get(), expected, border_tiles.0);
        self
    }

    /// Assert that TerritoryManager has recorded changes
    #[track_caller]
    pub fn has_territory_changes(self) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        assert!(mgr.has_changes(), "Expected territory changes to be tracked, but ChangeBuffer is empty");
        self
    }

    /// Assert that TerritoryManager has NO recorded changes
    #[track_caller]
    pub fn no_territory_changes(self) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        assert!(!mgr.has_changes(), "Expected no territory changes, but ChangeBuffer contains: {:?}", mgr.iter_changes().collect::<Vec<_>>());
        self
    }

    /// Assert that a resource exists in the world
    #[track_caller]
    pub fn resource_exists<T: Resource>(self, resource_name: &str) -> Self {
        assert!(self.world.contains_resource::<T>(), "Expected resource '{}' to exist, but it was not found", resource_name);
        self
    }

    /// Assert that a resource does NOT exist in the world
    #[track_caller]
    pub fn resource_missing<T: Resource>(self, resource_name: &str) -> Self {
        assert!(!self.world.contains_resource::<T>(), "Expected resource '{}' to be missing, but it exists", resource_name);
        self
    }

    /// Assert that no invalid game state exists
    ///
    /// Checks common invariants:
    /// - No self-attacks in ActiveAttacks
    /// - All nation entities have required components
    /// - Territory ownership is consistent
    #[track_caller]
    pub fn no_invalid_state(self) -> Self {
        let attacks = self.world.resource::<ActiveAttacks>();
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();

        for &nation_id in entity_map.0.keys() {
            let nation_attacks = attacks.get_attacks_for_nation(nation_id);
            for (attacker, target, _, _, _) in nation_attacks {
                if let Some(target_id) = target {
                    assert!(attacker != target_id, "INVARIANT VIOLATION: Nation {} is attacking itself", attacker.get());
                }
            }
        }

        for (&nation_id, &entity) in &entity_map.0 {
            assert!(self.world.get::<borders_core::game::Troops>(entity).is_some(), "Nation {} entity missing Troops component", nation_id.get());
            assert!(self.world.get::<BorderTiles>(entity).is_some(), "Nation {} entity missing BorderTiles component", nation_id.get());
        }

        self
    }

    /// Assert that a nation has a specific troop count
    #[track_caller]
    pub fn player_troops(self, nation_id: NationId, expected: f32) -> Self {
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();
        let entity = entity_map.get_entity(nation_id);

        let troops = self.world.get::<borders_core::game::Troops>(entity).expect("Troops component not found");

        let difference = (troops.0 - expected).abs();
        assert!(difference < 0.01, "Expected nation {} to have {} troops, but found {} (difference: {})", nation_id.get(), expected, troops.0, difference);
        self
    }

    /// Assert that a tile is a border tile
    #[allow(clippy::wrong_self_convention)]
    #[track_caller]
    pub fn is_border(self, tile: U16Vec2) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        assert!(mgr.is_border(tile), "Expected tile {:?} to be a border tile, but it is not", tile);
        self
    }

    /// Assert that a tile is NOT a border tile
    #[track_caller]
    pub fn not_border(self, tile: U16Vec2) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        assert!(!mgr.is_border(tile), "Expected tile {:?} to NOT be a border tile, but it is", tile);
        self
    }

    /// Assert that a nation owns all tiles in a slice
    #[track_caller]
    pub fn player_owns_all(self, tiles: &[U16Vec2], expected: NationId) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        for &tile in tiles {
            let actual = mgr.get_nation_id(tile);
            assert!(actual == Some(expected), "Expected nation {} to own tile {:?}, but found {:?}", expected.get(), tile, actual.map(|id| id.get()));
        }
        self
    }

    /// Assert that the ChangeBuffer contains exactly N changes
    #[track_caller]
    pub fn change_count(self, expected: usize) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        let actual = mgr.iter_changes().count();
        assert!(actual == expected, "Expected {} changes in ChangeBuffer, but found {}", expected, actual);
        self
    }

    /// Assert that a nation entity has all standard game components
    ///
    /// Checks for: NationName, NationColor, BorderTiles, Troops, TerritorySize
    #[track_caller]
    pub fn player_has_components(self, nation_id: NationId) -> Self {
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();
        let entity = entity_map.get_entity(nation_id);

        assert!(self.world.get::<borders_core::game::NationName>(entity).is_some(), "Nation {} missing NationName component", nation_id.get());
        assert!(self.world.get::<borders_core::game::NationColor>(entity).is_some(), "Nation {} missing NationColor component", nation_id.get());
        assert!(self.world.get::<BorderTiles>(entity).is_some(), "Nation {} missing BorderTiles component", nation_id.get());
        assert!(self.world.get::<borders_core::game::Troops>(entity).is_some(), "Nation {} missing Troops component", nation_id.get());
        assert!(self.world.get::<borders_core::game::TerritorySize>(entity).is_some(), "Nation {} missing TerritorySize component", nation_id.get());

        self
    }

    /// Assert that the map has specific dimensions
    #[track_caller]
    pub fn map_dimensions(self, width: u16, height: u16) -> Self {
        let mgr = self.world.resource::<TerritoryManager>();
        assert!(mgr.width() == width, "Expected map width {}, but found {}", width, mgr.width());
        assert!(mgr.height() == height, "Expected map height {}, but found {}", height, mgr.height());
        assert!(mgr.len() == (width as usize) * (height as usize), "Expected map size {}, but found {}", (width as usize) * (height as usize), mgr.len());
        self
    }

    /// Assert that BorderCache is synchronized with ECS BorderTiles component
    #[track_caller]
    pub fn border_cache_synced(self, nation_id: NationId) -> Self {
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();
        let entity = entity_map.get_entity(nation_id);

        let component_borders = self.world.get::<BorderTiles>(entity).expect("BorderTiles component not found");

        let cache = self.world.resource::<BorderCache>();
        let cache_borders = cache.get(nation_id);

        match cache_borders {
            Some(cached) => {
                assert!(&component_borders.0 == cached, "BorderCache out of sync with BorderTiles component for nation {}.\nECS component: {:?}\nCache: {:?}", nation_id.get(), component_borders.0, cached);
            }
            None => {
                assert!(component_borders.0.is_empty(), "BorderCache missing entry for nation {} but component has {} borders", nation_id.get(), component_borders.0.len());
            }
        }
        self
    }

    /// Assert that a nation has NO border tiles
    #[track_caller]
    pub fn no_border_tiles(self, nation_id: NationId) -> Self {
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();
        let entity = entity_map.get_entity(nation_id);

        let border_tiles = self.world.get::<BorderTiles>(entity).expect("BorderTiles component not found");

        assert!(border_tiles.0.is_empty(), "Expected nation {} to have no border tiles, but found {} tiles: {:?}", nation_id.get(), border_tiles.0.len(), border_tiles.0);
        self
    }

    /// Assert that a nation has a specific number of border tiles
    #[track_caller]
    pub fn border_count(self, nation_id: NationId, expected: usize) -> Self {
        let entity_map = self.world.resource::<borders_core::game::NationEntityMap>();
        let entity = entity_map.get_entity(nation_id);

        let border_tiles = self.world.get::<BorderTiles>(entity).expect("BorderTiles component not found");

        assert!(border_tiles.0.len() == expected, "Expected nation {} to have {} border tiles, but found {}", nation_id.get(), expected, border_tiles.0.len());
        self
    }
}

/// Fluent assertion builder for Game state
///
/// Access via `game.assert()` to chain multiple assertions.
pub struct GameAssertions<'g> {
    game: &'g Game,
}

/// Extension trait to add assertion capabilities to Game
#[extension(pub trait GameAssertExt)]
impl Game {
    /// Begin a fluent assertion chain
    fn assert(&self) -> GameAssertions<'_> {
        GameAssertions { game: self }
    }
}

impl<'g> GameAssertions<'g> {
    /// Assert that a nation owns a specific tile
    #[track_caller]
    pub fn player_owns(self, tile: U16Vec2, expected: NationId) -> Self {
        self.game.world().assert().player_owns(tile, expected);
        self
    }

    /// Assert that a tile is unclaimed
    #[track_caller]
    pub fn tile_unclaimed(self, tile: U16Vec2) -> Self {
        self.game.world().assert().tile_unclaimed(tile);
        self
    }

    /// Assert that an attack exists between attacker and target
    #[track_caller]
    pub fn attack_exists(self, attacker: NationId, target: Option<NationId>) -> Self {
        self.game.world().assert().attack_exists(attacker, target);
        self
    }

    /// Assert that no attack exists between attacker and target
    #[track_caller]
    pub fn no_attack(self, attacker: NationId, target: Option<NationId>) -> Self {
        self.game.world().assert().no_attack(attacker, target);
        self
    }

    /// Assert that a nation has specific border tiles
    #[track_caller]
    pub fn border_tiles(self, nation_id: NationId, expected: &HashSet<U16Vec2>) -> Self {
        self.game.world().assert().border_tiles(nation_id, expected);
        self
    }

    /// Assert that TerritoryManager has recorded changes
    #[track_caller]
    pub fn has_territory_changes(self) -> Self {
        self.game.world().assert().has_territory_changes();
        self
    }

    /// Assert that TerritoryManager has NO recorded changes
    #[track_caller]
    pub fn no_territory_changes(self) -> Self {
        self.game.world().assert().no_territory_changes();
        self
    }

    /// Assert that a resource exists in the world
    #[track_caller]
    pub fn resource_exists<T: Resource>(self, resource_name: &str) -> Self {
        self.game.world().assert().resource_exists::<T>(resource_name);
        self
    }

    /// Assert that a resource does NOT exist in the world
    #[track_caller]
    pub fn resource_missing<T: Resource>(self, resource_name: &str) -> Self {
        self.game.world().assert().resource_missing::<T>(resource_name);
        self
    }

    /// Assert that no invalid game state exists
    #[track_caller]
    pub fn no_invalid_state(self) -> Self {
        self.game.world().assert().no_invalid_state();
        self
    }

    /// Assert that a nation has a specific troop count
    #[track_caller]
    pub fn player_troops(self, nation_id: NationId, expected: f32) -> Self {
        self.game.world().assert().player_troops(nation_id, expected);
        self
    }

    /// Assert that a tile is a border tile
    #[allow(clippy::wrong_self_convention)]
    #[track_caller]
    pub fn is_border(self, tile: U16Vec2) -> Self {
        self.game.world().assert().is_border(tile);
        self
    }

    /// Assert that a tile is NOT a border tile
    #[track_caller]
    pub fn not_border(self, tile: U16Vec2) -> Self {
        self.game.world().assert().not_border(tile);
        self
    }

    /// Assert that a nation owns all tiles in a slice
    #[track_caller]
    pub fn player_owns_all(self, tiles: &[U16Vec2], expected: NationId) -> Self {
        self.game.world().assert().player_owns_all(tiles, expected);
        self
    }

    /// Assert that the ChangeBuffer contains exactly N changes
    #[track_caller]
    pub fn change_count(self, expected: usize) -> Self {
        self.game.world().assert().change_count(expected);
        self
    }

    /// Assert that a nation entity has all standard game components
    #[track_caller]
    pub fn player_has_components(self, nation_id: NationId) -> Self {
        self.game.world().assert().player_has_components(nation_id);
        self
    }

    /// Assert that the map has specific dimensions
    #[track_caller]
    pub fn map_dimensions(self, width: u16, height: u16) -> Self {
        self.game.world().assert().map_dimensions(width, height);
        self
    }

    /// Assert that BorderCache is synchronized with ECS BorderTiles component
    #[track_caller]
    pub fn border_cache_synced(self, nation_id: NationId) -> Self {
        self.game.world().assert().border_cache_synced(nation_id);
        self
    }

    /// Assert that a nation has NO border tiles
    #[track_caller]
    pub fn no_border_tiles(self, nation_id: NationId) -> Self {
        self.game.world().assert().no_border_tiles(nation_id);
        self
    }

    /// Assert that a nation has a specific number of border tiles
    #[track_caller]
    pub fn border_count(self, nation_id: NationId, expected: usize) -> Self {
        self.game.world().assert().border_count(nation_id, expected);
        self
    }
}
