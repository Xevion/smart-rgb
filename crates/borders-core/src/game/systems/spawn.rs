use bevy_ecs::prelude::*;

use crate::game::NationId;

/// Represents a spawn point for a nation (player or bot)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpawnPoint {
    pub nation: NationId,
    pub tile: glam::U16Vec2,
}

impl SpawnPoint {
    pub fn new(nation: NationId, tile: glam::U16Vec2) -> Self {
        Self { nation, tile }
    }
}

/// Manages spawn positions during the pre-game spawn phase
///
/// This resource tracks bot and nation spawn positions before the game starts ticking.
/// It allows for dynamic recalculation of bot positions when nations change their spawn
/// location, implementing the two-pass spawn system described in the README.
#[derive(Resource)]
pub struct SpawnManager {
    /// Initial bot spawn positions from first pass
    pub initial_bot_spawns: Vec<SpawnPoint>,

    /// Current bot spawn positions after recalculation
    /// These are updated whenever a player chooses/changes their spawn
    pub current_bot_spawns: Vec<SpawnPoint>,

    /// Nation spawn positions
    /// Tracks human nation spawn selections
    pub player_spawns: Vec<SpawnPoint>,

    /// RNG seed for deterministic spawn calculations
    pub rng_seed: u64,
}

impl SpawnManager {
    /// Create a new SpawnManager with initial bot spawns
    pub fn new(initial_bot_spawns: Vec<SpawnPoint>, rng_seed: u64) -> Self {
        Self { current_bot_spawns: initial_bot_spawns.clone(), initial_bot_spawns, player_spawns: Vec::new(), rng_seed }
    }

    /// Update a nation's spawn position and recalculate bot spawns if necessary
    ///
    /// This triggers the second pass of the two-pass spawn system, relocating
    /// any bots that are too close to the new nation position.
    pub fn update_player_spawn(&mut self, nation_id: NationId, tile_index: glam::U16Vec2, territory_manager: &crate::game::TerritoryManager, terrain: &crate::game::terrain::TerrainData) {
        let spawn_point = SpawnPoint::new(nation_id, tile_index);

        // Update or add nation spawn
        if let Some(entry) = self.player_spawns.iter_mut().find(|spawn| spawn.nation == nation_id) {
            *entry = spawn_point;
        } else {
            self.player_spawns.push(spawn_point);
        }

        // Recalculate bot spawns with updated nation positions
        self.current_bot_spawns = crate::game::ai::bot::recalculate_spawns_with_players(self.initial_bot_spawns.clone(), &self.player_spawns, territory_manager, terrain, self.rng_seed);
    }

    /// Get all current spawn positions (nations + bots)
    pub fn get_all_spawns(&self) -> Vec<SpawnPoint> {
        let mut all_spawns = self.player_spawns.clone();
        all_spawns.extend(self.current_bot_spawns.iter().copied());
        all_spawns
    }

    /// Get only bot spawn positions
    pub fn get_bot_spawns(&self) -> &[SpawnPoint] {
        &self.current_bot_spawns
    }

    /// Get only nation spawn positions
    pub fn get_player_spawns(&self) -> &[SpawnPoint] {
        &self.player_spawns
    }
}
