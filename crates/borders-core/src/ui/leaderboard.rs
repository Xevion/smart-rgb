//! Shared leaderboard data structures and utilities
//!
//! This module contains types and systems for managing leaderboard data
//! that are shared between desktop and WASM builds.

use std::collections::HashMap;
use std::time::Duration;

use bevy_ecs::prelude::*;

use crate::game::ActiveAttacks;
use crate::game::entities::{Dead, Troops};
use crate::game::input::context::LocalPlayerContext;
use crate::game::systems::turn::CurrentTurn;
use crate::game::{NationColor, NationName, TerritoryManager, TerritorySize, world::NationId};

use crate::time::Time;
// Re-export UI types from protocol for convenience
pub use crate::ui::protocol::{AttackEntry, AttacksUpdatePayload, BackendMessage, LeaderboardEntry, LeaderboardSnapshot};

/// Convert RGBA color to hex string (without alpha)
pub fn rgba_to_hex(color: [f32; 4]) -> String {
    let r = (color[0] * 255.0) as u8;
    let g = (color[1] * 255.0) as u8;
    let b = (color[2] * 255.0) as u8;
    format!("{:02X}{:02X}{:02X}", r, g, b)
}

/// Resolve a nation's display name with fallbacks for missing or empty names
fn resolve_player_name(nation_id: NationId, client_nation_id: NationId, nation_data: Option<(&NationName, &NationColor)>) -> String {
    nation_data.map(|(name, _)| if name.0.is_empty() { if nation_id == client_nation_id { "Player".to_string() } else { format!("Nation {}", nation_id) } } else { name.0.clone() }).unwrap_or_else(|| format!("Nation {}", nation_id))
}

/// Resource to track last emitted leaderboard state for deduplication
#[derive(Resource, Default, Debug)]
pub struct LastLeaderboardDigest {
    pub entries: Vec<(NationId, String, u32, u32)>, // (id, name, tile_count, troops)
    pub turn: u64,
}

/// Resource to track last emitted attacks state for deduplication
#[derive(Resource, Default, Debug)]
pub struct LastAttacksDigest {
    pub entries: Vec<(NationId, Option<NationId>, u32, u64, bool)>, // (attacker_id, target_id, troops, id, is_outgoing)
    pub turn: u64,
    pub count: usize, // Track number of attacks to always detect add/remove
}

/// Resource to track display order update cycles (updates every 3rd tick)
#[derive(Resource, Default, Debug)]
pub struct DisplayOrderUpdateCounter {
    pub tick: u32,
}

/// Resource to track last calculated display order for each nation
#[derive(Resource, Default, Debug)]
pub struct LastDisplayOrder {
    pub order_map: HashMap<NationId, usize>,
}

/// Resource to throttle leaderboard snapshot emissions
#[derive(Resource, Debug)]
pub struct LeaderboardThrottle {
    last_emission_elapsed: Option<Duration>,
    throttle_duration: Duration,
}

impl Default for LeaderboardThrottle {
    fn default() -> Self {
        Self {
            last_emission_elapsed: None,
            throttle_duration: Duration::from_millis(420), // 420ms (3x faster, display order updates every 3rd tick)
        }
    }
}

/// Build a complete leaderboard snapshot from current game state
/// Returns None if nothing has changed since last_digest
pub fn build_leaderboard_snapshot(
    turn_number: u64,
    total_land_tiles: u32,
    nation_stats: &[(NationId, u32, u32)], // (id, tile_count, troops)
    client_nation_id: NationId,
    nations_by_id: &HashMap<NationId, (&NationName, &NationColor)>,
    last_digest: &mut LastLeaderboardDigest,
    display_order_map: &HashMap<NationId, usize>,
) -> Option<LeaderboardSnapshot> {
    // Build current digest for comparison (includes names now), filter out eliminated nations
    let current_entries: Vec<(NationId, String, u32, u32)> = nation_stats
        .iter()
        .filter(|(_, tile_count, _)| *tile_count > 0) // Exclude eliminated nations
        .map(|(id, tile_count, troops)| {
            let nation_data = nations_by_id.get(id).copied();
            let name = resolve_player_name(*id, client_nation_id, nation_data);
            (*id, name, *tile_count, *troops)
        })
        .collect();

    // Check if anything has changed (stats OR names)
    if current_entries == last_digest.entries && turn_number == last_digest.turn {
        return None; // No changes
    }

    // Update digest
    last_digest.entries = current_entries;
    last_digest.turn = turn_number;

    // Build complete leaderboard entries (names + colors + stats), filter out eliminated nations
    let mut entries: Vec<LeaderboardEntry> = nation_stats
        .iter()
        .filter(|(_, tile_count, _)| *tile_count > 0) // Exclude eliminated nations
        .map(|(id, tile_count, troops)| {
            let nation_data = nations_by_id.get(id).copied();

            let name = resolve_player_name(*id, client_nation_id, nation_data);

            let color = nation_data.map(|(_, color)| rgba_to_hex(color.0.to_rgba())).unwrap_or_else(|| "808080".to_string()); // Gray fallback

            let territory_percent = if total_land_tiles > 0 { *tile_count as f32 / total_land_tiles as f32 } else { 0.0 };

            LeaderboardEntry {
                id: *id,
                name,
                color,
                tile_count: *tile_count,
                troops: *troops,
                territory_percent,
                rank: 0,          // Assigned after sorting
                display_order: 0, // Assigned after sorting
            }
        })
        .collect();

    // Sort by tile count descending
    entries.sort_by(|a, b| b.tile_count.cmp(&a.tile_count));

    // Assign rank and display_order after sorting
    for (idx, entry) in entries.iter_mut().enumerate() {
        entry.rank = idx + 1; // 1-indexed rank

        // Use display_order from map, or fallback to current rank position
        // TODO: Handle mid-game joins by initializing display_order for new players
        entry.display_order = display_order_map.get(&entry.id).copied().unwrap_or(idx);
    }

    Some(LeaderboardSnapshot { turn: turn_number, total_land_tiles, entries, client_nation_id })
}

