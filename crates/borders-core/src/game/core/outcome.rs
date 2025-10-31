use crate::game::core::constants::outcome::WIN_THRESHOLD;
use crate::game::entities::{Dead, NationName, TerritorySize};
use crate::game::input::context::LocalPlayerContext;
use crate::game::{NationEntityMap, NationId, TerritoryManager};
use crate::ui::protocol::{BackendMessage, GameOutcome};
use bevy_ecs::prelude::*;
use tracing::info;

/// System that checks if the local player has won or lost
pub fn check_local_player_outcome(mut local_context: If<ResMut<LocalPlayerContext>>, territory_manager: Res<TerritoryManager>, active_nations: Query<(&NationId, &TerritorySize, &NationName), Without<Dead>>, nation_entity_map: Res<NationEntityMap>, mut backend_messages: MessageWriter<BackendMessage>) {
    // Don't check if outcome already determined
    if local_context.my_outcome.is_some() {
        return;
    }

    let my_player_id = local_context.id;

    // Get local player entity and stats
    let Some(&my_entity) = nation_entity_map.0.get(&my_player_id) else {
        return;
    };

    let Ok((_, my_territory_size, _)) = active_nations.get(my_entity) else {
        // Query failed but entity exists in entity_map, so player must have Dead marker
        info!("Local player defeated - eliminated (Dead marker)");
        local_context.mark_defeated();
        backend_messages.write(BackendMessage::GameEnded { outcome: GameOutcome::Defeat });
        return;
    };

    let my_tiles = my_territory_size.0;

    // Don't check outcome until player has spawned (has tiles)
    if my_tiles == 0 {
        return;
    }

    // Calculate total claimable tiles for victory condition checks
    let total_claimable_tiles = crate::game::queries::count_land_tiles(&territory_manager);

    if total_claimable_tiles > 0 {
        let my_occupation = my_tiles as f32 / total_claimable_tiles as f32;

        // Check if I've won by occupation
        if my_occupation >= WIN_THRESHOLD {
            info!("Local player victorious - reached {:.1}% occupation ({}/{} claimable tiles, threshold: {:.0}%)", my_occupation * 100.0, my_tiles, total_claimable_tiles, WIN_THRESHOLD * 100.0);
            local_context.mark_victorious();
            backend_messages.write(BackendMessage::GameEnded { outcome: GameOutcome::Victory });
            return;
        }

        // Check if any opponent has won by occupation (which means I lost)
        for (id, territory, name) in active_nations.iter() {
            if *id == my_player_id {
                continue;
            }

            let occupation = territory.0 as f32 / total_claimable_tiles as f32;

            if occupation >= WIN_THRESHOLD {
                tracing::event!(target:module_path!(),tracing::Level::INFO,"Local player defeated - {} reached {:.1}% occupation ({}/{} claimable tiles, threshold: {:.0}%)",name.0,occupation*100.0,territory.0,total_claimable_tiles,WIN_THRESHOLD*100.0);
                local_context.mark_defeated();
                backend_messages.write(BackendMessage::GameEnded { outcome: GameOutcome::Defeat });
                return;
            }
        }
    }

    // Check victory by eliminating all opponents
    // If no living opponents remain (query filters Without<Dead>), then I've won
    let any_opponents_alive = active_nations.iter().any(|(opponent_id, _, _)| *opponent_id != my_player_id);

    if !any_opponents_alive {
        info!("Local player victorious - all opponents eliminated");
        local_context.mark_victorious();
        backend_messages.write(BackendMessage::GameEnded { outcome: GameOutcome::Victory });
    }
}
