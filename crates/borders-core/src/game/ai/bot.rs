use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use glam::{IVec2, U16Vec2};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::game::SpawnPoint;
use crate::game::core::action::GameAction;
use crate::game::core::constants::bot::*;
use crate::game::core::utils::neighbors;
use crate::game::terrain::data::TerrainData;
use crate::game::world::{NationId, TerritoryManager};

/// Bot AI component - stores per-bot state for decision making
#[derive(Component)]
pub struct Bot {
    pub last_action_tick: u64,
    pub action_cooldown: u64,
}

impl Default for Bot {
    fn default() -> Self {
        Self::new()
    }
}

impl Bot {
    pub fn new() -> Self {
        Self::with_seed(0)
    }

    /// Create a bot with deterministic initial cooldown based on seed
    pub fn with_seed(seed: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(seed);
        // Ensure initial cooldown is at least ACTION_COOLDOWN_MIN to allow borders to be calculated
        let cooldown = rng.random_range(ACTION_COOLDOWN_MIN..INITIAL_COOLDOWN_MAX);
        Self { last_action_tick: 0, action_cooldown: cooldown }
    }

    /// Sample a random subset of border tiles to reduce O(n) iteration cost
    fn sample_border_tiles(border_tiles: &HashSet<U16Vec2>, border_count: usize, rng: &mut StdRng) -> Vec<U16Vec2> {
        if border_count <= MAX_BORDER_SAMPLES {
            border_tiles.iter().copied().collect()
        } else {
            // Random sampling without replacement using Fisher-Yates
            let mut border_vec: Vec<U16Vec2> = border_tiles.iter().copied().collect();

            // Partial Fisher-Yates shuffle for first MAX_BORDER_SAMPLES elements
            for i in 0..MAX_BORDER_SAMPLES {
                let j = rng.random_range(i..border_count);
                border_vec.swap(i, j);
            }

            border_vec.truncate(MAX_BORDER_SAMPLES);
            border_vec
        }
    }

    /// Tick the bot AI - now deterministic based on turn number and RNG seed
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, turn_number: u64, nation_id: NationId, troops: &crate::game::Troops, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng_seed: u64) -> Option<GameAction> {
        // Only act every few ticks
        if turn_number < self.last_action_tick + self.action_cooldown {
            return None;
        }

        self.last_action_tick = turn_number;

        // Deterministic RNG based on turn number, nation ID, and global seed
        let seed = rng_seed.wrapping_add(turn_number).wrapping_add(nation_id.get() as u64);
        let mut rng = StdRng::seed_from_u64(seed);
        self.action_cooldown = rng.random_range(ACTION_COOLDOWN_MIN..ACTION_COOLDOWN_MAX);

        // Decide action: expand into wilderness or attack a neighbor
        let action_type: f32 = rng.random();

        if action_type < EXPAND_PROBABILITY {
            // Expand into wilderness (60% chance)
            self.expand_wilderness(nation_id, troops, territory_manager, terrain, nation_borders, &mut rng)
        } else {
            // Attack a neighbor (40% chance)
            self.attack_neighbor(nation_id, troops, territory_manager, nation_borders, &mut rng)
        }
    }

    /// Expand into unclaimed territory
    fn expand_wilderness(&self, nation_id: NationId, troops: &crate::game::Troops, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng: &mut StdRng) -> Option<GameAction> {
        let border_tiles = nation_borders.get(&nation_id)?;
        let border_count = border_tiles.len();

        let size = territory_manager.size();
        let tiles_to_check = Self::sample_border_tiles(border_tiles, border_count, rng);

        // Find a valid, unclaimed neighbor tile to attack
        for &tile in &tiles_to_check {
            if let Some(_neighbor) = neighbors(tile, size).find(|&neighbor| !territory_manager.has_owner(neighbor) && terrain.is_conquerable(neighbor)) {
                let troop_percentage: f32 = rng.random_range(EXPAND_TROOPS_MIN..EXPAND_TROOPS_MAX);
                let troop_count = (troops.0 * troop_percentage).floor() as u32;
                return Some(GameAction::Attack { target: None, troops: troop_count });
            }
        }

        None
    }

    /// Attack a neighboring nation
    fn attack_neighbor(&self, nation_id: NationId, troops: &crate::game::Troops, territory_manager: &TerritoryManager, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng: &mut StdRng) -> Option<GameAction> {
        let border_tiles = nation_borders.get(&nation_id)?;
        let border_count = border_tiles.len();

        // Find neighboring nations
        let mut neighboring_nations = HashSet::new();
        let size = territory_manager.size();

        let tiles_to_check = Self::sample_border_tiles(border_tiles, border_count, rng);

        for &tile in &tiles_to_check {
            neighboring_nations.extend(neighbors(tile, size).filter_map(|neighbor| {
                let ownership = territory_manager.get_ownership(neighbor);
                ownership.nation_id().filter(|&other_nation_id| other_nation_id != nation_id)
            }));
        }

        if neighboring_nations.is_empty() {
            return None;
        }

        // Pick a random neighbor to attack
        let neighbor_count = neighboring_nations.len();
        let target_id = neighboring_nations.into_iter().nth(rng.random_range(0..neighbor_count)).unwrap();

        let troop_percentage: f32 = rng.random_range(ATTACK_TROOPS_MIN..ATTACK_TROOPS_MAX);
        let troop_count = (troops.0 * troop_percentage).floor() as u32;
        Some(GameAction::Attack { target: Some(target_id), troops: troop_count })
    }
}

