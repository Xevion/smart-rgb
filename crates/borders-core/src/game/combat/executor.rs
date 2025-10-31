/// Attack execution logic
///
/// This module contains the `AttackExecutor` which manages the progression
/// of a single attack over multiple turns. It handles tile prioritization,
/// border expansion, and conquest mechanics.
use std::collections::{BinaryHeap, HashMap, HashSet};

use glam::U16Vec2;
use rand::Rng;

use super::calculator::{CombatParams, calculate_combat_result, calculate_tiles_per_tick};
use crate::game::core::constants::combat::*;
use crate::game::core::rng::DeterministicRng;
use crate::game::core::utils::neighbors;
use crate::game::entities::{NationEntityMap, TerritorySize, Troops};
use crate::game::terrain::TerrainData;
use crate::game::world::{NationId, TerritoryManager};
use bevy_ecs::prelude::*;

/// Priority queue entry for tile conquest
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TilePriority {
    tile: U16Vec2,
    priority: i64, // Lower value = higher priority (conquered sooner)
}

impl PartialOrd for TilePriority {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for TilePriority {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        other.priority.cmp(&self.priority).then_with(|| self.tile.x.cmp(&other.tile.x).then_with(|| self.tile.y.cmp(&other.tile.y)))
    }
}

/// Configuration for creating an AttackExecutor
pub struct AttackConfig<'a> {
    pub id: u64,
    pub source: NationId,
    pub target: Option<NationId>,
    pub troops: f32,
    pub border_tiles: Option<&'a HashSet<U16Vec2>>,
    pub territory_manager: &'a TerritoryManager,
    pub nation_borders: &'a HashMap<NationId, &'a HashSet<U16Vec2>>,
    pub turn_number: u64,
    pub terrain: &'a TerrainData,
}

/// Executes a single ongoing attack (conquering tiles over time)
///
/// An attack progresses over multiple turns, conquering tiles based on:
/// - Available troops
/// - Troop ratio vs defender
/// - Border size and connectivity
/// - Combat formulas from the calculator module
///
/// The executor maintains a priority queue of tiles to conquer and updates
/// borders as it progresses.
pub struct AttackExecutor {
    id: u64,
    pub source: NationId,
    pub target: Option<NationId>,
    troops: f32,
    /// Active conquest frontier - tiles being evaluated/conquered by this attack.
    /// Distinct from nation BorderTiles: dynamically shrinks as tiles are conquered
    /// and expands as new neighbors become targets.
    conquest_frontier: HashSet<U16Vec2>,
    priority_queue: BinaryHeap<TilePriority>,
    start_turn: u64,
    current_turn: u64,
    tiles_conquered: usize, // Counter for each tile conquered (for priority calculation)
    pending_removal: bool,  // Mark attack for removal on next tick (allows final troops=0 update)
}

impl AttackExecutor {
    /// Create a new attack executor
    pub fn new(config: AttackConfig, rng: &DeterministicRng) -> Self {
        let mut executor = Self { id: config.id, source: config.source, target: config.target, troops: config.troops, conquest_frontier: HashSet::new(), priority_queue: BinaryHeap::new(), start_turn: config.turn_number, current_turn: config.turn_number, tiles_conquered: 0, pending_removal: false };

        executor.initialize_border(config.border_tiles, config.territory_manager, config.terrain, config.nation_borders, rng);

        executor
    }

    /// Modify the amount of troops in the attack
    pub fn modify_troops(&mut self, amount: f32) {
        self.troops += amount;
    }

    /// Add new border tiles to the attack, allowing expansion from multiple fronts
    ///
    /// This enables multi-region expansion when attacking the same target from different areas
    pub fn add_borders(&mut self, new_border_tiles: &HashSet<U16Vec2>, territory_manager: &TerritoryManager, terrain: &TerrainData, rng: &DeterministicRng) {
        // Add neighbors from each new border tile
        for &tile in new_border_tiles {
            for neighbor in neighbors(tile, territory_manager.size()) {
                if self.is_valid_target(neighbor, territory_manager, terrain) && !self.conquest_frontier.contains(&neighbor) {
                    self.add_tile_to_border(neighbor, territory_manager, rng);
                }
            }
        }
    }

