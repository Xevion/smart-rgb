/// Active attacks management
///
/// This module manages all ongoing attacks in the game. It provides efficient
/// lookup and coordination of attacks, ensuring proper merging of attacks on
/// the same target and handling counter-attacks.
use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::*;
use glam::U16Vec2;
use slotmap::{SlotMap, new_key_type};

new_key_type! {
    /// Unique key for identifying attacks in the SlotMap
    pub struct AttackKey;
}

use super::executor::{AttackConfig, AttackExecutor};
use crate::game::NationId;
use crate::game::ai::bot::Bot;
use crate::game::core::rng::DeterministicRng;
use crate::game::entities::{NationEntityMap, TerritorySize, Troops};
use crate::game::terrain::TerrainData;
use crate::game::world::TerritoryManager;

/// Index structure for efficient attack lookups
///
/// Maintains multiple indices for O(1) lookups by different criteria.
/// All methods maintain index consistency automatically - you cannot
/// accidentally update one index without updating the others.
struct AttackIndex {
    /// Maps each pair of nations to the on-going attacks between them
    /// Multiple attacks can exist between same pair (islands, ship landings, etc.)
    nation_index: HashMap<(NationId, NationId), HashSet<AttackKey>>,

    /// Maps each nation to the unclaimed attack it has scheduled, if any
    /// Each nation can only have one on-going unclaimed attack at a time
    unclaimed_index: HashMap<NationId, AttackKey>,

    /// Maps each nation to the on-going attacks it is involved in
    /// A nation can have multiple on-going attacks with the same target.
    nation_attack_list: HashMap<NationId, HashSet<AttackKey>>,

    /// Maps each nation to the on-going attacks it is targeted by
    /// A nation can have multiple on-going attacks from the same attacker.
    target_attack_list: HashMap<NationId, HashSet<AttackKey>>,
}

impl AttackIndex {
    fn new() -> Self {
        Self { nation_index: HashMap::new(), unclaimed_index: HashMap::new(), nation_attack_list: HashMap::new(), target_attack_list: HashMap::new() }
    }

    fn clear(&mut self) {
        self.nation_index.clear();
        self.unclaimed_index.clear();
        self.nation_attack_list.clear();
        self.target_attack_list.clear();
    }

    /// Get existing unclaimed attack for a nation
    fn get_unclaimed_attack(&self, nation_id: NationId) -> Option<AttackKey> {
        self.unclaimed_index.get(&nation_id).copied()
    }

    /// Get first existing attack on a target (for merging troops)
    fn get_existing_attack(&self, nation_id: NationId, target_id: NationId) -> Option<AttackKey> {
        self.nation_index.get(&(nation_id, target_id))?.iter().next().copied()
    }

    /// Check if counter-attacks exist (opposite direction)
    fn has_counter_attacks(&self, nation_id: NationId, target_id: NationId) -> bool {
        self.nation_index.get(&(target_id, nation_id)).is_some_and(|set| !set.is_empty())
    }

    /// Get first counter-attack key (for resolution)
    fn get_counter_attack(&self, nation_id: NationId, target_id: NationId) -> Option<AttackKey> {
        self.nation_index.get(&(target_id, nation_id))?.iter().next().copied()
    }

    /// Get all attacks where nation is attacker
    fn get_attacks_by_nation(&self, nation: NationId) -> Option<&HashSet<AttackKey>> {
        self.nation_attack_list.get(&nation)
    }

    /// Get all attacks where nation is target
    fn get_attacks_on_nation(&self, nation: NationId) -> Option<&HashSet<AttackKey>> {
        self.target_attack_list.get(&nation)
    }

    /// Add a Nation-vs-Nation attack to all indices atomically
    fn add_nation_attack(&mut self, source: NationId, target: NationId, key: AttackKey) {
        // Invariant: Cannot attack yourself
        if source == target {
            tracing::error!(
                nation_id = %source,
                attack_key = ?key,
                "Attempted to add self-attack to index (invariant violation)"
            );
            return;
        }

        self.nation_index.entry((source, target)).or_default().insert(key);
        self.nation_attack_list.entry(source).or_default().insert(key);
        self.target_attack_list.entry(target).or_default().insert(key);
    }

    /// Add an unclaimed territory attack to all indices atomically
    fn add_unclaimed_attack(&mut self, nation: NationId, key: AttackKey) {
        self.unclaimed_index.insert(nation, key);
        self.nation_attack_list.entry(nation).or_default().insert(key);
    }

    /// Remove a Nation-vs-Nation attack from all indices atomically
    fn remove_nation_attack(&mut self, source: NationId, target: NationId, key: AttackKey) {
        if let Some(attack_set) = self.nation_attack_list.get_mut(&source) {
            attack_set.remove(&key);
        }
        if let Some(attack_set) = self.target_attack_list.get_mut(&target) {
            attack_set.remove(&key);
        }
        if let Some(attack_set) = self.nation_index.get_mut(&(source, target)) {
            attack_set.remove(&key);
        }
    }

