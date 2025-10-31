use bevy_ecs::prelude::*;
use bevy_ecs::schedule::IntoScheduleConfigs;
use glam::U16Vec2;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use std::sync::{Arc, atomic::AtomicBool};
use tracing::{debug, info};

use crate::game::{Game, NationId, ai::bot, core::constants, input};
use crate::{game, networking, time, ui};

/// Builder for Game initialization with unified configuration
///
/// Provides a fluent API for constructing fully-initialized Game instances with:
/// - Map/terrain configuration (required)
/// - Nations configuration (bots, local player)
/// - Network mode (required)
/// - Optional frontend integration
/// - Time system configuration
/// - Spawn phase settings
/// - Deterministic RNG seeding
/// - System toggle for unit tests
///
/// # Examples
///
/// Production desktop game:
/// ```ignore
/// let game = GameBuilder::new()
///     .with_map(terrain_data)
///     .with_bots(500)
///     .with_local_player(NationId::ZERO)
///     .with_network(NetworkMode::Local)
///     .with_frontend(TauriTransport::new())
///     .build();
/// ```
///
/// Integration test:
/// ```ignore
/// let terrain = MapBuilder::new(100, 100).all_conquerable().build();
/// let game = GameBuilder::new()
///     .with_map(terrain)
///     .with_bots(20)
///     .with_network(NetworkMode::Local)
///     .with_spawn_phase(None)
///     .with_rng_seed(42)
///     .build();
/// ```
pub struct GameBuilder {
    // Required
    terrain_data: Option<Arc<game::TerrainData>>,
    network_mode: Option<networking::NetworkMode>,

    // Optional with defaults
    bot_count: u32,
    local_player_id: Option<NationId>,
    frontend_transport: Option<Arc<dyn ui::FrontendTransport>>,
    tick_rate: u32,
    clock: Option<time::Clock>,
    spawn_timeout_secs: Option<u32>,
    rng_seed: Option<u64>,
    enable_systems: bool,

    // Input system (Arc allows sharing across game instances on desktop)
    input_queue: Arc<game::InputQueue>,
}

impl GameBuilder {
    /// Create a new GameBuilder with default configuration
    ///
    /// Defaults:
    /// - bot_count: 0
    /// - local_player_id: None (headless)
    /// - tick_rate: 10 TPS
    /// - clock: real-time clock
    /// - spawn_timeout_secs: Some(60)
    /// - rng_seed: random
    /// - enable_systems: true
    pub fn new() -> Self {
        Self { terrain_data: None, network_mode: None, bot_count: 0, local_player_id: None, frontend_transport: None, tick_rate: 10, clock: None, spawn_timeout_secs: Some(60), rng_seed: None, enable_systems: true, input_queue: Arc::new(game::InputQueue::new()) }
    }

    /// Add frontend transport for UI integration
    pub fn with_frontend(mut self, transport: impl ui::FrontendTransport + 'static) -> Self {
        self.frontend_transport = Some(Arc::new(transport));
        self
    }

    /// Set terrain/map data (required)
    pub fn with_map(mut self, terrain: Arc<game::TerrainData>) -> Self {
        self.terrain_data = Some(terrain);
        self
    }

    /// Set number of bot nations (default: 0)
    pub fn with_bots(mut self, count: u32) -> Self {
        self.bot_count = count;
        self
    }

    /// Set local player ID (default: None for headless)
    pub fn with_local_player(mut self, id: NationId) -> Self {
        self.local_player_id = Some(id);
        self
    }

    /// Set network mode (required)
    pub fn with_network(mut self, mode: networking::NetworkMode) -> Self {
        self.network_mode = Some(mode);
        self
    }

    /// Set tick rate in ticks per second (default: 10 TPS)
    pub fn with_tick_rate(mut self, tps: u32) -> Self {
        self.tick_rate = tps;
        self
    }

    /// Set custom clock for time system (default: real-time clock)
    pub fn with_clock(mut self, clock: time::Clock) -> Self {
        self.clock = Some(clock);
        self
    }

