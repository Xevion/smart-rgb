use crate::game::NationId;
use crate::networking::{Intent, SourcedIntent, Turn};
use glam::U16Vec2;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Spawn timeout duration (milliseconds)
const SPAWN_TIMEOUT_MS: f64 = 5000.0;

/// Output from the turn generator
#[derive(Debug, Clone)]
pub enum TurnOutput {
    /// No output this tick
    None,
    /// Spawn configuration was updated
    SpawnUpdate(HashMap<NationId, U16Vec2>),
    /// Game turn (includes initial Turn 0)
    Turn(Turn),
}

/// Shared turn generation logic for both local coordinator and relay server
pub struct SharedTurnGenerator {
    turn_number: u64,
    accumulated_time: f64,                // milliseconds
    buffered_intents: Vec<SourcedIntent>, // Buffer intents across frames
    spawn_config: HashMap<NationId, U16Vec2>,
    spawn_timeout_accumulated: Option<f64>, // milliseconds since first spawn
    game_started: bool,
}

impl SharedTurnGenerator {
    pub fn new() -> Self {
        Self { turn_number: 0, accumulated_time: 0.0, buffered_intents: Vec::new(), spawn_config: HashMap::new(), spawn_timeout_accumulated: None, game_started: false }
    }

    /// Process a single sourced intent, returns output if spawn config changed
    pub fn process_intent(&mut self, sourced_intent: SourcedIntent) -> TurnOutput {
        match sourced_intent.intent {
            Intent::SetSpawn { tile_index } => {
                if self.game_started {
                    warn!("Received SetSpawn intent after game started - ignoring");
                    return TurnOutput::None;
                }

                let nation_id = sourced_intent.source;
                debug!("Nation {} set spawn at tile {}", nation_id, tile_index);
                self.spawn_config.insert(nation_id, tile_index);

                // Start timeout on first spawn
                if self.spawn_timeout_accumulated.is_none() {
                    self.spawn_timeout_accumulated = Some(0.0);
                    debug!("Spawn timeout started ({}ms)", SPAWN_TIMEOUT_MS);
                }

                TurnOutput::SpawnUpdate(self.spawn_config.clone())
            }
            Intent::Action(_) => {
                if !self.game_started {
                    warn!("Received Action intent during spawn phase - ignoring");
                }
                TurnOutput::None
            }
        }
    }

    /// Tick with delta time (ms), returns turn if ready
    /// During spawn phase, checks timeout. During game phase, accumulates time and generates turns.
    pub fn tick(&mut self, delta_ms: f64, sourced_intents: Vec<SourcedIntent>) -> TurnOutput {
        // Buffer incoming intents for the next turn (filter out SetSpawn during game phase)
        if self.game_started {
            for sourced_intent in sourced_intents {
                match sourced_intent.intent {
                    Intent::Action(_) => self.buffered_intents.push(sourced_intent),
                    Intent::SetSpawn { .. } => {
                        warn!("Received SetSpawn intent after game started - ignoring");
                    }
                }
            }
        }

        // If game started and we're at turn 0, emit the initial turn immediately
        if self.game_started && self.turn_number == 0 {
            let start_turn = Turn { turn_number: 0, intents: Vec::new() };

            self.turn_number = 1; // Next turn will be turn 1
            info!("Turn(0) emitted - game starting");
            return TurnOutput::Turn(start_turn);
        }

        // During spawn phase, handle timeout
        if !self.game_started {
            if let Some(ref mut accumulated) = self.spawn_timeout_accumulated {
                *accumulated += delta_ms;

                // Check if timeout expired
                if *accumulated >= SPAWN_TIMEOUT_MS {
                    debug!("Spawn timeout expired - starting game");

                    // Create Turn(0) to start game
                    let start_turn = Turn { turn_number: 0, intents: Vec::new() };

                    info!("Turn(0) ready to start game (spawns already configured)");

                    // Mark game as started and clear spawn phase
                    self.game_started = true;
                    self.spawn_config.clear();
                    self.spawn_timeout_accumulated = None;
                    self.turn_number = 1; // Next turn will be turn 1
                    self.accumulated_time = 0.0; // Reset for clean turn timing

                    info!("Spawn phase complete - game started, next turn will be Turn 1");

                    return TurnOutput::Turn(start_turn);
                }
            }

            return TurnOutput::None;
        }

        // Normal turn generation (after game has started)
        self.accumulated_time += delta_ms;

        // Only generate turn if enough time has passed (100ms tick interval)
        if self.accumulated_time < 100.0 {
            return TurnOutput::None;
        }

        // Reset accumulated time
        self.accumulated_time -= 100.0;

        // Drain buffered intents into the turn
        let turn_intents = std::mem::take(&mut self.buffered_intents);

        // Create turn
        let turn = Turn { turn_number: self.turn_number, intents: turn_intents };

        self.turn_number += 1;

        TurnOutput::Turn(turn)
    }

    /// Get current turn number
    pub fn turn_number(&self) -> u64 {
        self.turn_number
    }

    /// Check if game has started
    pub fn game_started(&self) -> bool {
        self.game_started
    }

    /// Get current spawn configuration
    pub fn spawn_config(&self) -> &HashMap<NationId, U16Vec2> {
        &self.spawn_config
    }

    /// Skip spawn phase and start the game immediately
    /// The next call to tick() will emit Turn(0) to start the game
    /// Useful for tests that don't need the spawn phase
    pub fn skip_spawn_phase(&mut self) {
        self.game_started = true;
        self.turn_number = 0; // Next tick will emit Turn(0), then proceed to Turn(1)
        self.spawn_timeout_accumulated = None;
        self.spawn_config.clear();
        self.buffered_intents.clear(); // Clear any buffered intents
    }
}

impl Default for SharedTurnGenerator {
    fn default() -> Self {
        Self::new()
    }
}