    /// Remove an unclaimed territory attack from all indices atomically
    fn remove_unclaimed_attack(&mut self, nation: NationId, key: AttackKey) {
        self.unclaimed_index.remove(&nation);
        if let Some(attack_set) = self.nation_attack_list.get_mut(&nation) {
            attack_set.remove(&key);
        }
    }
}

/// Manages all active attacks in the game
///
/// This resource tracks ongoing attacks and provides efficient lookup
/// by attacker/target relationships. Attacks progress over multiple turns
/// until they run out of troops or conquerable tiles.
///
/// Uses SlotMap for stable keys - no index shifting needed on removal.
/// Uses AttackIndex for consistent multi-index management.
#[derive(Resource)]
pub struct ActiveAttacks {
    attacks: SlotMap<AttackKey, AttackExecutor>,
    index: AttackIndex,
    next_attack_id: u64,
}

impl Default for ActiveAttacks {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveAttacks {
    pub fn new() -> Self {
        Self { attacks: SlotMap::with_key(), index: AttackIndex::new(), next_attack_id: 0 }
    }

    /// Initialize the attack handler
    pub fn init(&mut self) {
        self.attacks.clear();
        self.index.clear();
        self.next_attack_id = 0;
    }

    /// Schedule an attack on unclaimed territory
    ///
    /// If an attack on unclaimed territory already exists for this nation,
    /// the troops are added to it and borders are expanded.
    #[allow(clippy::too_many_arguments)]
    pub fn schedule_unclaimed(&mut self, nation_id: NationId, troops: f32, border_tiles: Option<&HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, turn_number: u64, rng: &DeterministicRng) {
        // Check if there's already an attack on unclaimed territory
        if let Some(attack_key) = self.index.get_unclaimed_attack(nation_id) {
            // Add troops to existing attack
            self.attacks[attack_key].modify_troops(troops);

            // Add new borders to allow multi-region expansion
            if let Some(borders) = border_tiles.or_else(|| nation_borders.get(&nation_id).copied()) {
                self.attacks[attack_key].add_borders(borders, territory_manager, terrain, rng);
            }
            return;
        }

        // Create new attack
        self.add_unclaimed(nation_id, troops, border_tiles, territory_manager, terrain, nation_borders, turn_number, rng);
    }

    /// Schedule an attack on another nation
    ///
    /// Handles attack merging (if attacking same target) and counter-attacks
    /// (opposite direction attacks are resolved first).
    #[allow(clippy::too_many_arguments)]
    pub fn schedule_attack(&mut self, nation_id: NationId, target_id: NationId, mut troops: f32, border_tiles: Option<&HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, turn_number: u64, rng: &DeterministicRng) {
        // Prevent self-attacks early (before any processing)
        if nation_id == target_id {
            tracing::warn!(
                nation_id = %nation_id,
                "Attempted self-attack prevented"
            );
            return;
        }

        // Check if there's already an attack on this target
        if let Some(attack_key) = self.index.get_existing_attack(nation_id, target_id) {
            // Add troops to existing attack
            self.attacks[attack_key].modify_troops(troops);

            // Add new borders to allow multi-region expansion
            if let Some(borders) = border_tiles.or_else(|| nation_borders.get(&nation_id).copied()) {
                self.attacks[attack_key].add_borders(borders, territory_manager, terrain, rng);
            }
            return;
        }

        // Check for counter-attacks (opposite direction) - prevent mutual attacks
        while self.index.has_counter_attacks(nation_id, target_id) {
            let opposite_key = self.index.get_counter_attack(nation_id, target_id).unwrap();

            if self.attacks[opposite_key].oppose(troops) {
                // Counter-attack absorbed the new attack
                return;
            }

            // Counter-attack was defeated, deduct its troops from the new attack
            troops -= self.attacks[opposite_key].get_troops();

            // Remove the defeated counter-attack
            self.remove_attack(opposite_key);
        }

        // Create new attack
        self.add_attack(nation_id, target_id, troops, border_tiles, territory_manager, terrain, nation_borders, turn_number, rng);
    }

    /// Tick all active attacks
    ///
    /// Progresses each attack by one turn. Attacks that run out of troops
    /// or conquerable tiles are removed and their remaining troops are
    /// returned to the attacking nation.
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, entity_map: &NationEntityMap, nations: &mut Query<(&mut Troops, &mut TerritorySize)>, territory_manager: &mut TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, rng: &DeterministicRng, is_bot_query: &Query<Has<Bot>>) {
        let attack_count = self.attacks.len();
        let _guard = tracing::trace_span!("attacks_tick", attack_count).entered();

        let mut attacks_to_remove = Vec::new();

        for (attack_key, attack) in &mut self.attacks {
            let should_continue = attack.tick(entity_map, nations, territory_manager, terrain, nation_borders, rng);

            if !should_continue {
                // Return remaining troops to nation (ECS component)
                let remaining_troops = attack.get_troops();

                if let Some(&entity) = entity_map.0.get(&attack.source)
                    && let Ok((mut troops, territory_size)) = nations.get_mut(entity)
                {
                    let is_bot = is_bot_query.get(entity).unwrap_or(false);
                    troops.0 = crate::game::entities::add_troops_capped(troops.0, remaining_troops, territory_size.0, is_bot);
                }

                // Mark attack for removal
                attacks_to_remove.push(attack_key);
            }
        }

        // Remove completed attacks
        for attack_key in attacks_to_remove {
            self.remove_attack(attack_key);
        }
    }

