//! Input event and message types

use bevy_ecs::message::Message;
use glam::{U16Vec2, Vec2};
use serde::{Deserialize, Serialize};

use super::types::{ButtonState, KeyCode, MouseButton};

/// Raw input events sent by platforms via the input queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum InputEvent {
    /// Mouse button pressed or released
    MouseButton { button: MouseButton, state: ButtonState, tile: Option<U16Vec2>, world_pos: Vec2 },
    /// Mouse moved over the map
    MouseMotion { tile: Option<U16Vec2>, world_pos: Vec2 },
    /// Keyboard key pressed or released
    KeyEvent { key: KeyCode, state: ButtonState },
}

/// Mouse button message
#[derive(Message, Debug, Clone)]
pub struct MouseButtonMessage {
    pub button: MouseButton,
    pub state: ButtonState,
    pub tile: Option<U16Vec2>,
    pub world_pos: Vec2,
}

/// Mouse motion message
#[derive(Message, Debug, Clone)]
pub struct MouseMotionMessage {
    pub tile: Option<U16Vec2>,
    pub world_pos: Vec2,
}

/// Keyboard key message
#[derive(Message, Debug, Clone)]
pub struct KeyEventMessage {
    pub key: KeyCode,
    pub state: ButtonState,
}

/// A tile was clicked
#[derive(Message, Debug, Clone)]
pub struct TileClickedAction {
    pub tile: U16Vec2,
    pub button: MouseButton,
}

/// Camera-related actions
#[derive(Message, Debug, Clone)]
pub enum CameraAction {
    /// Center camera on player territory
    Center,
    /// Camera drag/interaction started
    InteractionStarted,
    /// Camera drag/interaction ended
    InteractionEnded,
}

/// UI-related actions
#[derive(Message, Debug, Clone)]
pub enum UiAction {
    /// Adjust attack ratio by amount
    UpdateAttackRatio { amount: f32 },
    /// Toggle pause
    TogglePause,
}
