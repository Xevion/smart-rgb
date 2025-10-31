//! Game action system
//!
//! This module defines the core action types that can be performed in the game.
//! Actions represent discrete game events that can be initiated by both human players
//! and AI bots. They are processed deterministically during turn execution.

use rkyv::{Archive, Deserialize as RkyvDeserialize, Serialize as RkyvSerialize};
use serde::{Deserialize, Serialize};

use crate::game::core::utils::u16vec2_serde;
use crate::game::world::NationId;

/// Core game action type
///
/// This enum represents all possible actions that can be performed in the game.
/// Unlike `Intent`, which is a network-layer wrapper, `GameAction` is the actual
/// game-level operation.
///
/// Actions can originate from:
/// - Players (via input systems → intents → network → SourcedIntent wrapper)
/// - Bots (calculated deterministically during turn execution)
///
/// Nation identity is provided separately:
/// - For players: wrapped in SourcedIntent by server (prevents spoofing)
/// - For bots: generated with id in turn execution context
///
/// Note: Spawning is handled separately via Turn(0) and direct spawn manager updates,
/// not through the action system.
#[derive(Debug, Clone, Serialize, Deserialize, Archive, RkyvSerialize, RkyvDeserialize)]
#[rkyv(derive(Debug))]
pub enum GameAction {
    /// Attack a target nation with a specified number of troops
    ///
    /// The attack will proceed across all borders shared with the target:
    /// - `target: Some(nation_id)` - Attack specific nation across all shared borders
    /// - `target: None` - Expand into unclaimed territory from all borders
    Attack { target: Option<NationId>, troops: u32 },
    /// Launch a transport ship to attack across water
    LaunchShip {
        #[serde(with = "u16vec2_serde")]
        target_tile: glam::U16Vec2,
        troops: u32,
    },
    // Future action types:
    // BuildStructure { target: U16Vec2, structure_type: StructureType },
    // LaunchNuke { target: U16Vec2 },
    // RequestAlliance { target: NationId },
    // DeclareWar { target: NationId },
}

/// Troop count specification for attacks
pub enum TroopCount {
    /// Use a ratio of the nation's current troops (0.0-1.0)
    Ratio(f32),
    /// Use an absolute troop count
    Absolute(u32),
}