/// Spatial grid for fast spawn collision detection
/// Divides map into cells for O(1) neighbor queries instead of O(n)
struct SpawnGrid {
    grid: HashMap<IVec2, Vec<U16Vec2>>,
    cell_size: f32,
}

impl SpawnGrid {
    fn new(cell_size: f32) -> Self {
        Self { grid: HashMap::new(), cell_size }
    }

    fn insert(&mut self, pos: U16Vec2) {
        let cell = self.pos_to_cell(pos);
        self.grid.entry(cell).or_default().push(pos);
    }

    #[inline]
    fn pos_to_cell(&self, pos: U16Vec2) -> IVec2 {
        let x = pos.x as f32 / self.cell_size;
        let y = pos.y as f32 / self.cell_size;
        IVec2::new(x as i32, y as i32)
    }

    fn has_nearby(&self, pos: U16Vec2, radius: f32) -> bool {
        let cell = self.pos_to_cell(pos);
        let cell_radius = (radius / self.cell_size).ceil() as i32;

        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                let check_cell = cell + IVec2::new(dx, dy);
                if let Some(positions) = self.grid.get(&check_cell) {
                    for &existing_pos in positions {
                        if calculate_position_distance(pos, existing_pos) < radius {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }
}

/// Calculate Euclidean distance between two positions
#[inline]
fn calculate_position_distance(pos1: U16Vec2, pos2: U16Vec2) -> f32 {
    pos1.as_vec2().distance(pos2.as_vec2())
}

/// Calculate initial bot spawn positions (first pass)
///
/// Places bots at random valid locations with adaptive spacing.
/// Uses spatial grid for O(1) neighbor checks and adaptively reduces
/// minimum distance when map becomes crowded.
///
/// Guarantees all bots spawn (no silent drops). This is deterministic based on rng_seed.
///
/// Returns Vec<SpawnPoint> for each bot
pub fn calculate_initial_spawns(bot_nation_ids: &[NationId], territory_manager: &TerritoryManager, terrain: &TerrainData, rng_seed: u64) -> Vec<SpawnPoint> {
    let _guard = tracing::trace_span!("calculate_initial_spawns", bot_count = bot_nation_ids.len()).entered();

    let size = territory_manager.size();

    let mut spawn_positions = Vec::with_capacity(bot_nation_ids.len());
    let mut grid = SpawnGrid::new(MIN_SPAWN_DISTANCE);
    let mut current_min_distance = MIN_SPAWN_DISTANCE;

    for (bot_index, &nation_id) in bot_nation_ids.iter().enumerate() {
        // Deterministic RNG for spawn location
        let seed = rng_seed.wrapping_add(nation_id.get() as u64).wrapping_add(bot_index as u64);
        let mut rng = StdRng::seed_from_u64(seed);

        let mut placed = false;

        // Try with current minimum distance
        while !placed && current_min_distance >= ABSOLUTE_MIN_DISTANCE {
            // Phase 1: Random sampling
            for _ in 0..SPAWN_RANDOM_ATTEMPTS {
                let tile_pos = U16Vec2::new(rng.random_range(0..size.x), rng.random_range(0..size.y));

                // Check if tile is valid land
                if territory_manager.has_owner(tile_pos) || !terrain.is_conquerable(tile_pos) {
                    continue;
                }

                // Check distance using spatial grid (O(1) instead of O(n))
                if !grid.has_nearby(tile_pos, current_min_distance) {
                    spawn_positions.push(SpawnPoint::new(nation_id, tile_pos));
                    grid.insert(tile_pos);
                    placed = true;
                    break;
                }
            }

            // Phase 2: Grid-guided fallback (if random sampling failed)
            if !placed {
                // Try a systematic grid search with stride
                let stride = (current_min_distance * SPAWN_GRID_STRIDE_FACTOR) as u16;
                let mut attempts = 0;
                for y in (0..size.y).step_by(stride.max(1) as usize) {
                    for x in (0..size.x).step_by(stride.max(1) as usize) {
                        let tile_pos = U16Vec2::new(x, y);

                        if territory_manager.has_owner(tile_pos) || !terrain.is_conquerable(tile_pos) {
                            continue;
                        }

                        if !grid.has_nearby(tile_pos, current_min_distance) {
                            spawn_positions.push(SpawnPoint::new(nation_id, tile_pos));
                            grid.insert(tile_pos);
                            placed = true;
                            break;
                        }

                        attempts += 1;
                        if attempts > SPAWN_GRID_MAX_ATTEMPTS {
                            break;
                        }
                    }
                    if placed {
                        break;
                    }
                }
            }

            // Phase 3: Reduce minimum distance and retry
            if !placed {
                current_min_distance *= DISTANCE_REDUCTION_FACTOR;
                if bot_index % 100 == 0 && current_min_distance < MIN_SPAWN_DISTANCE {
                    tracing::debug!("Adaptive spawn: reduced min_distance to {:.1} for bot {}", current_min_distance, bot_index);
                }
            }
        }

        // Final fallback: Place at any valid land tile (guaranteed)
        if !placed {
            for _ in 0..SPAWN_FALLBACK_ATTEMPTS {
                let tile_pos = U16Vec2::new(rng.random_range(0..size.x), rng.random_range(0..size.y));
                if !territory_manager.has_owner(tile_pos) && terrain.is_conquerable(tile_pos) {
                    spawn_positions.push(SpawnPoint::new(nation_id, tile_pos));
                    grid.insert(tile_pos);
                    placed = true;
                    tracing::warn!("Bot {} placed with fallback (no distance constraint)", nation_id);
                    break;
                }
            }
        }

        if !placed {
            tracing::error!("Failed to place bot {} after all attempts", nation_id);
        }
    }

    spawn_positions
}

/// Recalculate bot spawns considering human nation positions (second pass)
///
/// For any bot that is too close to a human nation spawn, find a new position.
/// Uses adaptive algorithm with grid acceleration to guarantee all displaced
/// bots find new positions. This maintains determinism while ensuring proper spawn spacing.
///
/// Arguments:
/// - `initial_bot_spawns`: Bot positions from first pass
/// - `human_spawns`: Human nation spawn positions
/// - `territory_manager`: For checking valid tiles
/// - `terrain`: For checking conquerable tiles
/// - `rng_seed`: For deterministic relocation
///
/// Returns updated Vec<SpawnPoint> with relocated bots
pub fn recalculate_spawns_with_players(initial_bot_spawns: Vec<SpawnPoint>, player_spawns: &[SpawnPoint], territory_manager: &TerritoryManager, terrain: &TerrainData, rng_seed: u64) -> Vec<SpawnPoint> {
    let _guard = tracing::trace_span!("recalculate_spawns_with_players", bot_count = initial_bot_spawns.len(), human_count = player_spawns.len()).entered();

    let size = territory_manager.size();

    // Build spatial grid to track occupied spawn locations
    // Contains all human nation spawns plus bots that don't need relocation
    // Enables O(1) distance checks instead of O(n) iteration
    let mut grid = SpawnGrid::new(MIN_SPAWN_DISTANCE);
    for spawn in player_spawns {
        grid.insert(spawn.tile);
    }

    // Partition bots into two groups:
    // 1. Bots that are far enough from all human nation spawns (keep as-is)
    // 2. Bots that violate MIN_SPAWN_DISTANCE from any human nation (need relocation)
    let mut bots_to_relocate = Vec::new();
    let mut final_spawns = Vec::new();

    for spawn in initial_bot_spawns {
        let mut needs_relocation = false;

        for human_spawn in player_spawns {
            if calculate_position_distance(spawn.tile, human_spawn.tile) < MIN_SPAWN_DISTANCE {
                needs_relocation = true;
                break;
            }
        }

        if needs_relocation {
            bots_to_relocate.push(spawn.nation);
        } else {
            // Bot is valid - add to final list and mark space as occupied
            final_spawns.push(spawn);
            grid.insert(spawn.tile);
        }
    }

    // Relocate displaced bots using a three-phase adaptive algorithm:
    // Phase 1: Random sampling (fast, works well when space is available)
    // Phase 2: Grid-based systematic search (fallback when random fails)
    // Phase 3: Adaptive distance reduction (progressively relax spacing constraints)
    //
    // This adaptively reduces spacing as the map fills up, ensuring all bots
    // eventually find placement even on crowded maps
    let mut current_min_distance = MIN_SPAWN_DISTANCE;

    for (reloc_index, &nation_id) in bots_to_relocate.iter().enumerate() {
        // Deterministic RNG with a different seed offset to avoid reusing original positions
        let seed = rng_seed.wrapping_add(nation_id.get() as u64).wrapping_add(0xDEADBEEF);
        let mut rng = StdRng::seed_from_u64(seed);

        let mut placed = false;

        // Keep trying with progressively relaxed distance constraints
        while !placed && current_min_distance >= ABSOLUTE_MIN_DISTANCE {
            // Phase 1: Random sampling - try random tiles until we find a valid spot
            // Fast and evenly distributed when sufficient space exists
            for _ in 0..SPAWN_RANDOM_ATTEMPTS {
                let tile_pos = U16Vec2::new(rng.random_range(0..size.x), rng.random_range(0..size.y));

                // Skip tiles that are already owned or unconquerable (water/mountains)
                if territory_manager.has_owner(tile_pos) || !terrain.is_conquerable(tile_pos) {
                    continue;
                }

                // Check if this tile is far enough from all existing spawns
                // Grid lookup is O(1) - only checks cells within radius, not all spawns
                if !grid.has_nearby(tile_pos, current_min_distance) {
                    final_spawns.push(SpawnPoint::new(nation_id, tile_pos));
                    grid.insert(tile_pos);
                    placed = true;
                    break;
                }
            }

            // Phase 2: Grid-based systematic search
            // When random sampling fails (map is crowded), use a strided grid search
            // to systematically check evenly-spaced candidate positions
            if !placed {
                // Stride determines spacing between checked positions (larger = faster but might miss spots)
                let stride = (current_min_distance * SPAWN_GRID_STRIDE_FACTOR) as u16;
                let mut attempts = 0;

                for y in (0..size.y).step_by(stride.max(1) as usize) {
                    for x in (0..size.x).step_by(stride.max(1) as usize) {
                        let tile_pos = U16Vec2::new(x, y);

                        if territory_manager.has_owner(tile_pos) || !terrain.is_conquerable(tile_pos) {
                            continue;
                        }

                        if !grid.has_nearby(tile_pos, current_min_distance) {
                            final_spawns.push(SpawnPoint::new(nation_id, tile_pos));
                            grid.insert(tile_pos);
                            placed = true;
                            break;
                        }

                        // Prevent infinite loops on maps with very little valid space
                        attempts += 1;
                        if attempts > SPAWN_GRID_MAX_ATTEMPTS {
                            break;
                        }
                    }
                    if placed {
                        break;
                    }
                }
            }

            // Phase 3: Adaptive distance reduction
            // If both random and grid search failed, the map is too crowded
            // Reduce minimum spacing requirement and retry both phases
            if !placed {
                current_min_distance *= DISTANCE_REDUCTION_FACTOR;
                if reloc_index % 50 == 0 && current_min_distance < MIN_SPAWN_DISTANCE {
                    tracing::debug!("Adaptive relocation: reduced min_distance to {:.1} for bot {}", current_min_distance, reloc_index);
                }
            }
        }

        // Final fallback: ignore all distance constraints
        // Guarantees placement even on extremely crowded maps
        // Simply finds any valid conquerable tile
        if !placed {
            for _ in 0..SPAWN_FALLBACK_ATTEMPTS {
                let tile_pos = U16Vec2::new(rng.random_range(0..size.x), rng.random_range(0..size.y));
                if !territory_manager.has_owner(tile_pos) && terrain.is_conquerable(tile_pos) {
                    final_spawns.push(SpawnPoint::new(nation_id, tile_pos));
                    grid.insert(tile_pos);
                    placed = true;
                    tracing::warn!("Bot {} relocated with fallback (no distance constraint)", nation_id);
                    break;
                }
            }
        }

        if !placed {
            tracing::error!("Failed to relocate bot {} after all attempts", nation_id);
        }
    }

    final_spawns
}
