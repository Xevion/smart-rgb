/// Pure combat calculation functions
///
/// This module contains all combat mathematics extracted from the attack system.
/// All functions are pure (no side effects) and deterministic, making them
/// easy to test, reason about, and modify.
use glam::U16Vec2;

use crate::game::core::constants::combat::*;
use crate::game::world::TerritoryManager;

/// Parameters for combat result calculation
pub struct CombatParams<'a> {
    pub attacker_troops: f32,
    pub attacker_territory_size: usize,
    pub defender_troops: Option<f32>,
    pub defender_territory_size: Option<usize>,
    pub tile: U16Vec2,
    pub territory_manager: &'a TerritoryManager,
    pub width: u16,
}

/// Result of combat calculations for conquering one tile
#[derive(Debug, Clone, Copy)]
pub struct CombatResult {
    /// Troops lost by the attacker
    pub attacker_loss: f32,
    /// Troops lost by the defender
    pub defender_loss: f32,
    /// How much of the "tiles per tick" budget this conquest consumes
    pub tiles_per_tick_used: f32,
}

/// Sigmoid function for smooth scaling curves
///
/// Used for empire size balancing to create smooth transitions
/// rather than hard thresholds.
#[inline]
pub fn sigmoid(x: f32, decay_rate: f32, midpoint: f32) -> f32 {
    1.0 / (1.0 + (-(x - midpoint) * decay_rate).exp())
}

/// Calculate combat result for conquering one tile
///
/// This function determines troop losses and conquest cost based on:
/// - Attacker and defender troop counts and empire sizes
/// - Terrain properties (currently plains baseline)
/// - Empire size balancing (prevents snowballing)
/// - Defense structures (placeholder for future implementation)
pub fn calculate_combat_result(params: CombatParams) -> CombatResult {
    if let (Some(defender_troops), Some(defender_territory_size)) = (params.defender_troops, params.defender_territory_size) {
        // Attacking claimed territory

        // Base terrain values (plains baseline)
        let mut mag = BASE_MAG_PLAINS;
        let mut speed = BASE_SPEED_PLAINS;

        // Defense post check (placeholder - always false for now)
        let has_defense_post = check_defense_post_nearby(params.tile, params.territory_manager);
        if has_defense_post {
            mag *= DEFENSE_POST_MAG_MULTIPLIER;
            speed *= DEFENSE_POST_SPEED_MULTIPLIER;
        }

        // Empire size balancing - prevents snowballing
        // Large defenders get debuffed, large attackers get penalized
        let defense_sig = 1.0 - sigmoid(defender_territory_size as f32, DEFENSE_DEBUFF_DECAY_RATE, DEFENSE_DEBUFF_MIDPOINT);
        let large_defender_speed_debuff = LARGE_DEFENDER_BASE_DEBUFF + LARGE_DEFENDER_SCALING * defense_sig;
        let large_defender_attack_debuff = LARGE_DEFENDER_BASE_DEBUFF + LARGE_DEFENDER_SCALING * defense_sig;

        let large_attacker_bonus = if params.attacker_territory_size > LARGE_EMPIRE_THRESHOLD as usize { (LARGE_EMPIRE_THRESHOLD as f32 / params.attacker_territory_size as f32).sqrt().powf(LARGE_ATTACKER_POWER_EXPONENT) } else { 1.0 };

        let large_attacker_speed_bonus = if params.attacker_territory_size > LARGE_EMPIRE_THRESHOLD as usize { (LARGE_EMPIRE_THRESHOLD as f32 / params.attacker_territory_size as f32).powf(LARGE_ATTACKER_SPEED_EXPONENT) } else { 1.0 };

        // Calculate troop ratio
        let troop_ratio = (defender_troops / params.attacker_troops.max(1.0)).clamp(TROOP_RATIO_MIN, TROOP_RATIO_MAX);

        // Final attacker loss
        let attacker_loss = troop_ratio * mag * ATTACKER_LOSS_MULTIPLIER * large_defender_attack_debuff * large_attacker_bonus;

        // Defender loss (simple: troops per tile)
        let defender_loss = defender_troops / defender_territory_size.max(1) as f32;

        // Tiles per tick cost for this tile
        let tiles_per_tick_used = (defender_troops / (TILES_PER_TICK_DIVISOR * params.attacker_troops.max(1.0))).clamp(TILES_PER_TICK_MIN, TILES_PER_TICK_MAX) * speed * large_defender_speed_debuff * large_attacker_speed_bonus;

        CombatResult { attacker_loss, defender_loss, tiles_per_tick_used }
    } else {
        // Attacking unclaimed territory
        CombatResult { attacker_loss: BASE_MAG_PLAINS / UNCLAIMED_ATTACK_LOSS_DIVISOR, defender_loss: 0.0, tiles_per_tick_used: ((UNCLAIMED_BASE_MULTIPLIER * BASE_SPEED_PLAINS.max(MIN_SPEED_PLAINS)) / params.attacker_troops.max(1.0)).clamp(UNCLAIMED_TILES_MIN, UNCLAIMED_TILES_MAX) }
    }
}

/// Calculate tiles conquered per tick based on troop ratio and border size
///
/// This determines how fast an attack progresses. It's based on:
/// - The attacker's troop advantage (or disadvantage)
/// - The size of the attack border
/// - Random variation for organic-looking expansion
pub fn calculate_tiles_per_tick(attacker_troops: f32, defender_troops: Option<f32>, border_size: f32) -> f32 {
    if let Some(defender_troops) = defender_troops {
        // Dynamic based on troop ratio
        let ratio = ((ATTACK_RATIO_MULTIPLIER * attacker_troops) / defender_troops.max(1.0)) * ATTACK_RATIO_SCALE;
        let clamped_ratio = ratio.clamp(ATTACK_RATIO_MIN, ATTACK_RATIO_MAX);
        clamped_ratio * border_size * CLAIMED_TILES_PER_TICK_MULTIPLIER
    } else {
        // Fixed rate for unclaimed territory
        border_size * UNCLAIMED_TILES_PER_TICK_MULTIPLIER
    }
}

/// Check if defender has a defense post nearby (placeholder)
///
/// This will be implemented when defense structures are added to the game.
/// For now, always returns false.
fn check_defense_post_nearby(_tile: U16Vec2, _territory_manager: &TerritoryManager) -> bool {
    // Placeholder for future defense post implementation
    false
}
