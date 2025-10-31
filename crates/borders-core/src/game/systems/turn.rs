use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemState;

use crate::networking::{ProcessTurnEvent, Turn};

/// Marker resource indicating a turn is actively being processed
/// Added when a new turn arrives, removed after all turn-based systems complete
#[derive(Resource)]
pub struct ActiveTurn {
    pub turn_number: u64,
}

impl ActiveTurn {
    pub fn new(turn: &Turn) -> Self {
        Self { turn_number: turn.turn_number }
    }
}

/// Resource containing the current turn data
/// Updated once per turn (10 TPS), provides turn context to all gameplay systems
#[derive(Resource)]
pub struct CurrentTurn {
    pub active: bool,
    pub turn: Turn,
}

impl CurrentTurn {
    pub fn new(turn: Turn) -> Self {
        Self { active: true, turn }
    }
}

/// System to receive turn events and update CurrentTurn resource
/// Exclusive system to ensure ActiveTurn is inserted immediately (not deferred)
pub fn update_current_turn_system(world: &mut World) {
    // Create a SystemState to read messages in exclusive system
    let mut system_state: SystemState<MessageReader<ProcessTurnEvent>> = SystemState::new(world);
    let mut turn_events = system_state.get_mut(world);

    // Read all turn events (should only be one per frame at 10 TPS)
    let turns: Vec<Turn> = turn_events.read().map(|e| e.0.clone()).collect();

    // Apply the state back to world
    system_state.apply(world);

    if turns.is_empty() {
        return;
    }

    // Take the latest turn (in case multiple arrived, though this shouldn't happen)
    let turn = turns.into_iter().last().unwrap();

    // Insert ActiveTurn immediately to signal turn is being processed
    world.insert_resource(ActiveTurn::new(&turn));

    // Update CurrentTurn (must always exist)
    let mut current_turn = world.get_resource_mut::<CurrentTurn>().expect("CurrentTurn must be initialized in GameBuilder::build()");
    current_turn.active = true;
    current_turn.turn = turn;
}

/// System to cleanup ActiveTurn and CurrentTurn resources after all turn-based systems complete
pub fn turn_cleanup_system(mut current_turn: ResMut<CurrentTurn>, mut commands: Commands) {
    if current_turn.active {
        commands.remove_resource::<ActiveTurn>();
        current_turn.active = false;
    }
}