    /// Set spawn phase timeout in seconds (default: Some(60), None to skip spawn phase)
    pub fn with_spawn_phase(mut self, timeout_secs: Option<u32>) -> Self {
        self.spawn_timeout_secs = timeout_secs;
        self
    }

    /// Set RNG seed for deterministic gameplay (default: random)
    pub fn with_rng_seed(mut self, seed: u64) -> Self {
        self.rng_seed = Some(seed);
        self
    }

    /// Enable or disable systems (default: true, set to false for unit tests)
    pub fn with_systems(mut self, enable: bool) -> Self {
        self.enable_systems = enable;
        self
    }

    /// Get a sender for platforms to send input events
    ///
    /// Platforms should call this method to get a flume Sender that can be used
    /// to send InputEvents into the game's input queue. The queue is automatically
    /// processed at the start of each frame during Game::update().
    pub fn input_sender(&self) -> flume::Sender<input::InputEvent> {
        self.input_queue.sender()
    }

    /// Use an existing input queue instead of creating a new one
    ///
    /// This allows platforms to create the input queue once and reuse it across
    /// multiple game instances (e.g., after QuitGame/StartGame cycles on desktop).
    pub fn with_input_queue(mut self, queue: Arc<game::InputQueue>) -> Self {
        self.input_queue = queue;
        self
    }