    /// Oppose an attack (counter-attack)
    ///
    /// Returns true if the attack continues, false if it was defeated
    pub fn oppose(&mut self, troop_count: f32) -> bool {
        if self.troops > troop_count {
            self.troops -= troop_count;
            true
        } else {
            false
        }
    }

    /// Get the unique attack identifier
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Get the amount of troops in the attack
    pub fn get_troops(&self) -> f32 {
        self.troops.max(0.0).floor()
    }

    /// Get the turn this attack started
    pub fn get_start_turn(&self) -> u64 {
        self.start_turn
    }

    /// Tick the attack executor
    ///
    /// Returns true if the attack continues, false if it's finished
    pub fn tick(&mut self, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>, territory_manager: &mut TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng: &DeterministicRng) -> bool {
        let _guard = tracing::trace_span!("attack_tick", nation_id = %self.source).entered();

        // If marked for removal, remove now (allows one final update with troops=0)
        if self.pending_removal {
            return false;
        }

        self.current_turn += 1;

        // Calculate how many tiles to conquer this tick
        let mut tiles_per_tick = self.calculate_tiles_per_tick(entity_map, nations, rng);

        // Track if we've already refreshed this tick to prevent infinite refresh loops
        let mut has_refreshed = false;

        // Process tiles from priority queue
        while tiles_per_tick > 0.0 {
            if self.troops < 1.0 {
                self.troops = 0.0;
                self.pending_removal = true;
                return true; // Keep alive for one more tick to send troops=0
            }

            if self.priority_queue.is_empty() {
                // If we already refreshed this tick, stop to prevent infinite loop
                if has_refreshed {
                    self.troops = 0.0;
                    self.pending_removal = true;
                    return true; // Keep alive for one more tick to send troops=0
                }

                // Remember border size before refresh
                let border_size_before = self.conquest_frontier.len();

                // Refresh border tiles one last time before giving up
                self.refresh_border(nation_borders, territory_manager, terrain, rng);
                has_refreshed = true;

                // If refresh found no new tiles, attack is finished
                if self.conquest_frontier.len() == border_size_before {
                    self.troops = 0.0;
                    self.pending_removal = true;
                    return true; // Keep alive for one more tick to send troops=0
                }

                // If still empty after refresh (all tiles invalid), attack is finished
                if self.priority_queue.is_empty() {
                    self.troops = 0.0;
                    self.pending_removal = true;
                    return true; // Keep alive for one more tick to send troops=0
                }
            }

            let tile_priority = self.priority_queue.pop().unwrap();
            let tile = tile_priority.tile;
            self.conquest_frontier.remove(&tile);

            // Check connectivity and validity
            let on_border = Self::check_borders_tile(tile, self.source, territory_manager);
            let tile_valid = self.is_valid_target(tile, territory_manager, terrain);

            // Prevent attacking own tiles (race condition during conquest)
            let tile_owner = territory_manager.get_ownership(tile);
            let attacking_self = tile_owner.nation_id() == Some(self.source);

            // Skip if any check fails
            if !tile_valid || !on_border || attacking_self {
                continue;
            }

            // Add neighbors BEFORE conquering (critical for correct expansion)
            self.add_neighbors_to_border(tile, territory_manager, terrain, rng);

            // Query attacker territory size from ECS
            let attacker_troops = self.troops;
            let attacker_territory_size = if let Some(&attacker_entity) = entity_map.0.get(&self.source)
                && let Ok((_, territory)) = nations.get(attacker_entity)
            {
                territory.0
            } else {
                // Attacker no longer exists - immediate removal (error state)
                return false;
            };

            // Query defender stats from ECS if attacking a player
            let (defender_troops, defender_territory_size) = if let Some(target_id) = self.target { if let Some(&defender_entity) = entity_map.0.get(&target_id) { if let Ok((troops, territory)) = nations.get(defender_entity) { (Some(troops.0), Some(territory.0)) } else { (None, None) } } else { (None, None) } } else { (None, None) };

            // Calculate losses for this tile
            let combat_result = { calculate_combat_result(CombatParams { attacker_troops, attacker_territory_size: attacker_territory_size as usize, defender_troops, defender_territory_size: defender_territory_size.map(|s| s as usize), tile, territory_manager, width: territory_manager.width() }) };

            // Check if we still have enough troops to conquer this tile
            if self.troops < combat_result.attacker_loss {
                self.troops = 0.0;
                self.pending_removal = true;
                return true; // Keep alive for one more tick to send troops=0
            }

            // Apply troop losses
            self.troops -= combat_result.attacker_loss;
            if let Some(target_id) = self.target
                && let Some(&defender_entity) = entity_map.0.get(&target_id)
                && let Ok((mut troops, _)) = nations.get_mut(defender_entity)
            {
                troops.0 = (troops.0 - combat_result.defender_loss).max(0.0);
            }

            // Conquer the tile
            let previous_owner = territory_manager.conquer(tile, self.source);

            // Update nation territory sizes
            if let Some(nation_id) = previous_owner
                && let Some(&nation_entity) = entity_map.0.get(&nation_id)
                && let Ok((_, mut territory_size)) = nations.get_mut(nation_entity)
            {
                territory_size.0 = territory_size.0.saturating_sub(1);
            }
            if let Some(&nation_entity) = entity_map.0.get(&self.source)
                && let Ok((_, mut territory_size)) = nations.get_mut(nation_entity)
            {
                territory_size.0 += 1;
            }

            // Increment tiles conquered counter (used for priority calculation)
            self.tiles_conquered += 1;

            // Decrement tiles per tick counter
            tiles_per_tick -= combat_result.tiles_per_tick_used;
        }

        // Check if attack should continue
        !self.priority_queue.is_empty() && self.troops >= 1.0
    }

