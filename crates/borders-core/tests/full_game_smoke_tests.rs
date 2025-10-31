//! Full game smoke tests that run complete games with bots
//!
//! These tests verify the entire game lifecycle by running complete games
//! for 600 ticks (60 seconds equivalent at 10 TPS) with multiple bots.
//! The tests ensure the game doesn't crash and that all systems execute properly.

mod common;

use assert2::assert;
use borders_core::prelude::*;
use common::MapBuilder;
use rstest::rstest;
use std::collections::HashMap;
use std::sync::Arc;

/// Statistics collected during a full game run
#[derive(Debug)]
struct GameStats {
    /// Final turn number reached
    final_turn: u64,
    /// Total tiles owned by all players
    total_territory_owned: u32,
    /// Number of bots that have claimed territory
    bots_with_territory: usize,
    /// Number of bots that took at least one action
    active_bots: usize,
    /// Total number of bot actions taken
    total_bot_actions: usize,
}

/// Set up a test game using shared initialization code
///
/// # Arguments
/// * `player_count` - Number of bot players (no human players)
/// * `map_size` - (width, height) of the map in tiles
/// * `rng_seed` - Optional RNG seed for deterministic tests (defaults to 0xDEADBEEF)
///
/// # Returns
/// Configured Game ready to run game updates
fn setup_test_game(player_count: usize, map_size: (u16, u16), rng_seed: Option<u64>) -> Game {
    let (map_width, map_height) = map_size;

    // Generate terrain - all land tiles for maximum playable area
    let terrain_data = MapBuilder::new(map_width, map_height).all_conquerable().build();

    // Create game with unified GameBuilder API
    let mut game = GameBuilder::new()
        .with_map(Arc::new(terrain_data))
        .with_bots(player_count as u32)
        .with_network(NetworkMode::Local)
        .with_spawn_phase(None) // Skip spawn phase for immediate game start
        .with_rng_seed(rng_seed.unwrap_or(0xDEADBEEF)) // Deterministic for reproducible tests
        .build();

    // Initialize UI messages if the ui feature is enabled (for headless tests with UI systems)
    use borders_core::ui::protocol::{BackendMessage, FrontendMessage};
    game.add_message::<BackendMessage>();
    game.add_message::<FrontendMessage>();

    // When skipping spawn phase, immediately mark the game as started
    if let Some(mut generator) = game.world_mut().get_resource_mut::<borders_core::networking::server::TurnGenerator>() {
        generator.start_game_immediately();
    }

    game
}

