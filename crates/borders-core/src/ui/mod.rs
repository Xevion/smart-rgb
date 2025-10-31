//! UI/Frontend module for rendering and user interaction
//!
//! This module contains all frontend-related concerns including:
//! - Protocol definitions for frontend-backend communication
//! - Leaderboard management
//! - Platform transport abstraction

use std::collections::{HashMap, HashSet};

use bevy_ecs::hierarchy::ChildOf;
use bevy_ecs::prelude::*;

use crate::game::input::MouseMotionMessage;
use crate::game::systems::turn::CurrentTurn;
use crate::game::{NationId, Ship, TerritoryManager};

pub mod leaderboard;
pub mod protocol;
pub mod transport;

// Re-export commonly used types
pub use leaderboard::*;
pub use protocol::*;
pub use transport::*;

/// Resource to track currently highlighted nation for visual feedback
#[derive(Resource, Default, Debug)]
pub struct NationHighlightState {
    pub highlighted_nation: Option<NationId>,
}

/// System that tracks hovered nation and emits highlight events
pub fn emit_nation_highlight_system(mut mouse_motion: MessageReader<MouseMotionMessage>, territory_manager: Res<TerritoryManager>, mut highlight_state: If<ResMut<NationHighlightState>>, mut backend_messages: MessageWriter<BackendMessage>) {
    // Get the latest mouse motion event
    let latest_motion = mouse_motion.read().last();

    let new_highlighted = if let Some(motion) = latest_motion {
        if let Some(tile_coord) = motion.tile {
            let ownership = territory_manager.get_ownership(tile_coord);
            ownership.nation_id()
        } else {
            None
        }
    } else {
        // No motion events this frame, keep current highlight
        return;
    };

    // Only emit if highlight changed
    if new_highlighted != highlight_state.highlighted_nation {
        highlight_state.highlighted_nation = new_highlighted;
        backend_messages.write(BackendMessage::HighlightNation { nation_id: new_highlighted });
    }
}

/// Resource to track previous ship states for delta updates
#[derive(Resource, Default)]
pub struct ShipStateTracker {
    /// Map of ship ID to current_path_index
    ship_indices: HashMap<u32, u32>,
}

/// System that emits ship update variants to the frontend (delta-based)
/// - Create: sent when ship first appears
/// - Move: sent only when current_path_index changes
/// - Destroy: sent when ship disappears
pub fn emit_ships_update_system(current_turn: Option<Res<CurrentTurn>>, territory_manager: Res<TerritoryManager>, ships: Query<(&Ship, &ChildOf)>, player_query: Query<&NationId>, mut ship_tracker: ResMut<ShipStateTracker>, mut backend_messages: MessageWriter<BackendMessage>) {
    let Some(current_turn) = current_turn else {
        return;
    };

    let current_ship_ids: HashSet<u32> = ships.iter().map(|(ship, _)| ship.id).collect();

    let mut updates = Vec::new();

    // Detect destroyed ships
    for &ship_id in ship_tracker.ship_indices.keys() {
        if !current_ship_ids.contains(&ship_id) {
            updates.push(ShipUpdateVariant::Destroy { id: ship_id });
        }
    }

    // Detect new ships and moved ships
    for (ship, parent) in ships.iter() {
        let owner_id = player_query.get(parent.0).copied().unwrap_or(NationId::ZERO);

        // Convert path from U16Vec2 to tile indices
        let path: Vec<u32> = ship.path.iter().map(|pos| territory_manager.pos_to_index(*pos)).collect();

        match ship_tracker.ship_indices.get(&ship.id) {
            None => {
                // New ship - send Create
                updates.push(ShipUpdateVariant::Create { id: ship.id, owner_id, path, troops: ship.troops });
                ship_tracker.ship_indices.insert(ship.id, ship.current_path_index as u32);
            }
            Some(&prev_index) if prev_index != ship.current_path_index as u32 => {
                // Ship moved to next tile - send Move
                updates.push(ShipUpdateVariant::Move { id: ship.id, current_path_index: ship.current_path_index as u32 });
                ship_tracker.ship_indices.insert(ship.id, ship.current_path_index as u32);
            }
            _ => {
                // No change, do nothing
            }
        }
    }

    // Clean up destroyed ships from tracker
    ship_tracker.ship_indices.retain(|id, _| current_ship_ids.contains(id));

    // Only send if there are updates
    if !updates.is_empty() {
        backend_messages.write(BackendMessage::ShipsUpdate(ShipsUpdatePayload { turn: current_turn.turn.turn_number, updates }));
    }
}
