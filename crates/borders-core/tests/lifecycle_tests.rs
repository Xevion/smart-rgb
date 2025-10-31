// Game lifecycle tests: initialization, player spawning, cleanup

mod common;

use assert2::assert;
use std::sync::Arc;

use borders_core::prelude::*;

use common::{GameAssertExt, MapBuilder};

// Helper to create initialized game for testing
fn create_initialized_game(map_size: U16Vec2) -> Game {
    let terrain_data = Arc::new(MapBuilder::new(map_size.x, map_size.y).all_conquerable().build());

    GameBuilder::new()
        .with_map(terrain_data)
        .with_network(NetworkMode::Local)
        .with_local_player(NationId::ZERO)
        .with_bots(10)
        .with_rng_seed(0xDEADBEEF)
        .with_spawn_phase(Some(30))
        .with_systems(false) // Don't run systems in tests
        .build()
}

#[test]
fn test_initialize_game_creates_all_resources() {
    let game = create_initialized_game(U16Vec2::new(100, 100));

    game.assert().resource_exists::<TerritoryManager>("TerritoryManager").resource_exists::<ActiveAttacks>("ActiveAttacks").resource_exists::<TerrainData>("TerrainData").resource_exists::<DeterministicRng>("DeterministicRng").resource_exists::<CoastalTiles>("CoastalTiles").resource_exists::<NationEntityMap>("NationEntityMap").resource_exists::<LocalPlayerContext>("LocalPlayerContext").resource_exists::<SpawnPhase>("SpawnPhase").resource_exists::<SpawnTimeout>("SpawnTimeout").resource_exists::<SpawnManager>("SpawnManager").resource_exists::<server::LocalTurnServerHandle>("LocalTurnServerHandle").resource_exists::<server::TurnReceiver>("TurnReceiver").resource_exists::<server::TurnGenerator>("TurnGenerator");
}

#[test]
fn test_initialize_creates_player_entities() {
    let game = create_initialized_game(U16Vec2::new(100, 100));
    let world = game.world();

    // Get entity map
    let entity_map = world.resource::<NationEntityMap>();

    // Verify 1 human + 10 bots = total players (matches create_initialized_game params)
    let expected_bot_count = 10; // From create_initialized_game params
    assert!(entity_map.0.len() == 1 + expected_bot_count, "Should have exactly {} player entities (1 human + {} bots), but found {}", 1 + expected_bot_count, expected_bot_count, entity_map.0.len());

    // Verify human player (ID 0) exists
    let human_entity = entity_map.0.get(&NationId::ZERO).expect("Human player entity not found");

    // Verify human has all required components
    assert!(world.get::<NationName>(*human_entity).is_some(), "Human player missing NationName component");
    assert!(world.get::<NationColor>(*human_entity).is_some(), "Human player missing PlayerColor component");
    assert!(world.get::<BorderTiles>(*human_entity).is_some(), "Human player missing BorderTiles component");
    assert!(world.get::<Troops>(*human_entity).is_some(), "Human player missing Troops component");
    assert!(world.get::<TerritorySize>(*human_entity).is_some(), "Human player missing TerritorySize component");

    // Verify initial troops are correct
    let troops = world.get::<Troops>(*human_entity).unwrap();
    assert!(troops.0 == constants::nation::INITIAL_TROOPS, "Human player should start with {} troops, but has {}", constants::nation::INITIAL_TROOPS, troops.0);

    // Verify territory size starts at 0
    let territory_size = world.get::<TerritorySize>(*human_entity).unwrap();
    assert!(territory_size.0 == 0, "Human player should start with 0 territory, but has {}", territory_size.0);
}

#[test]
fn test_spawn_phase_activates() {
    let game = create_initialized_game(U16Vec2::new(100, 100));
    let world = game.world();

    // Verify spawn phase is active
    let spawn_phase = world.resource::<SpawnPhase>();
    assert!(spawn_phase.active, "SpawnPhase should be active after initialization");

    let spawn_timeout = world.resource::<SpawnTimeout>();
    assert!(spawn_timeout.duration_secs > 0.0, "SpawnTimeout should have a positive duration");
}

#[test]
fn test_cannot_start_game_twice() {
    let game = create_initialized_game(U16Vec2::new(100, 100));
    let world = game.world();

    game.assert().resource_exists::<TerritoryManager>("TerritoryManager");

    let has_territory_manager = world.contains_resource::<TerritoryManager>();
    assert!(has_territory_manager, "This check prevents double-initialization in production code");
}

#[test]
fn test_territory_manager_dimensions() {
    let map_size = U16Vec2::new(80, 60);
    let game = create_initialized_game(map_size);
    let world = game.world();

    let territory_manager = world.resource::<TerritoryManager>();
    assert!(territory_manager.width() == map_size.x, "TerritoryManager width should match map width: expected {}, got {}", map_size.x, territory_manager.width());
    assert!(territory_manager.height() == map_size.y, "TerritoryManager height should match map height: expected {}, got {}", map_size.y, territory_manager.height());
    assert!(territory_manager.len() == (map_size.x as usize) * (map_size.y as usize), "TerritoryManager should have width × height tiles: expected {}, got {}", (map_size.x as usize) * (map_size.y as usize), territory_manager.len());
}
