//! Tests for intent timing and turn boundary race conditions
//!
//! These tests verify that intents sent between turn boundaries are properly
//! buffered and included in the next turn, rather than being dropped.

mod common;

use assert2::{assert, let_assert};
use borders_core::networking::Intent;
use borders_core::networking::client::Connection;
use borders_core::networking::server::LocalTurnServerHandle;
use borders_core::prelude::*;
use borders_core::time::Time;
use common::MapBuilder;
use std::sync::Arc;

/// Set up a minimal game for testing turn timing
fn setup_turn_test_game() -> Game {
    let terrain_data = MapBuilder::new(20, 20).all_conquerable().build();

    let mut game = GameBuilder::new()
        .with_map(Arc::new(terrain_data))
        .with_bots(1) // Need at least 1 bot to have a valid player
        .with_network(NetworkMode::Local)
        .with_spawn_phase(None) // Skip spawn phase
        .with_rng_seed(0x12345)
        .build();

    // Start game immediately (skip spawn phase) - same as smoke tests
    if let Some(mut generator) = game.world_mut().get_resource_mut::<borders_core::networking::server::TurnGenerator>() {
        generator.start_game_immediately();
    }

    game
}

/// Helper to send an attack intent
fn send_attack_intent(game: &mut Game, troops: u32) {
    let world = game.world_mut();

    // Get current turn for intent tracking
    let current_turn = world.get_resource::<borders_core::game::turn::CurrentTurn>().map(|ct| ct.turn.turn_number).unwrap_or(0);

    // Get the connection and send intent
    let mut connection = world.get_resource_mut::<Connection>().expect("Connection should exist");

    let intent = Intent::Action(GameAction::Attack { target: None, troops });

    connection.send_intent(intent, current_turn);
}

/// Helper to advance time by specific milliseconds
fn advance_time(game: &mut Game, millis: u64) {
    if let Some(mut time) = game.world_mut().get_resource_mut::<Time>() {
        #[allow(deprecated)]
        time.update(std::time::Duration::from_millis(millis));
    }
}

/// Helper to get the current turn from CurrentTurn resource
fn get_current_turn(game: &Game) -> Option<Turn> {
    game.world().get_resource::<borders_core::game::turn::CurrentTurn>().map(|ct| ct.turn.clone())
}

#[test]
fn test_intent_sent_between_turns_is_included() {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("debug"))).with_test_writer().try_init();

    let mut game = setup_turn_test_game();

    // Verify server is running
    {
        let handle = game.world().get_resource::<LocalTurnServerHandle>().expect("LocalTurnServerHandle should exist");
        assert!(handle.is_running());
        assert!(!handle.is_paused());
    }

    // Frame 1: First update to emit Turn(0)
    advance_time(&mut game, 100);
    game.update();

    let turn0 = get_current_turn(&game);
    let_assert!(Some(turn) = turn0);
    assert!(turn.turn_number == 0, "Should get Turn(0) initially");
    assert!(turn.intents.is_empty(), "Turn(0) should have no intents (game start)");

    // Frame 2: Advance 50ms (halfway to next turn)
    advance_time(&mut game, 50);
    game.update();

    // Turn should still be Turn(0) - no new turn generated yet
    let still_turn0 = get_current_turn(&game);
    let_assert!(Some(turn) = still_turn0);
    assert!(turn.turn_number == 0, "Should still be Turn(0) at 50ms (halfway)");

    // NOW send the attack intent (at 50ms into the turn period)
    send_attack_intent(&mut game, 100);
    eprintln!("Intent sent at 50ms mark");

    // Frame 3: Advance another 50ms (should trigger Turn(1) at 100ms total)
    advance_time(&mut game, 50);
    game.update();

    // Turn(1) should be generated and should contain our intent
    let turn1 = get_current_turn(&game);
    let_assert!(Some(turn) = turn1, "Turn(1) should be generated at 100ms");
    assert!(turn.turn_number == 1, "Should be Turn(1)");

    // THIS IS THE KEY ASSERTION - it will FAIL with the current bug
    assert!(turn.intents.len() == 1, "Turn(1) should contain the intent sent at 50ms (found {} intents)", turn.intents.len());

    // Verify it's an attack intent
    let_assert!(Some(sourced_intent) = turn.intents.first());
    let_assert!(Intent::Action(GameAction::Attack { troops, .. }) = &sourced_intent.intent);
    assert!(*troops == 100, "Should be the attack we sent");

    eprintln!("✓ Intent successfully included in Turn(1)");
}

#[test]
fn test_intent_sent_at_turn_boundary_is_included() {
    let mut game = setup_turn_test_game();

    // Emit Turn(0)
    advance_time(&mut game, 100);
    game.update();

    // Send intent right at the start of the turn period (0ms into next turn)
    send_attack_intent(&mut game, 50);

    // Advance full 100ms to generate next turn
    advance_time(&mut game, 100);
    game.update();

    let turn1 = get_current_turn(&game);
    let_assert!(Some(turn) = turn1, "Turn(1) should be generated at 100ms");
    assert!(turn.turn_number == 1);
    assert!(turn.intents.len() == 1, "Intent sent at 0ms should be included in Turn(1)");
}

#[test]
fn test_multiple_intents_across_frames() {
    let mut game = setup_turn_test_game();

    // Emit Turn(0)
    advance_time(&mut game, 100);
    game.update();

    // Send first intent at 25ms
    advance_time(&mut game, 25);
    game.update();
    send_attack_intent(&mut game, 100);

    // Send second intent at 50ms
    advance_time(&mut game, 25);
    game.update();
    send_attack_intent(&mut game, 200);

    // Send third intent at 75ms
    advance_time(&mut game, 25);
    game.update();
    send_attack_intent(&mut game, 300);

    // Generate turn at 100ms
    advance_time(&mut game, 25);
    game.update();

    let turn1 = get_current_turn(&game);
    let_assert!(Some(turn) = turn1, "Turn(1) should be generated at 100ms");
    assert!(turn.turn_number == 1);
    assert!(turn.intents.len() == 3, "All 3 intents sent during turn period should be included (found {})", turn.intents.len());
}

#[test]
fn test_intent_sent_at_99ms_is_included() {
    let mut game = setup_turn_test_game();

    // Emit Turn(0)
    advance_time(&mut game, 100);
    game.update();

    // Advance almost to turn boundary (99ms)
    advance_time(&mut game, 99);
    game.update();

    // Send intent at 99ms (1ms before turn fires)
    send_attack_intent(&mut game, 150);

    // Generate turn at 100ms+
    advance_time(&mut game, 1);
    game.update();

    let turn1 = get_current_turn(&game);
    let_assert!(Some(turn) = turn1, "Turn(1) should be generated at 100ms");
    assert!(turn.turn_number == 1);
    assert!(turn.intents.len() == 1, "Intent sent at 99ms should be included in Turn(1)");
}
