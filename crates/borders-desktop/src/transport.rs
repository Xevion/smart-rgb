//! Tauri-specific frontend transport and command handlers
//!
//! This module provides the Tauri implementation of FrontendTransport,
//! along with Tauri command handlers for input events and state recovery.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use bevy_ecs::message::MessageReader;
use bevy_ecs::system::NonSend;
use borders_core::game::input::InputEvent;
use borders_core::ui::FrontendTransport;
use borders_core::ui::protocol::{BackendMessage, BinaryMessageType, FrontendMessage, LeaderboardSnapshot, encode_binary_message};
use tauri::{AppHandle, Emitter, ipc::Channel};

/// Storage for the unified binary channel used for streaming initialization and delta data
pub struct BinaryChannelStorage(pub Arc<Mutex<Option<Channel<Vec<u8>>>>>);

/// Tauri-specific frontend transport using Tauri channels for binary data
#[derive(Clone)]
pub struct TauriTransport {
    app_handle: AppHandle,
    /// Inbound messages from the frontend
    inbound_messages: Arc<Mutex<VecDeque<FrontendMessage>>>,
    /// Unified binary channel for streaming all binary data (init + deltas)
    binary_channel: Arc<Mutex<Option<Channel<Vec<u8>>>>>,
}

impl TauriTransport {
    pub fn new(app_handle: AppHandle) -> Self {
        Self { app_handle, inbound_messages: Arc::new(Mutex::new(VecDeque::new())), binary_channel: Arc::new(Mutex::new(None)) }
    }

    /// Get a reference to the inbound messages queue (for Tauri command handler)
    pub fn inbound_messages(&self) -> Arc<Mutex<VecDeque<FrontendMessage>>> {
        self.inbound_messages.clone()
    }

    /// Get the binary channel for registration
    pub fn binary_channel(&self) -> BinaryChannelStorage {
        BinaryChannelStorage(self.binary_channel.clone())
    }
}

impl FrontendTransport for TauriTransport {
    fn send_backend_message(&self, message: &BackendMessage) -> Result<(), String> {
        let _guard = tracing::trace_span!(
            "tauri_send_backend_message",
            message_type = match message {
                BackendMessage::LeaderboardSnapshot(_) => "LeaderboardSnapshot",
                BackendMessage::AttacksUpdate(_) => "AttacksUpdate",
                BackendMessage::ShipsUpdate(_) => "ShipsUpdate",
                BackendMessage::GameEnded { .. } => "GameEnded",
                BackendMessage::SpawnPhaseUpdate { .. } => "SpawnPhaseUpdate",
                BackendMessage::SpawnPhaseEnded => "SpawnPhaseEnded",
                BackendMessage::HighlightNation { .. } => "HighlightNation",
            }
        )
        .entered();

        self.app_handle.emit("backend:message", message).map_err(|e| format!("Failed to emit backend message: {}", e))
    }

    fn send_binary(&self, msg_type: BinaryMessageType, payload: Vec<u8>) -> Result<(), String> {
        let msg_type_str = match msg_type {
            BinaryMessageType::Init => "init",
            BinaryMessageType::Delta => "delta",
        };
        let _guard = tracing::trace_span!("tauri_send_binary", msg_type = msg_type_str, size = payload.len()).entered();

        // Encode with type tag envelope
        let data = encode_binary_message(msg_type, payload);

        let ch_lock = self.binary_channel.lock().map_err(|_| "Failed to lock binary channel")?;
        let channel = ch_lock.as_ref().ok_or("Binary channel not registered")?;

        channel.send(data).map_err(|e| format!("Failed to send binary data via channel: {}", e))
    }

    fn try_recv_frontend_message(&self) -> Option<FrontendMessage> {
        if let Ok(mut messages) = self.inbound_messages.lock() { messages.pop_front() } else { None }
    }
}

/// Tauri command to register the unified binary channel for streaming all binary data
#[tauri::command]
pub fn register_binary_channel(channel: Channel<Vec<u8>>, channel_storage: tauri::State<BinaryChannelStorage>) -> Result<(), String> {
    tracing::info!("Binary channel registered (handles init + deltas)");
    channel_storage
        .0
        .lock()
        .map_err(|_| {
            tracing::error!("Failed to acquire lock on binary channel storage");
            "Failed to acquire lock on binary channel storage".to_string()
        })?
        .replace(channel);
    Ok(())
}

/// Tauri command handler for receiving frontend messages
#[tauri::command]
pub fn send_frontend_message(message: FrontendMessage, bridge: tauri::State<Arc<Mutex<VecDeque<FrontendMessage>>>>) -> Result<(), String> {
    tracing::info!("Frontend sent message: {:?}", message);
    if let Ok(mut messages) = bridge.lock() {
        messages.push_back(message);
        tracing::debug!("Message queued, queue size: {}", messages.len());
        Ok(())
    } else {
        tracing::error!("Failed to acquire lock on message queue");
        Err("Failed to acquire lock on message queue".to_string())
    }
}

/// Handle input events from the frontend
#[tauri::command]
pub fn handle_render_input(event: InputEvent, input_sender: tauri::State<flume::Sender<InputEvent>>) -> Result<(), String> {
    // Send to input queue
    input_sender.send(event).map_err(|e| format!("Failed to send input event: {}", e))
}

/// Get current game state for frontend recovery after reload
/// Note: Initialization data (terrain, territory, nation palette) is not recoverable after reload.
/// The frontend must wait for a fresh game to start.
#[tauri::command]
pub fn get_game_state(leaderboard_state: tauri::State<Arc<Mutex<Option<LeaderboardSnapshot>>>>) -> Result<Option<LeaderboardSnapshot>, String> {
    leaderboard_state.lock().map(|state| state.clone()).map_err(|e| format!("Failed to lock leaderboard state: {}", e))
}

/// System to cache leaderboard snapshots for state recovery
pub fn cache_leaderboard_snapshot_system(mut events: MessageReader<BackendMessage>, shared_leaderboard_state: Option<NonSend<Arc<Mutex<Option<LeaderboardSnapshot>>>>>) {
    let Some(shared_state) = shared_leaderboard_state else {
        return;
    };

    for event in events.read() {
        if let BackendMessage::LeaderboardSnapshot(snapshot) = event
            && let Ok(mut state) = shared_state.lock()
        {
            *state = Some(snapshot.clone());
            tracing::trace!("Cached leaderboard snapshot for state recovery");
        }
    }
}