    /// Handle a tile being added to a nation's territory
    ///
    /// Notifies all relevant attacks that territory has changed so they can
    /// update their borders and targets.
    pub fn handle_territory_add(&mut self, tile: U16Vec2, nation_id: NationId, territory_manager: &TerritoryManager, terrain: &TerrainData, rng: &DeterministicRng) {
        // Notify all attacks where this nation is the attacker
        if let Some(attack_set) = self.index.get_attacks_by_nation(nation_id) {
            for &attack_key in attack_set {
                self.attacks[attack_key].handle_nation_tile_add(tile, territory_manager, terrain, rng);
            }
        }

        // Notify all attacks where this nation is the target
        if let Some(attack_set) = self.index.get_attacks_on_nation(nation_id) {
            for &attack_key in attack_set {
                self.attacks[attack_key].handle_target_tile_add(tile, territory_manager, rng);
            }
        }
    }

    /// Add an attack on unclaimed territory
    #[allow(clippy::too_many_arguments)]
    fn add_unclaimed(&mut self, nation: NationId, troops: f32, border_tiles: Option<&HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, turn_number: u64, rng: &DeterministicRng) {
        let attack_id = self.next_attack_id;
        self.next_attack_id += 1;

        let attack = AttackExecutor::new(AttackConfig { id: attack_id, source: nation, target: None, troops, border_tiles, territory_manager, nation_borders, turn_number, terrain }, rng);

        let attack_key = self.attacks.insert(attack);
        self.index.add_unclaimed_attack(nation, attack_key);
    }

    /// Add an attack on a nation
    #[allow(clippy::too_many_arguments)]
    fn add_attack(&mut self, nation_id: NationId, target_id: NationId, troops: f32, border_tiles: Option<&HashSet<U16Vec2>>, territory_manager: &TerritoryManager, terrain: &TerrainData, nation_borders: &HashMap<NationId, &HashSet<U16Vec2>>, turn_number: u64, rng: &DeterministicRng) {
        let attack_id = self.next_attack_id;
        self.next_attack_id += 1;

        let attack = AttackExecutor::new(AttackConfig { id: attack_id, source: nation_id, target: Some(target_id), troops, border_tiles, territory_manager, nation_borders, turn_number, terrain }, rng);

        let attack_key = self.attacks.insert(attack);
        self.index.add_nation_attack(nation_id, target_id, attack_key);
    }

    /// Get all attacks involving a specific nation (as attacker or target)
    ///
    /// Returns a list of (attacker_id, target_id, troops, start_turn, is_outgoing)
    /// sorted by start_turn descending (most recent first)
    pub fn get_attacks_for_nation(&self, nation_id: NationId) -> Vec<(NationId, Option<NationId>, f32, u64, bool)> {
        let mut attacks = Vec::new();

        // Add outgoing attacks (nation is attacker)
        if let Some(attack_set) = self.index.get_attacks_by_nation(nation_id) {
            for &attack_key in attack_set {
                let attack = &self.attacks[attack_key];
                attacks.push((
                    attack.source,
                    attack.target,
                    attack.get_troops(),
                    attack.id(),
                    true, // outgoing
                ));
            }
        }

        // Add incoming attacks (nation is target)
        if let Some(attack_set) = self.index.get_attacks_on_nation(nation_id) {
            for &attack_key in attack_set {
                let attack = &self.attacks[attack_key];
                attacks.push((
                    attack.source,
                    attack.target,
                    attack.get_troops(),
                    attack.id(),
                    false, // incoming
                ));
            }
        }

        // Sort by attack ID descending (most recent first)
        attacks.sort_by(|a, b| b.3.cmp(&a.3));
        attacks
    }

    /// Remove an attack and update all indices
    ///
    /// With SlotMap, keys remain stable so no index shifting is needed.
    /// HashSet provides O(1) removal without element shifting.
    fn remove_attack(&mut self, attack_key: AttackKey) {
        let attack = &self.attacks[attack_key];

        // Remove from all indices atomically
        if let Some(target) = attack.target {
            self.index.remove_nation_attack(attack.source, target, attack_key);
        } else {
            self.index.remove_unclaimed_attack(attack.source, attack_key);
        }

        // Remove attack from slot map - no index shifting needed!
        self.attacks.remove(attack_key);
    }
}
