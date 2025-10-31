use bevy_ecs::prelude::*;
use rand::SeedableRng;
use rand::rngs::StdRng;

use crate::game::NationId;

/// Centralized deterministic RNG resource
///
/// This resource provides deterministic random number generation for all game systems.
/// It is updated at the start of each turn with the current turn number, ensuring that
/// the same sequence of turns always produces the same random values.
///
/// # Determinism Guarantees
///
/// - Same turn number + base seed + context → same RNG state
/// - No stored RNG state in individual systems (prevents desync)
/// - All randomness flows through this single source of truth
///
/// # Usage
///
/// Systems should never store RNG state. Instead, request context-specific RNG:
///
/// ```rust,ignore
/// fn my_system(rng: Res<DeterministicRng>) {
///     let mut player_rng = rng.for_player(player_id);
///     let random_value = player_rng.gen_range(0..10);
/// }
/// ```
#[derive(Resource)]
pub struct DeterministicRng {
    /// Base seed for the entire game (set at game start)
    base_seed: u64,
    /// Current turn number (updated each turn)
    turn_number: u64,
}

impl DeterministicRng {
    /// Create a new DeterministicRng with a base seed
    pub fn new(base_seed: u64) -> Self {
        Self { base_seed, turn_number: 0 }
    }

    /// Update the turn number (should be called at start of each turn)
    pub fn update_turn(&mut self, turn_number: u64) {
        self.turn_number = turn_number;
    }

    /// Get the current turn number
    #[inline]
    pub fn turn_number(&self) -> u64 {
        self.turn_number
    }

    /// Create an RNG for a specific context within the current turn
    ///
    /// The context_id allows different systems/entities to have independent
    /// random sequences while maintaining determinism.
    #[inline]
    pub fn for_context(&self, context_id: u64) -> StdRng {
        let seed = self
            .turn_number
            .wrapping_mul(997) // Prime multiplier for turn
            .wrapping_add(self.base_seed)
            .wrapping_add(context_id.wrapping_mul(1009)); // Prime multiplier for context
        StdRng::seed_from_u64(seed)
    }

    /// Get an RNG for a specific nation's actions this turn
    ///
    /// This is a convenience wrapper around `for_context` for nation-specific randomness.
    #[inline]
    pub fn for_nation(&self, id: NationId) -> StdRng {
        self.for_context(id.get() as u64)
    }

    /// Get an RNG for a specific tile's calculations this turn
    ///
    /// Useful for tile-based randomness that should be consistent within a turn.
    pub fn for_tile(&self, tile: glam::U16Vec2) -> StdRng {
        // Use large offset to avoid collision with nation IDs
        // Convert tile position to unique ID
        let tile_id = (tile.y as u64) * u16::MAX as u64 + (tile.x as u64);
        self.for_context(1_000_000 + tile_id)
    }
}