    /// Calculate tiles conquered per tick based on troop ratio and border size
    fn calculate_tiles_per_tick(&mut self, entity_map: &NationEntityMap, nations: &Query<(&mut Troops, &mut TerritorySize)>, rng: &DeterministicRng) -> f32 {
        // Add random 0-4 to border size
        // This introduces natural variation in expansion speed
        let mut context_rng = rng.for_context(self.source.get() as u64);
        let random_border_adjustment = context_rng.random_range(0..BORDER_RANDOM_ADJUSTMENT_MAX) as f32;
        let border_size = self.priority_queue.len() as f32 + random_border_adjustment;

        // Query defender troops if attacking a player
        let defender_troops = if let Some(target_id) = self.target { entity_map.0.get(&target_id).and_then(|&entity| nations.get(entity).ok()).map(|(troops, _)| troops.0) } else { None };

        calculate_tiles_per_tick(self.troops, defender_troops, border_size)
    }

    /// Check if a tile is a valid target for this attack
    fn is_valid_target(&self, tile: U16Vec2, territory_manager: &TerritoryManager, terrain: &TerrainData) -> bool {
        if let Some(target_id) = self.target {
            territory_manager.is_owner(tile, target_id)
        } else {
            // For unclaimed attacks, check if tile is unowned and conquerable (not water)
            !territory_manager.has_owner(tile) && terrain.is_conquerable(tile)
        }
    }

    /// Add a tile to the border with proper priority calculation
    fn add_tile_to_border(&mut self, tile: U16Vec2, territory_manager: &TerritoryManager, rng: &DeterministicRng) {
        self.conquest_frontier.insert(tile);
        let priority = self.calculate_tile_priority(tile, territory_manager, rng);
        self.priority_queue.push(TilePriority { tile, priority });
    }

    /// Initialize border tiles from player's existing borders
    fn initialize_border(&mut self, border_tiles: Option<&HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng: &DeterministicRng) {
        self.initialize_border_internal(border_tiles, territory_manager, terrain, nation_borders, rng, false);
    }

    /// Refresh the attack border by re-scanning all nation border tiles
    ///
    /// This gives the attack one last chance to find conquerable tiles before ending
    fn refresh_border(&mut self, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, rng: &DeterministicRng) {
        self.initialize_border_internal(None, territory_manager, terrain, nation_borders, rng, true);
    }