/// Run a complete game with the specified number of bot players
///
/// # Arguments
/// * `player_count` - Number of bot players (no human players)
/// * `map_size` - (width, height) of the map in tiles
/// * `target_ticks` - Number of game ticks to run (10 TPS = 100ms per tick)
/// * `rng_seed` - Optional RNG seed for deterministic tests
///
/// # Returns
/// Statistics about the completed game
fn run_full_game(player_count: usize, map_size: (u16, u16), target_ticks: u64, rng_seed: Option<u64>) -> GameStats {
    let mut game = setup_test_game(player_count, map_size, rng_seed);

    // Debug: Check initial state
    eprintln!("=== Initial State ===");
    eprintln!("TurnGenerator exists: {}", game.world().get_resource::<borders_core::networking::server::TurnGenerator>().is_some());
    eprintln!("TurnReceiver exists: {}", game.world().get_resource::<borders_core::networking::server::TurnReceiver>().is_some());
    eprintln!("LocalPlayerContext exists: {}", game.world().get_resource::<borders_core::game::LocalPlayerContext>().is_some());
    if let Some(handle) = game.world().get_resource::<borders_core::networking::server::LocalTurnServerHandle>() {
        eprintln!("Server running: {}, paused: {}", handle.is_running(), handle.is_paused());
    }
    if let Some(spawn_manager) = game.world().get_resource::<borders_core::game::SpawnManager>() {
        eprintln!("SpawnManager spawns: {} bots, {} players", spawn_manager.get_bot_spawns().len(), spawn_manager.get_player_spawns().len());
    }

    // Track initial and final territory sizes to verify bot activity
    let initial_territory_sizes: HashMap<_, _> = {
        use borders_core::game::ai::bot::Bot;
        use borders_core::game::entities::TerritorySize;
        let world = game.world_mut();
        let mut query = world.query::<(&Bot, &NationId, &TerritorySize)>();
        query.iter(world).map(|(_, &id, size)| (id, size.0)).collect()
    };

    // Main game loop - run for target_ticks iterations
    for tick_num in 0..target_ticks {
        // Advance time by exactly 100ms per tick (10 TPS)
        if let Some(mut time) = game.world_mut().get_resource_mut::<Time>() {
            #[allow(deprecated)]
            time.update(std::time::Duration::from_millis(100));
        }

        // Update the game - this runs all Update and Last schedules
        game.update();

        // Debug: Check BorderCache on first few turns
        if tick_num == 1 {
            use borders_core::game::BorderCache;

            let world = game.world();
            if let Some(border_cache) = world.get_resource::<BorderCache>() {
                let cache_map = border_cache.as_map();
                let mut cache_keys: Vec<u16> = cache_map.keys().map(|id| id.get()).collect();
                cache_keys.sort();
                eprintln!("Tick {}: BorderCache has {} entries", tick_num, cache_map.len());
                eprintln!("  BorderCache keys (sorted): {:?}", cache_keys);
                if player_count <= 10 {
                    for (id, borders) in &cache_map {
                        eprintln!("    Player {}: {} border tiles", id.get(), borders.len());
                    }
                }
            }
        }

        // Print current turn for debugging
        if let Some(current_turn) = game.world().get_resource::<CurrentTurn>() {
            if tick_num <= 5 {
                eprintln!("  CurrentTurn = Turn({})", current_turn.turn.turn_number);
            }
        } else if tick_num < 10 {
            eprintln!("Tick {}: No CurrentTurn resource", tick_num);
        }
    }

    // Collect final statistics
    let final_turn = game.world().get_resource::<CurrentTurn>().map(|ct| ct.turn.turn_number).unwrap_or(0);

    // Calculate territory ownership, border tiles, and troop levels from ECS components
    use borders_core::game::BorderTiles;
    use borders_core::game::ai::bot::Bot;
    use borders_core::game::entities::{TerritorySize, Troops};
    let (total_territory_owned, bots_with_territory, bots_with_borders, avg_troops) = {
        let world = game.world_mut();
        let mut total = 0u32;
        let mut bots_count = 0usize;
        let mut borders_count = 0usize;
        let mut total_troops = 0.0f32;
        let mut bot_count = 0;

        for (_, territory_size, border_tiles, troops) in world.query::<(&Bot, &TerritorySize, &BorderTiles, &Troops)>().iter(world) {
            total += territory_size.0;
            if territory_size.0 > 0 {
                bots_count += 1;
            }
            if !border_tiles.0.is_empty() {
                borders_count += 1;
            }
            total_troops += troops.0;
            bot_count += 1;
        }
        let avg = if bot_count > 0 { total_troops / bot_count as f32 } else { 0.0 };
        (total, bots_count, borders_count, avg)
    };

    // Calculate how many bots expanded their territory (proxy for activity)
    let final_territory_sizes: HashMap<_, _> = {
        use borders_core::game::ai::bot::Bot;
        use borders_core::game::entities::TerritorySize;
        let world = game.world_mut();
        let mut query = world.query::<(&Bot, &NationId, &TerritorySize)>();
        query.iter(world).map(|(_, &id, size)| (id, size.0)).collect()
    };

    let active_bots = final_territory_sizes
        .iter()
        .filter(|(id, final_size)| {
            let initial_size = initial_territory_sizes.get(id).copied().unwrap_or(0);
            **final_size > initial_size + 10 // Expanded by more than 10 tiles
        })
        .count();

    let territory_changes: usize = final_territory_sizes
        .iter()
        .map(|(id, final_size)| {
            let initial_size = initial_territory_sizes.get(id).copied().unwrap_or(0);
            final_size.saturating_sub(initial_size) as usize
        })
        .sum();

    eprintln!("=== Final Game Stats ===");
    eprintln!("Bots with territory: {}/{}", bots_with_territory, player_count);
    eprintln!("Bots with border tiles: {}/{}", bots_with_borders, player_count);
    eprintln!("Average bot troops: {:.1}", avg_troops);
    eprintln!("Bots that expanded territory: {}/{}", active_bots, player_count);
    eprintln!("Total territory owned: {}", total_territory_owned);
    eprintln!("Total territory gained: {}", territory_changes);

    GameStats { final_turn, total_territory_owned, bots_with_territory, active_bots, total_bot_actions: territory_changes }
}

