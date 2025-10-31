// Spawn system tests: activation, deactivation, territory claiming

mod common;

use assert2::assert;
use borders_core::game::builder::GameBuilder;
use borders_core::networking::NetworkMode;
use borders_core::prelude::*;
use common::{GameAssertExt, GameBuilderTestExt, GameTestExt};

#[test]
fn test_spawn_phase_activation() {
    let game = GameBuilder::simple().with_nation(NationId::ZERO, 100.0).with_spawn_phase_enabled().with_network(NetworkMode::Local).with_systems(false).build();

    game.assert().resource_exists::<SpawnPhase>("SpawnPhase");

    let spawn_phase = game.world().resource::<SpawnPhase>();
    assert!(spawn_phase.active, "SpawnPhase should be active when initialized with with_spawn_phase_enabled()");
}

#[test]
fn test_spawn_phase_deactivation() {
    let mut game = GameBuilder::simple().with_nation(NationId::ZERO, 100.0).with_spawn_phase_enabled().with_network(NetworkMode::Local).with_systems(false).build();

    assert!(game.world().resource::<SpawnPhase>().active);

    game.deactivate_spawn_phase();

    assert!(!game.world().resource::<SpawnPhase>().active, "SpawnPhase should be inactive after deactivation");
}

#[test]
fn test_spawn_territory_claiming() {
    let nation0 = NationId::ZERO;
    let nation1 = NationId::new(1).unwrap();

    let mut game = GameBuilder::simple().with_nation(nation0, 100.0).with_nation(nation1, 100.0).with_spawn_phase_enabled().with_network(NetworkMode::Local).with_systems(false).build();

    let spawn0 = U16Vec2::new(25, 25);
    let spawn1 = U16Vec2::new(75, 75);

    game.conquer_tile(spawn0, nation0);
    game.conquer_tile(spawn1, nation1);

    game.assert().player_owns(spawn0, nation0).player_owns(spawn1, nation1);

    assert!(game.world().resource::<SpawnPhase>().active);
}

#[test]
fn test_spawn_with_pre_assigned_territories() {
    let nation0 = NationId::ZERO;
    let nation1 = NationId::new(1).unwrap();

    let spawn_territory0 = vec![U16Vec2::new(25, 25), U16Vec2::new(25, 26), U16Vec2::new(26, 25)];
    let spawn_territory1 = vec![U16Vec2::new(75, 75), U16Vec2::new(75, 76), U16Vec2::new(76, 75)];

    let mut game = GameBuilder::simple().with_nation(nation0, 100.0).with_nation(nation1, 100.0).with_spawn_phase_enabled().with_network(NetworkMode::Local).with_systems(false).build();

    // Pre-assign territories using extension trait
    game.conquer_tiles(&spawn_territory0, nation0);
    game.conquer_tiles(&spawn_territory1, nation1);

    let mut assertions = game.assert();
    for tile in &spawn_territory0 {
        assertions = assertions.player_owns(*tile, nation0);
    }
    for tile in &spawn_territory1 {
        assertions = assertions.player_owns(*tile, nation1);
    }

    assert!(game.world().resource::<SpawnPhase>().active);
}
