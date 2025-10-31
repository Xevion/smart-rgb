//! Tests for tick rate system to prevent regressions
//!
//! These tests verify that the turn generation system produces turns at the
//! correct rate regardless of frame rate, and that timing remains accurate
//! over long simulations without drift.
//!
//! ## Current Implementation Notes
//!
//! The turn generation system is currently hardcoded to 10 TPS (100ms per tick)
//! in `SharedTurnGenerator`. While `GameBuilder::with_tick_rate()` exists and
//! creates a `FixedTime` resource, this resource is not currently used by the
//! turn generator. Custom tick rates are not supported at this time.
//!
//! These tests focus on verifying the 10 TPS behavior works correctly across
//! different frame rates and long simulations.

mod common;

use assert2::assert;
use borders_core::networking::server::LocalTurnServerHandle;
use borders_core::prelude::*;
use borders_core::time::{Clock, Time};
use common::MapBuilder;
use quanta::Mock;
use rstest::rstest;
use std::sync::Arc;
use std::time::Duration;

/// Tolerance for timing assertions (±2ms per tick)
const TIMING_TOLERANCE_MS: f64 = 2.0;

/// Fixed tick rate (hardcoded in SharedTurnGenerator)
const TICK_RATE_TPS: f64 = 10.0;

/// Set up a minimal game for testing tick rate with custom TPS and mock clock
fn setup_tick_rate_test_game(tick_rate: u32) -> (Game, Arc<Mock>) {
    let terrain_data = MapBuilder::new(20, 20).all_conquerable().build();

    // Create mock clock for precise time control
    let (clock, mock) = Clock::mock();

    let mut game = GameBuilder::new()
        .with_map(Arc::new(terrain_data))
        .with_bots(1) // Need at least 1 bot for valid game
        .with_network(NetworkMode::Local)
        .with_spawn_phase(None) // Skip spawn phase
        .with_tick_rate(tick_rate)
        .with_clock(clock)
        .with_rng_seed(0x12345)
        .build();

    // Start game immediately (skip spawn phase)
    if let Some(mut generator) = game.world_mut().get_resource_mut::<borders_core::networking::server::TurnGenerator>() {
        generator.start_game_immediately();
    }

    (game, mock)
}

/// Helper to verify tick timing matches expected TPS within tolerance
///
/// Note: Turn numbers start at 0, so turn N means (N+1) turns have been generated.
/// For example, reaching turn 599 means 600 turns total (Turn 0 through Turn 599).
fn assert_tick_timing(expected_tps: f64, final_turn_number: u64, elapsed_ms: f64, tolerance_ms: f64) {
    let tick_interval_ms = 1000.0 / expected_tps;

    // Turn numbers start at 0, so turn N means (N+1) turns total
    let actual_turn_count = final_turn_number + 1;

    // Calculate expected turn count based on elapsed time
    let expected_turn_count = (elapsed_ms / tick_interval_ms).floor() as u64;

    // Calculate timing error
    let turn_count_diff = actual_turn_count as i64 - expected_turn_count as i64;
    let timing_error_ms = turn_count_diff as f64 * tick_interval_ms;

    assert!(timing_error_ms.abs() <= tolerance_ms, "Tick rate timing error: expected {} turns ({} TPS over {:.0}ms), got {} turns (final turn number: {}), error: {:.2}ms (tolerance: ±{}ms)", expected_turn_count, expected_tps, elapsed_ms, actual_turn_count, final_turn_number, timing_error_ms, tolerance_ms);
}

/// Simulates game updates at a specific frame rate for a given duration
///
/// Returns the final turn number reached
fn simulate_frames(game: &mut Game, mock: &Arc<Mock>, frame_interval_ms: f64, duration_ms: f64) -> u64 {
    let frame_count = (duration_ms / frame_interval_ms).ceil() as usize;
    let frame_duration_micros = (frame_interval_ms * 1000.0) as u64;

    for _ in 0..frame_count {
        // Advance mock clock by frame interval
        mock.increment(Duration::from_micros(frame_duration_micros));

        // Tick the time resource to measure delta
        if let Some(mut time) = game.world_mut().get_resource_mut::<Time>() {
            time.tick();
        }

        // Update game (processes turns, runs systems)
        game.update();
    }

    // Get final turn number
    game.world().get_resource::<borders_core::game::systems::CurrentTurn>().map(|ct| ct.turn.turn_number).unwrap_or(0)
}