/// Test game stability across different player counts and map sizes
///
/// Verifies that games run without crashing and maintain basic invariants:
/// - Game progresses beyond turn 0
/// - Bots claim and expand territory
/// - Territory ownership is tracked correctly
#[rstest]
#[case::tiny(10, (50, 50))] // 2,500 tiles ≈ 250 per player
#[case::small(25, (80, 80))] // 6,400 tiles ≈ 256 per player
#[case::medium_small(50, (115, 115))] // 13,225 tiles ≈ 264 per player
#[case::medium(100, (160, 160))] // 25,600 tiles ≈ 256 per player
#[case::large(250, (255, 255))] // 65,025 tiles ≈ 260 per player
#[case::xl(500, (360, 360))] // 129,600 tiles ≈ 259 per player
fn smoke_test_player_scaling(#[case] player_count: usize, #[case] map_size: (u16, u16)) {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"))).with_test_writer().try_init();

    let target_ticks = 600; // 60 seconds at 10 TPS
    let stats = run_full_game(player_count, map_size, target_ticks, None);

    eprintln!("{}-player game stats: {:#?}", player_count, stats);

    // Game should have progressed
    assert!(stats.final_turn > 0, "Game should have progressed past turn 0");

    // Bots should have claimed territory (spawns applied)
    assert!(stats.bots_with_territory > 0, "Bots should have claimed territory after spawning");

    // Total territory should be owned
    assert!(stats.total_territory_owned > 0, "Territory should be owned by players");

    // At least some bots should be active
    assert!(stats.active_bots > 0, "At least some bots should have expanded territory (active_bots: {}, total_territory_gained: {})", stats.active_bots, stats.total_bot_actions);
}

/// Test game stability across different simulation durations
///
/// Verifies that games run correctly for different time periods:
/// - Short runs (30 seconds)
/// - Standard runs (60 seconds)
/// - Extended runs (120 seconds)
#[rstest]
#[case::short(300)] // 30 seconds at 10 TPS
#[case::standard(600)] // 60 seconds at 10 TPS
#[case::long(1200)] // 120 seconds at 10 TPS
fn smoke_test_tick_duration(#[case] target_ticks: u64) {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"))).with_test_writer().try_init();

    let player_count = 100;
    let map_size = (160, 160); // Medium scale
    let stats = run_full_game(player_count, map_size, target_ticks, None);

    eprintln!("{}-tick game stats: {:#?}", target_ticks, stats);

    // Game should have progressed
    assert!(stats.final_turn > 0, "Game should have progressed past turn 0");

    // Bots should have claimed territory
    assert!(stats.bots_with_territory > 0, "Bots should have claimed territory after spawning");

    // Total territory should be owned
    assert!(stats.total_territory_owned > 0, "Territory should be owned by players");

    // At least some bots should be active
    assert!(stats.active_bots > 0, "At least some bots should have expanded territory");
}

/// Test determinism and robustness across different RNG seeds
///
/// Verifies that games with different random seeds all:
/// - Run without crashing
/// - Maintain basic game invariants
/// - Produce valid game states
#[rstest]
#[case::seed_1(0xDEADBEEF)]
#[case::seed_2(0xCAFEBABE)]
#[case::seed_3(0x8BADF00D)]
fn smoke_test_rng_seeds(#[case] rng_seed: u64) {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("trace"))).with_test_writer().try_init();

    let player_count = 100;
    let map_size = (160, 160); // Medium scale
    let target_ticks = 600; // 60 seconds at 10 TPS
    let stats = run_full_game(player_count, map_size, target_ticks, Some(rng_seed));

    eprintln!("RNG seed 0x{:X} game stats: {:#?}", rng_seed, stats);

    // Game should have progressed
    assert!(stats.final_turn > 0, "Game should have progressed past turn 0");

    // Bots should have claimed territory
    assert!(stats.bots_with_territory > 0, "Bots should have claimed territory after spawning");

    // Total territory should be owned
    assert!(stats.total_territory_owned > 0, "Territory should be owned by players");

    // At least some bots should be active
    assert!(stats.active_bots > 0, "At least some bots should have expanded territory");
}
