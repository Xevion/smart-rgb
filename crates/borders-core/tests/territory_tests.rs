// Territory manager tests: change tracking, ownership, border detection

mod common;

use assert2::assert;
use borders_core::game::builder::GameBuilder;
use borders_core::networking::NetworkMode;
use borders_core::prelude::*;
use common::{GameAssertExt, GameBuilderTestExt, GameTestExt};

#[test]
fn test_conquer_tile_adds_to_changes() {
    let mut game = GameBuilder::simple().with_network(NetworkMode::Local).with_systems(false).build();
    let mut territory_manager = game.world_mut().resource_mut::<TerritoryManager>();

    // Initially no changes
    assert!(!territory_manager.has_changes());

    // Conquer a tile
    let tile = U16Vec2::new(50, 50);
    territory_manager.conquer(tile, NationId::ZERO);

    assert!(territory_manager.has_changes(), "Conquering a tile should record a change in the ChangeBuffer");

    // The change should be the tile we conquered
    let changes: Vec<_> = territory_manager.iter_changes().collect();
    assert!(changes.len() == 1, "Should have exactly 1 change, but found {}", changes.len());
    assert!(changes[0] == tile, "Change should be the conquered tile {:?}, but found {:?}", tile, changes[0]);
}

#[test]
fn test_iter_changes_preserves_buffer() {
    let mut game = GameBuilder::simple().with_network(NetworkMode::Local).with_systems(false).build();
    let mut territory_manager = game.world_mut().resource_mut::<TerritoryManager>();

    // Conquer a tile
    territory_manager.conquer(U16Vec2::new(50, 50), NationId::ZERO);
    assert!(territory_manager.has_changes());

    let _changes: Vec<_> = territory_manager.iter_changes().collect();

    assert!(territory_manager.has_changes(), "iter_changes should not clear the ChangeBuffer");
}

#[test]
fn test_drain_changes_clears_buffer() {
    let mut game = GameBuilder::simple().with_network(NetworkMode::Local).with_systems(false).build();
    let mut territory_manager = game.world_mut().resource_mut::<TerritoryManager>();

    // Conquer tiles
    territory_manager.conquer(U16Vec2::new(50, 50), NationId::ZERO);
    territory_manager.conquer(U16Vec2::new(51, 50), NationId::ZERO);
    assert!(territory_manager.has_changes());

    // Drain changes
    let changes: Vec<_> = territory_manager.drain_changes().collect();
    assert!(changes.len() == 2, "Should have drained 2 changes, but found {}", changes.len());

    assert!(!territory_manager.has_changes(), "drain_changes should clear the ChangeBuffer");
}

#[test]
fn test_border_detection_map_edges() {
    let mut game = GameBuilder::with_map_size(100, 100).with_nation(NationId::ZERO, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let corner = U16Vec2::new(0, 0);
    game.conquer_tile(corner, NationId::ZERO);

    game.assert().is_border(corner);

    let edge = U16Vec2::new(0, 50);
    game.conquer_tile(edge, NationId::ZERO);

    game.assert().is_border(edge);
}

#[test]
fn test_border_detection_interior_tiles() {
    let mut game = GameBuilder::with_map_size(100, 100).with_nation(NationId::ZERO, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let center = U16Vec2::new(50, 50);
    for dy in -1..=1 {
        for dx in -1..=1 {
            let tile = U16Vec2::new((center.x as i32 + dx) as u16, (center.y as i32 + dy) as u16);
            game.conquer_tile(tile, NationId::ZERO);
        }
    }

    game.assert().not_border(center).is_border(U16Vec2::new(49, 50)).is_border(U16Vec2::new(51, 50));
}

#[test]
fn test_clear_tile_records_change() {
    let mut game = GameBuilder::simple().with_network(NetworkMode::Local).with_systems(false).build();
    let mut territory_manager = game.world_mut().resource_mut::<TerritoryManager>();

    // Conquer then clear changes
    let tile = U16Vec2::new(50, 50);
    territory_manager.conquer(tile, NationId::ZERO);
    territory_manager.clear_changes();
    assert!(!territory_manager.has_changes());

    // Clear the tile
    let previous_owner = territory_manager.clear(tile);
    assert!(previous_owner == Some(NationId::ZERO), "Expected previous owner to be Some(NationId::ZERO), but found {:?}", previous_owner);

    assert!(territory_manager.has_changes(), "Clearing a tile should record a change");
}
