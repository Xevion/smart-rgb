// Turn generation behavior tests: spawn timeout, turn intervals, intent validation

use assert2::assert;
use borders_core::prelude::*;
use rstest::rstest;

#[test]
fn test_spawn_phase_timeout() {
    let mut generator = server::SharedTurnGenerator::new();

    let sourced_intent = SourcedIntent { source: NationId::new_unchecked(1), intent_id: 1, intent: Intent::SetSpawn { tile_index: U16Vec2::new(5, 5) } };
    let output = generator.process_intent(sourced_intent);
    assert!(matches!(output, server::TurnOutput::SpawnUpdate(_)));

    let output = generator.tick(2000.0, vec![]);
    assert!(matches!(output, server::TurnOutput::None));

    let output = generator.tick(3500.0, vec![]);
    assert!(matches!(&output, server::TurnOutput::Turn(turn) if turn.turn_number == 0), "Expected Turn(0) after spawn timeout, got {:?}", output);
    assert!(generator.game_started());
}

#[test]
fn test_turn_generation() {
    let mut generator = server::SharedTurnGenerator::new();

    generator.process_intent(SourcedIntent { source: NationId::new_unchecked(1), intent_id: 1, intent: Intent::SetSpawn { tile_index: U16Vec2::new(5, 5) } });
    generator.tick(6000.0, vec![]);

    let output = generator.tick(50.0, vec![]);
    assert!(matches!(output, server::TurnOutput::None));

    let sourced_intents = vec![SourcedIntent { source: NationId::new_unchecked(1), intent_id: 2, intent: Intent::Action(GameAction::Attack { target: Some(NationId::new_unchecked(2)), troops: 100 }) }];
    let output = generator.tick(60.0, sourced_intents);
    if let server::TurnOutput::Turn(turn) = output {
        assert!(turn.turn_number == 1);
        assert!(turn.intents.len() == 1);
    } else {
        panic!("Expected TurnOutput::Turn");
    }
}

#[test]
fn test_ignore_action_during_spawn() {
    let mut generator = server::SharedTurnGenerator::new();

    let sourced_intent = SourcedIntent { source: NationId::new_unchecked(1), intent_id: 1, intent: Intent::Action(GameAction::Attack { target: None, troops: 50 }) };
    let output = generator.process_intent(sourced_intent);
    assert!(matches!(output, server::TurnOutput::None));
}

/// Test duplicate intent ID handling
///
/// Verifies that duplicate intent submissions (2x or 3x) are properly rejected
#[rstest]
#[case::double_submission(vec![50, 100])]
#[case::triple_submission(vec![50, 100, 150])]
fn test_duplicate_intent_id_submissions(#[case] troop_values: Vec<u32>) {
    let mut generator = server::SharedTurnGenerator::new();

    // Start game first
    generator.process_intent(SourcedIntent { source: NationId::new_unchecked(1), intent_id: 1, intent: Intent::SetSpawn { tile_index: U16Vec2::new(5, 5) } });
    generator.tick(6000.0, vec![]);

    // Submit first intent
    let first_intent = SourcedIntent { source: NationId::new_unchecked(1), intent_id: 2, intent: Intent::Action(GameAction::Attack { target: None, troops: troop_values[0] }) };
    generator.process_intent(first_intent);

    // Submit duplicate intents with same ID but different troop values
    for &troops in &troop_values[1..] {
        let duplicate_intent = SourcedIntent { source: NationId::new_unchecked(1), intent_id: 2, intent: Intent::Action(GameAction::Attack { target: None, troops }) };
        let output = generator.process_intent(duplicate_intent);
        assert!(matches!(output, server::TurnOutput::None));
    }
}

#[test]
fn test_duplicate_intent_id() {
    let mut generator = server::SharedTurnGenerator::new();

    generator.process_intent(SourcedIntent { source: NationId::new_unchecked(1), intent_id: 1, intent: Intent::SetSpawn { tile_index: U16Vec2::new(5, 5) } });
    generator.tick(6000.0, vec![]);

    let sourced_intent1 = SourcedIntent { source: NationId::new_unchecked(1), intent_id: 2, intent: Intent::Action(GameAction::Attack { target: None, troops: 50 }) };
    let sourced_intent2 = SourcedIntent { source: NationId::new_unchecked(1), intent_id: 2, intent: Intent::Action(GameAction::Attack { target: None, troops: 100 }) };

    generator.process_intent(sourced_intent1);
    let output = generator.process_intent(sourced_intent2);
    assert!(matches!(output, server::TurnOutput::None));
}
