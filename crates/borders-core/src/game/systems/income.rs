use bevy_ecs::prelude::*;
use tracing::trace;

use crate::game::ai::bot::Bot;
use crate::game::entities;
use crate::game::{ActiveTurn, CurrentTurn, Dead, TerritorySize, Troops};

/// Process player income at 10 TPS (once per turn)
/// Uses If<Res<ActiveTurn>> to skip when no active turn
///
/// Uses Has<Bot> to distinguish bot vs human players:
/// - true = bot player (60% income, 33% max troops)
/// - false = human player (100% income, 100% max troops)
pub fn process_nation_income_system(_active_turn: If<Res<ActiveTurn>>, current_turn: Res<CurrentTurn>, mut players: Query<(&mut Troops, &TerritorySize, Has<Bot>), Without<Dead>>) {
    // Skip income processing on Turn 0 - players haven't spawned yet
    // Spawning happens during execute_turn_gameplay_system on Turn 0
    if current_turn.turn.turn_number == 0 {
        trace!("Skipping income on Turn 0 (pre-spawn)");
        return;
    }

    // Process income for all alive players (Without<Dead> filter)
    for (mut troops, territory_size, is_bot) in &mut players {
        // Calculate and apply income
        let income = entities::calculate_income(troops.0, territory_size.0, is_bot);
        troops.0 = entities::add_troops_capped(troops.0, income, territory_size.0, is_bot);
    }

    trace!("Income processed for turn {}", current_turn.turn.turn_number);
}