    /// Internal method to initialize or refresh border tiles
    fn initialize_border_internal(&mut self, border_tiles: Option<&HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng: &DeterministicRng, clear_first: bool) {
        if clear_first {
            self.priority_queue.clear();
            self.conquest_frontier.clear();
        }

        // Get borders or use empty set as fallback (needs lifetime handling)
        let empty_borders = HashSet::new();
        let borders = border_tiles.or_else(|| nation_borders.get(&self.source).copied()).unwrap_or(&empty_borders);

        let border_count = borders.len();

        let _refresh_guard;
        let _init_guard;
        if clear_first {
            _refresh_guard = tracing::trace_span!("refresh_attack_border", border_count).entered();
        } else {
            _init_guard = tracing::trace_span!("initialize_attack_border", border_count).entered();
        }

        // Find all target tiles adjacent to our borders
        for &tile in borders {
            for neighbor in neighbors(tile, territory_manager.size()) {
                if self.is_valid_target(neighbor, territory_manager, terrain) && !self.conquest_frontier.contains(&neighbor) {
                    self.add_tile_to_border(neighbor, territory_manager, rng);
                }
            }
        }
    }

    /// Add neighbors of a newly conquered tile to the border
    fn add_neighbors_to_border(&mut self, tile: U16Vec2, territory_manager: &TerritoryManager, terrain: &TerrainData, rng: &DeterministicRng) {
        for neighbor in neighbors(tile, territory_manager.size()) {
            if self.is_valid_target(neighbor, territory_manager, terrain) && !self.conquest_frontier.contains(&neighbor) {
                self.add_tile_to_border(neighbor, territory_manager, rng);
            }
        }
    }

    /// Calculate priority for a tile (lower = conquered sooner)
    ///
    /// Uses tiles_conquered counter to ensure wave-like expansion
    fn calculate_tile_priority(&self, tile: U16Vec2, territory_manager: &TerritoryManager, rng: &DeterministicRng) -> i64 {
        // Count how many neighbors are owned by attacker
        let num_owned_by_attacker = neighbors(tile, territory_manager.size()).filter(|&neighbor| territory_manager.is_owner(neighbor, self.source)).count();

        let terrain_mag = 1.0;

        // Random factor (0-7)
        let mut tile_rng = rng.for_tile(tile);
        let random_factor = tile_rng.random_range(0..TILE_PRIORITY_RANDOM_MAX);

        // Priority calculation (lower = higher priority, conquered sooner)
        // Base calculation: tiles surrounded by more attacker neighbors get LOWER modifier values
        // Adding tiles_conquered ensures tiles discovered earlier get lower priority values
        // This creates wave-like expansion: older tiles (lower priority) conquered before newer tiles (higher priority)
        let base = (random_factor + 10) as f32;
        let modifier = TILE_PRIORITY_BASE - (num_owned_by_attacker as f32 * TILE_PRIORITY_NEIGHBOR_PENALTY) + (terrain_mag / 2.0);
        (base * modifier) as i64 + self.tiles_conquered as i64
    }

    /// Handle the addition of a tile to the nation's territory
    pub fn handle_nation_tile_add(&mut self, tile: U16Vec2, territory_manager: &TerritoryManager, terrain: &TerrainData, rng: &DeterministicRng) {
        // When nation gains a tile, check its neighbors for new targets
        self.add_neighbors_to_border(tile, territory_manager, terrain, rng);
    }

    /// Handle the addition of a tile to the target's territory
    pub fn handle_target_tile_add(&mut self, tile: U16Vec2, territory_manager: &TerritoryManager, rng: &DeterministicRng) {
        // If target gains a tile that borders our territory, add it to attack
        if Self::check_borders_tile(tile, self.source, territory_manager) && !self.conquest_frontier.contains(&tile) {
            self.conquest_frontier.insert(tile);
            let priority = self.calculate_tile_priority(tile, territory_manager, rng);
            self.priority_queue.push(TilePriority { tile, priority });
        }
    }

    /// Check if a tile borders the nation's territory
    fn check_borders_tile(tile: U16Vec2, nation_id: NationId, territory_manager: &TerritoryManager) -> bool {
        neighbors(tile, territory_manager.size()).any(|neighbor| territory_manager.is_owner(neighbor, nation_id))
    }
}
