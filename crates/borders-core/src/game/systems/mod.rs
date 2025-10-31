//! Game systems that run each tick/turn
//!
//! This module contains systems that execute game logic.

use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;

use crate::game::entities::NationEntityMap;
use crate::game::terrain::data::TerrainData;

pub mod borders;
pub mod income;
pub mod spawn;
pub mod spawn_territory;
pub mod spawn_timeout;
pub mod turn;
pub mod turn_actions;
pub mod turn_attacks;
pub mod turn_spawns;

// Re-export system functions and types
pub use borders::*;
pub use income::*;
pub use spawn::*;
pub use spawn_territory::*;
pub use spawn_timeout::*;
pub use turn::*;
pub use turn_actions::process_and_apply_actions_system;
pub use turn_attacks::tick_attacks_system;
pub use turn_spawns::handle_spawns_system;

/// SystemParam to group read-only game resources
/// Used across multiple turn execution systems
#[derive(SystemParam)]
pub struct GameResources<'w> {
    pub border_cache: Res<'w, BorderCache>,
    pub nation_entity_map: Res<'w, NationEntityMap>,
    pub terrain: Res<'w, TerrainData>,
}