#[test]
fn test_default_tick_rate_10_tps() {
    let (mut game, mock) = setup_tick_rate_test_game(10);

    // Verify server is running
    {
        let handle = game.world().get_resource::<LocalTurnServerHandle>().expect("LocalTurnServerHandle should exist");
        assert!(handle.is_running());
        assert!(!handle.is_paused());
    }

    // Simulate 60 seconds at 60 FPS (standard frame rate)
    let duration_ms = 60_000.0;
    let frame_rate = 60.0;
    let frame_interval_ms = 1000.0 / frame_rate;

    let final_turn = simulate_frames(&mut game, &mock, frame_interval_ms, duration_ms);

    // At 10 TPS, 60 seconds should produce 600 turns (Turn 0-599)
    assert_tick_timing(TICK_RATE_TPS, final_turn, duration_ms, TIMING_TOLERANCE_MS);
}

/// Test tick rate independence across various frame rates
///
/// Verifies that turn generation remains consistent at 10 TPS regardless of
/// the frame rate at which game.update() is called.
#[rstest]
#[case::fps_30(30.0, "30 FPS (slower frame rate)")]
#[case::fps_60(60.0, "60 FPS (standard frame rate)")]
#[case::fps_90(90.0, "90 FPS (above standard)")]
#[case::fps_120(120.0, "120 FPS (high refresh rate)")]
#[case::fps_144(144.0, "144 FPS (high refresh rate)")]
fn test_tick_rate_frame_independence(#[case] fps: f64, #[case] description: &str) {
    let (mut game, mock) = setup_tick_rate_test_game(10);

    // Simulate 60 seconds at specified frame rate
    let duration_ms = 60_000.0;
    let frame_interval_ms = 1000.0 / fps;

    let final_turn = simulate_frames(&mut game, &mock, frame_interval_ms, duration_ms);

    // Should produce 600 turns at 10 TPS regardless of frame rate
    // Test name via case attribute provides context
    let _ = description; // Suppress unused warning
    assert_tick_timing(TICK_RATE_TPS, final_turn, duration_ms, TIMING_TOLERANCE_MS);
}

#[test]
fn test_tick_rate_frame_independence_variable_fps() {
    let (mut game, mock) = setup_tick_rate_test_game(10);

    // Simulate variable frame rate (jittery timing) by alternating between
    // fast and slow frames
    let target_duration_ms = 60_000.0;
    let mut elapsed_ms = 0.0;

    // Alternate between 30 FPS and 100 FPS frames
    let mut use_fast_frame = true;
    while elapsed_ms < target_duration_ms {
        let frame_interval_ms = if use_fast_frame {
            1000.0 / 100.0 // 10ms (100 FPS)
        } else {
            1000.0 / 30.0 // ~33.33ms (30 FPS)
        };

        mock.increment(Duration::from_micros((frame_interval_ms * 1000.0) as u64));

        if let Some(mut time) = game.world_mut().get_resource_mut::<Time>() {
            time.tick();
        }

        game.update();

        elapsed_ms += frame_interval_ms;
        use_fast_frame = !use_fast_frame;
    }

    let final_turn = game.world().get_resource::<borders_core::game::systems::CurrentTurn>().map(|ct| ct.turn.turn_number).unwrap_or(0);

    // Variable frame rate may overshoot target duration slightly due to while loop
    // condition, causing ±1 turn variance. Use tolerance of ±1 tick (100ms at 10 TPS)
    let variable_fps_tolerance_ms = 100.0;
    assert_tick_timing(TICK_RATE_TPS, final_turn, elapsed_ms, variable_fps_tolerance_ms);
}

#[test]
fn test_no_drift_over_long_simulation() {
    let (mut game, mock) = setup_tick_rate_test_game(10);

    // Simulate 5 minutes (300 seconds) to catch cumulative drift
    let duration_ms = 300_000.0;
    let frame_interval_ms = 1000.0 / 60.0;

    let final_turn = simulate_frames(&mut game, &mock, frame_interval_ms, duration_ms);

    // At 10 TPS, 300 seconds should produce 3000 turns (Turn 0-2999)
    // Use same tolerance - no cumulative drift should occur
    assert_tick_timing(TICK_RATE_TPS, final_turn, duration_ms, TIMING_TOLERANCE_MS);
}
