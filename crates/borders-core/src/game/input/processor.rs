//! Input processor system - converts raw input into action messages

use bevy_ecs::prelude::*;

use crate::game::core::constants::input::*;

use super::events::*;
use super::types::{ButtonState, KeyCode, MouseButton};

/// Processes raw input messages and emits action messages
///
/// This system runs in PreUpdate and:
/// 1. Reads raw MouseButtonMessage, KeyEventMessage from the queue
/// 2. Filters out camera drags (clicks during middle mouse drag)
/// 3. Emits semantic action messages (TileClickedAction, CameraAction, UiAction)
///
/// Action handler systems in Update consume these action messages.
pub fn input_processor_system(mut mouse_events: MessageReader<MouseButtonMessage>, mut key_events: MessageReader<KeyEventMessage>, mut tile_clicked: MessageWriter<TileClickedAction>, mut camera_action: MessageWriter<CameraAction>, mut ui_action: MessageWriter<UiAction>, mut camera_dragging: Local<bool>) {
    // Process mouse button events
    for event in mouse_events.read() {
        match event.button {
            MouseButton::Left => match event.state {
                ButtonState::Pressed => {
                    // Reset camera drag flag on new press
                    *camera_dragging = false;
                }
                ButtonState::Released => {
                    // Only emit TileClicked if no camera drag occurred
                    if !*camera_dragging && let Some(tile) = event.tile {
                        tile_clicked.write(TileClickedAction { tile, button: event.button });
                    }
                    *camera_dragging = false;
                }
            },
            MouseButton::Middle => {
                if event.state == ButtonState::Pressed {
                    *camera_dragging = true;
                    camera_action.write(CameraAction::InteractionStarted);
                } else {
                    camera_action.write(CameraAction::InteractionEnded);
                }
            }
            _ => {}
        }
    }

    // Process keyboard events
    for event in key_events.read() {
        // Only process key presses (not releases)
        if event.state != ButtonState::Pressed {
            continue;
        }

        match event.key {
            KeyCode::KeyC => {
                camera_action.write(CameraAction::Center);
            }
            KeyCode::Digit1 => {
                ui_action.write(UiAction::UpdateAttackRatio { amount: -ATTACK_RATIO_STEP });
            }
            KeyCode::Digit2 => {
                ui_action.write(UiAction::UpdateAttackRatio { amount: ATTACK_RATIO_STEP });
            }
            KeyCode::Space => {
                ui_action.write(UiAction::TogglePause);
            }
            _ => {}
        }
    }
}