    /// Build and initialize a Game instance
    pub fn build(self) -> Game {
        let terrain_data = self.terrain_data.expect("Map/terrain data is required - call .with_map()");
        let network_mode = self.network_mode.expect("Network mode is required - call .with_network()");

        info!("Creating Game with GameBuilder...");

        let mut game = Game::new_internal();

        let time = if let Some(clock) = self.clock { time::Time::with_clock(clock, 0) } else { time::Time::new() };
        game.insert_resource(time);
        game.insert_resource(time::FixedTime::from_seconds(1.0 / self.tick_rate as f64));

        let map_size = terrain_data.size();
        let _guard = tracing::trace_span!(
            "game_initialization",
            map_size = ?map_size,
        )
        .entered();

        let conquerable_tiles: Vec<bool> = (0..map_size.y)
            .flat_map(|y| {
                let terrain = terrain_data.clone();
                (0..map_size.x).map(move |x| terrain.is_conquerable(U16Vec2::new(x, y)))
            })
            .collect();

        let client_player_id = self.local_player_id.unwrap_or(NationId::ZERO);
        let player_count = if self.local_player_id.is_some() { 1 } else { 0 };
        let bot_count_usize = self.bot_count as usize;
        let nation_count = player_count + bot_count_usize;

        let rng_seed = self.rng_seed.unwrap_or_else(rand::random::<u64>);
        let mut rng = StdRng::seed_from_u64(rng_seed);

        let mut nation_metadata = Vec::new();
        let hue_offset = rng.random_range(0.0..360.0);

        for i in 0..nation_count {
            let is_human = i < player_count;
            let nation_id = NationId::new(i as u16).expect("valid player ID");

            let hue = (nation_id.get() as f32 * constants::colors::GOLDEN_ANGLE + hue_offset) % 360.0;
            let saturation = rng.random_range(constants::colors::SATURATION_MIN..=constants::colors::SATURATION_MAX);
            let lightness = rng.random_range(constants::colors::LIGHTNESS_MIN..=constants::colors::LIGHTNESS_MAX);
            let color = game::HSLColor::new(hue, saturation, lightness);

            let name = if is_human { if player_count == 1 { "Player".to_string() } else { format!("Player {}", i + 1) } } else { format!("Bot {}", i - player_count + 1) };

            nation_metadata.push((nation_id, name, color));
        }

        let mut territory_manager = game::TerritoryManager::new(map_size);
        territory_manager.reset(map_size, &conquerable_tiles);
        debug!("Territory manager initialized with {} tiles", conquerable_tiles.len());

        let mut active_attacks = game::ActiveAttacks::new();
        active_attacks.init();

        let bot_ids: Vec<NationId> = nation_metadata.iter().skip(player_count).map(|(id, _, _)| *id).collect();
        let initial_bot_spawns = bot::calculate_initial_spawns(&bot_ids, &territory_manager, &terrain_data, rng_seed);

        if initial_bot_spawns.len() < bot_count_usize {
            tracing::warn!("Only {} of {} bots were able to spawn - map may be too small or bot count too high", initial_bot_spawns.len(), bot_count_usize);
        }

        let mut nation_entity_map = game::NationEntityMap::default();
        let initial_territory_size = 0;

        for (nation_id, name, color) in &nation_metadata {
            let is_bot = bot_ids.contains(nation_id);

            let entity = if is_bot {
                let bot_seed = rng_seed.wrapping_add(nation_id.get() as u64);
                game.world_mut().spawn((bot::Bot::with_seed(bot_seed), *nation_id, game::NationName(name.clone()), game::NationColor(*color), game::BorderTiles::default(), game::Troops(constants::nation::INITIAL_TROOPS), game::TerritorySize(initial_territory_size), game::ships::ShipCount::default())).id()
            } else {
                game.world_mut().spawn((*nation_id, game::NationName(name.clone()), game::NationColor(*color), game::BorderTiles::default(), game::Troops(constants::nation::INITIAL_TROOPS), game::TerritorySize(initial_territory_size), game::ships::ShipCount::default())).id()
            };

            nation_entity_map.0.insert(*nation_id, entity);
        }

        let coastal_tiles = game::CoastalTiles::compute(&terrain_data, map_size);

        let world = game.world_mut();
        world.insert_resource(nation_entity_map);
        world.insert_resource(territory_manager);
        world.insert_resource(active_attacks);
        world.insert_resource(terrain_data.as_ref().clone());
        world.insert_resource(coastal_tiles);
        world.insert_resource(game::SpawnManager::new(initial_bot_spawns.clone(), rng_seed));
        world.insert_resource(game::ShipIdCounter::new());
        world.insert_resource(game::DeterministicRng::new(rng_seed));
        world.insert_resource(game::LocalPlayerContext::new(client_player_id));

        // Initialize CurrentTurn with turn 0 - this resource must always exist
        world.insert_resource(game::CurrentTurn::new(networking::Turn { turn_number: 0, intents: Vec::new() }));

        let skip_spawn_phase = self.spawn_timeout_secs.is_none();
        let spawn_timeout_secs_f32 = self.spawn_timeout_secs.unwrap_or(60) as f32;

        world.insert_resource(game::SpawnTimeout::new(spawn_timeout_secs_f32));
        debug!("SpawnTimeout initialized ({} seconds)", spawn_timeout_secs_f32);

        world.insert_resource(game::SpawnPhase { active: !skip_spawn_phase });

        let (turn_tx, turn_rx) = flume::unbounded();
        world.insert_resource(networking::server::LocalTurnServerHandle { paused: Arc::new(AtomicBool::new(!skip_spawn_phase)), running: Arc::new(AtomicBool::new(true)) });
        world.insert_resource(networking::server::TurnReceiver { turn_rx });
        world.insert_resource(networking::server::TurnGenerator::new(turn_tx));

        if self.enable_systems {
            let _guard = tracing::debug_span!("game_plugin_build").entered();

            game.add_message::<networking::IntentEvent>().add_message::<networking::ProcessTurnEvent>().add_message::<networking::SpawnConfigEvent>().add_message::<game::ships::LaunchShipMessage>().add_message::<game::ships::ShipArrivalMessage>();

            game.add_message::<input::MouseButtonMessage>().add_message::<game::input::MouseMotionMessage>().add_message::<input::KeyEventMessage>().add_message::<input::TileClickedAction>().add_message::<input::CameraAction>().add_message::<input::UiAction>().add_message::<ui::protocol::BackendMessage>().add_message::<ui::protocol::FrontendMessage>();

            game.init_resource::<ui::LastLeaderboardDigest>().init_resource::<ui::LastAttacksDigest>().init_resource::<ui::LeaderboardThrottle>().init_resource::<ui::DisplayOrderUpdateCounter>().init_resource::<ui::LastDisplayOrder>().init_resource::<ui::NationHighlightState>().init_resource::<ui::ShipStateTracker>();
            game.init_resource::<input::SpawnPhase>().init_resource::<input::AttackControls>().init_resource::<game::systems::BorderCache>().init_resource::<game::builder::PreviousSpawnState>();

            match &network_mode {
                networking::NetworkMode::Local => {
                    let _guard = tracing::trace_span!("network_setup", mode = "local").entered();
                    info!("Initializing game in Local mode");

                    let (tracked_intent_tx, tracked_intent_rx) = flume::unbounded();
                    let (_placeholder_tx, placeholder_rx) = flume::unbounded();

                    let backend = networking::client::LocalBackend::new(tracked_intent_tx, placeholder_rx, NationId::ZERO);
                    let connection = networking::client::Connection::new_local(backend);

                    game.insert_resource(connection).insert_resource(networking::client::IntentReceiver { rx: tracked_intent_rx });
                }
                #[cfg(not(target_arch = "wasm32"))]
                networking::NetworkMode::Remote { server_address: _ } => {
                    unimplemented!("Remote networking temporarily disabled");
                }
            }

            game.add_systems(game::Update, input::input_processor_system);

            game.add_systems(game::Update, game::systems::manage_spawn_phase_system);

            game.add_systems(game::Update, (game::systems::update_current_turn_system, game::systems::process_nation_income_system).chain());

            game.add_systems(game::Update, (game::systems::process_and_apply_actions_system, game::systems::tick_attacks_system, game::systems::handle_spawns_system, game::launch_ship_system).chain().after(game::systems::update_current_turn_system));

            game.add_systems(game::Update, (game::update_ships_system, game::handle_ship_arrivals_system, game::check_local_player_outcome, game::update_nation_borders_system).chain().after(game::launch_ship_system));

            game.add_systems(game::Update, (ui::emit_leaderboard_snapshot_system, ui::emit_attacks_update_system, ui::emit_ships_update_system, ui::emit_nation_highlight_system));

            game.add_systems(game::Update, ui::protocol::handle_frontend_messages_system);

            game.add_systems(game::Update, (input::handle_tile_clicked_system, input::handle_camera_action_system, input::handle_ui_action_system).after(input::input_processor_system));

            game.add_systems(game::Update, networking::server::generate_turns_system.before(networking::server::poll_turns_system));

            match &network_mode {
                networking::NetworkMode::Local => {
                    game.add_systems(game::Update, (networking::server::poll_turns_system.before(game::systems::update_current_turn_system), networking::client::send_intent_system));
                }
                #[cfg(not(target_arch = "wasm32"))]
                networking::NetworkMode::Remote { .. } => {}
            }

            game.add_systems(game::Last, (game::clear_territory_changes_system, game::systems::turn_cleanup_system).chain());

            if let Some(transport) = self.frontend_transport {
                let _guard = tracing::trace_span!("frontend_plugin_build").entered();

                game.insert_resource(ui::RenderBridge::new(transport));

                game.add_systems(game::Update, (ui::send_initial_render_data, ui::stream_territory_deltas, ui::stream_spawn_preview_deltas).chain().after(game::systems::update_nation_borders_system));

                game.add_systems(game::Update, (ui::emit_backend_messages_system, ui::ingest_frontend_messages_system));
            }
        }

        game.input_queue = Some(self.input_queue);

        if self.enable_systems {
            game.run_startup();
            game.finish();
        }

        info!("Game created and initialized successfully");
        game
    }
}

impl Default for GameBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Resource to track previous spawn state for incremental updates
#[derive(Resource, Default)]
pub struct PreviousSpawnState {
    pub spawns: Vec<game::SpawnPoint>,
}
