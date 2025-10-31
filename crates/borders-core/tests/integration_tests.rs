// Integration tests for multi-system interactions: spawn phase, territory conquest, attacks

mod common;

use assert2::assert;
use borders_core::game::builder::GameBuilder;
use borders_core::networking::NetworkMode;
use borders_core::prelude::*;
use common::{GameAssertExt, GameBuilderTestExt, GameTestExt};

#[test]
fn test_spawn_phase_lifecycle() {
    let mut game = GameBuilder::simple().with_nation(NationId::ZERO, 100.0).with_spawn_phase_enabled().with_network(NetworkMode::Local).with_systems(false).build();

    let spawn_phase = game.world().resource::<SpawnPhase>();
    assert!(spawn_phase.active, "SpawnPhase should be active after initialization");

    game.deactivate_spawn_phase();

    let spawn_phase = game.world().resource::<SpawnPhase>();
    assert!(!spawn_phase.active, "SpawnPhase should be inactive after deactivation");

    game.assert().resource_exists::<TerritoryManager>("TerritoryManager");
}

#[test]
fn test_territory_conquest_triggers_changes() {
    let player0 = NationId::ZERO;

    let mut game = GameBuilder::simple().with_nation(player0, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    game.assert().no_territory_changes();

    let tile = U16Vec2::new(50, 50);
    game.conquer_tile(tile, player0);

    game.assert().has_territory_changes().player_owns(tile, player0);
}

#[test]
fn test_multi_player_territory_conquest() {
    let player0 = NationId::ZERO;
    let player1 = NationId::new(1).unwrap();
    let player2 = NationId::new(2).unwrap();

    let mut game = GameBuilder::simple().with_nation(player0, 100.0).with_nation(player1, 100.0).with_nation(player2, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let tile0 = U16Vec2::new(10, 10);
    let tile1 = U16Vec2::new(20, 20);
    let tile2 = U16Vec2::new(30, 30);

    game.conquer_tile(tile0, player0);
    game.conquer_tile(tile1, player1);
    game.conquer_tile(tile2, player2);

    game.assert().player_owns(tile0, player0).player_owns(tile1, player1).player_owns(tile2, player2);

    let territory_manager = game.world().resource::<TerritoryManager>();
    let changes: Vec<_> = territory_manager.iter_changes().collect();
    assert!(changes.len() == 3, "Should have 3 changes (one per player), but found {}", changes.len());
}

#[test]
fn test_territory_ownership_transitions() {
    let player0 = NationId::ZERO;
    let player1 = NationId::new(1).unwrap();

    let mut game = GameBuilder::simple().with_nation(player0, 100.0).with_nation(player1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let tile = U16Vec2::new(50, 50);

    game.assert().tile_unclaimed(tile);

    game.conquer_tile(tile, player0);
    game.assert().player_owns(tile, player0);

    game.clear_territory_changes();
    game.assert().no_territory_changes();

    let previous_owner = game.clear_tile(tile);
    assert!(previous_owner == Some(player0), "Previous owner should be player 0");
    game.assert().tile_unclaimed(tile).has_territory_changes();

    game.clear_territory_changes();

    game.conquer_tile(tile, player1);
    game.assert().player_owns(tile, player1).has_territory_changes();
}

#[test]
fn test_border_updates_on_territory_change() {
    let player0 = NationId::ZERO;

    let mut game = GameBuilder::with_map_size(100, 100).with_nation(player0, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    let center = U16Vec2::new(50, 50);
    let adjacent = U16Vec2::new(51, 50);

    game.conquer_tile(center, player0);

    let territory_manager = game.world().resource::<TerritoryManager>();
    assert!(territory_manager.is_border(center), "Center tile should be a border (adjacent to unclaimed)");

    game.conquer_tile(adjacent, player0);

    let territory_manager = game.world().resource::<TerritoryManager>();
    assert!(territory_manager.is_border(center), "Center tile should still be a border");
    assert!(territory_manager.is_border(adjacent), "Adjacent tile should be a border");
}

#[test]
fn test_initialization_with_territories() {
    let player0 = NationId::ZERO;
    let player1 = NationId::new(1).unwrap();

    let territory0 = vec![U16Vec2::new(10, 10), U16Vec2::new(10, 11)];
    let territory1 = vec![U16Vec2::new(20, 20), U16Vec2::new(20, 21)];

    let mut game = GameBuilder::simple().with_nation(player0, 100.0).with_nation(player1, 100.0).with_network(NetworkMode::Local).with_systems(false).build();

    // Pre-assign territories using extension trait
    game.conquer_tiles(&territory0, player0);
    game.conquer_tiles(&territory1, player1);

    let mut assertions = game.assert();
    for tile in &territory0 {
        assertions = assertions.player_owns(*tile, player0);
    }
    for tile in &territory1 {
        assertions = assertions.player_owns(*tile, player1);
    }

    let territory_manager = game.world().resource::<TerritoryManager>();
    let changes_count = territory_manager.iter_changes().count();
    assert!(changes_count == 4, "After initialization with 4 tiles, expected 4 changes, but found {}", changes_count);
}