/// Bevy system that emits leaderboard snapshot events
#[allow(clippy::too_many_arguments)]
pub fn emit_leaderboard_snapshot_system(time: Res<Time>, current_turn: Option<Res<CurrentTurn>>, local_context: If<Res<LocalPlayerContext>>, territory_manager: Res<TerritoryManager>, nations: Query<(&NationId, &NationName, &NationColor)>, nation_stats: Query<(&NationId, &TerritorySize, &Troops), Without<Dead>>, mut last_digest: If<ResMut<LastLeaderboardDigest>>, mut throttle: If<ResMut<LeaderboardThrottle>>, mut counter: If<ResMut<DisplayOrderUpdateCounter>>, mut last_display_order: If<ResMut<LastDisplayOrder>>, mut backend_messages: MessageWriter<BackendMessage>) {
    let _guard = tracing::debug_span!("emit_leaderboard_snapshot").entered();
    let Some(current_turn) = current_turn else {
        return;
    };

    // Check if enough time has passed since last emission
    let current_elapsed = time.elapsed();
    let should_emit = throttle.last_emission_elapsed.map(|last| current_elapsed.saturating_sub(last) >= throttle.throttle_duration).unwrap_or(true); // Emit on first call

    if !should_emit {
        return;
    }

    // Build nation lookup map from ECS components
    let nations_by_id: HashMap<NationId, (&NationName, &NationColor)> = nations.iter().map(|(nation_id, name, color)| (*nation_id, (name, color))).collect();

    // Collect nation stats from ECS
    let nation_stats_data: Vec<(NationId, u32, u32)> = nation_stats.iter().map(|(nation_id, territory_size, troops)| (*nation_id, territory_size.0, troops.0 as u32)).collect();

    // Calculate total land tiles
    let total_land_tiles = crate::game::queries::count_land_tiles(&territory_manager);

    // Increment tick counter (wraps on overflow)
    counter.tick = counter.tick.wrapping_add(1);

    // Every 3rd tick, recalculate display order from current rankings
    if counter.tick.is_multiple_of(3) {
        // Build temporary sorted list to determine new display order
        let mut sorted_nations: Vec<_> = nation_stats_data
            .iter()
            .filter(|(_, tile_count, _)| *tile_count > 0) // Exclude eliminated nations
            .collect();
        sorted_nations.sort_by(|a, b| b.1.cmp(&a.1));

        // Update display order map with current rankings
        last_display_order.order_map.clear();
        for (idx, (nation_id, _, _)) in sorted_nations.iter().enumerate() {
            last_display_order.order_map.insert(*nation_id, idx);
        }
    }

    if let Some(snapshot) = build_leaderboard_snapshot(current_turn.turn.turn_number, total_land_tiles, &nation_stats_data, local_context.id, &nations_by_id, &mut last_digest, &last_display_order.order_map) {
        backend_messages.write(BackendMessage::LeaderboardSnapshot(snapshot));
        throttle.last_emission_elapsed = Some(current_elapsed);
    }
}

/// Build an attacks update payload from current game state
/// Always returns the current state (digest is used to prevent duplicate emissions)
pub fn build_attacks_update(active_attacks: &ActiveAttacks, turn_number: u64, client_nation_id: NationId, last_digest: &mut LastAttacksDigest) -> Option<AttacksUpdatePayload> {
    // Get attacks for the client nation
    let raw_attacks = active_attacks.get_attacks_for_nation(client_nation_id);

    // Build current digest for comparison
    let current_entries = raw_attacks.iter().map(|&(attacker_id, target_id, troops, id, is_outgoing)| (attacker_id, target_id, troops as u32, id, is_outgoing)).collect();

    let current_count = raw_attacks.len();

    // Always send update if attack count changed (add/remove)
    let count_changed = current_count != last_digest.count;

    // Check if digest changed (troop counts, etc.)
    let digest_changed = current_entries != last_digest.entries;

    if !count_changed && !digest_changed {
        return None; // No changes at all
    }

    // Update digest
    last_digest.entries = current_entries;
    last_digest.turn = turn_number;
    last_digest.count = current_count;

    // Build attack entries
    let entries: Vec<AttackEntry> = raw_attacks.into_iter().map(|(attacker_id, target_id, troops, id, is_outgoing)| AttackEntry { id, attacker_id, target_id, troops: troops as u32, is_outgoing }).collect();

    Some(AttacksUpdatePayload { turn: turn_number, entries })
}

/// Bevy system that emits attacks update events
pub fn emit_attacks_update_system(active_attacks: Res<ActiveAttacks>, current_turn: Option<Res<CurrentTurn>>, local_context: If<Res<LocalPlayerContext>>, mut last_digest: If<ResMut<LastAttacksDigest>>, mut backend_messages: MessageWriter<BackendMessage>) {
    let _guard = tracing::debug_span!("emit_attacks_update").entered();

    let Some(current_turn) = current_turn else {
        return;
    };

    if let Some(payload) = build_attacks_update(&active_attacks, current_turn.turn.turn_number, local_context.id, &mut last_digest) {
        backend_messages.write(BackendMessage::AttacksUpdate(payload));
    }
}
