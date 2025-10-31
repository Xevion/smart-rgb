//! Ship system using ECS architecture.
//!
//! Ships are entities with parent-child relationships to players.
//! See systems.rs for launch/update/arrival systems.

mod components;
pub mod pathfinding;
pub mod systems;

pub use components::*;
pub use pathfinding::*;
pub use systems::*;

// Re-export ship constants from central location
pub use crate::game::core::constants::ships::*;
