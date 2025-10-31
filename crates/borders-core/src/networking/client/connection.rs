//! Unified connection abstraction for local and remote networking
//!
//! This module provides a generic `Connection` interface that abstracts away
//! the transport mechanism (local channels vs network). Game systems interact
//! with a single `Connection` resource regardless of whether the game is
//! single-player or multiplayer.

use std::collections::{HashMap, VecDeque};

use bevy_ecs::prelude::*;
use flume::{Receiver, Sender};
use tracing::{debug, warn};

use crate::game::NationId;
use crate::networking::{Intent, Turn, protocol::NetMessage};

/// Intent with tracking ID for confirmation
#[derive(Debug, Clone)]
pub struct TrackedIntent {
    pub id: u64,
    pub intent: Intent,
}

/// Resource for receiving tracked intents (local mode)
#[derive(Resource)]
pub struct TrackedIntentReceiver {
    pub rx: Receiver<TrackedIntent>,
}

/// Backend trait for connection implementations
///
/// This trait abstracts the transport mechanism, allowing both local
/// and remote connections to be used interchangeably.
pub trait ConnectionBackend: Send + Sync {
    /// Send an intent with tracking ID
    fn send_intent(&self, id: u64, intent: Intent);

    /// Get the player ID for this connection
    fn player_id(&self) -> Option<NationId>;

    /// Try to receive a turn (non-blocking)
    /// Returns None if no turn is available
    fn try_recv_turn(&self) -> Option<Turn>;
}

/// Local backend implementation (single-player)
pub struct LocalBackend {
    intent_tx: Sender<TrackedIntent>,
    turn_rx: Receiver<Turn>,
    player_id: NationId,
}

impl LocalBackend {
    pub fn new(intent_tx: Sender<TrackedIntent>, turn_rx: Receiver<Turn>, player_id: NationId) -> Self {
        Self { intent_tx, turn_rx, player_id }
    }
}

impl ConnectionBackend for LocalBackend {
    fn send_intent(&self, id: u64, intent: Intent) {
        let tracked = TrackedIntent { id, intent };
        if let Err(e) = self.intent_tx.try_send(tracked) {
            warn!("Failed to send tracked intent: {:?}", e);
        }
    }

    fn player_id(&self) -> Option<NationId> {
        Some(self.player_id)
    }

    fn try_recv_turn(&self) -> Option<Turn> {
        self.turn_rx.try_recv().ok()
    }
}

/// Remote backend implementation (multiplayer)
pub struct RemoteBackend {
    intent_tx: Sender<NetMessage>,
    net_message_rx: Receiver<NetMessage>,
    player_id: std::sync::Arc<std::sync::RwLock<Option<NationId>>>,
}

impl RemoteBackend {
    pub fn new(intent_tx: Sender<NetMessage>, net_message_rx: Receiver<NetMessage>) -> Self {
        Self { intent_tx, net_message_rx, player_id: std::sync::Arc::new(std::sync::RwLock::new(None)) }
    }

    /// Try to receive a NetMessage (for processing in receive system)
    pub fn try_recv_message(&self) -> Option<NetMessage> {
        self.net_message_rx.try_recv().ok()
    }

    /// Set player ID when ServerConfig is received
    pub fn set_player_id(&self, player_id: NationId) {
        if let Ok(mut guard) = self.player_id.write() {
            *guard = Some(player_id);
        }
    }
}

impl ConnectionBackend for RemoteBackend {
    fn send_intent(&self, id: u64, intent: Intent) {
        let msg = NetMessage::Intent { id, intent };
        if let Err(e) = self.intent_tx.try_send(msg) {
            warn!("Failed to send net intent: {:?}", e);
        }
    }

    fn player_id(&self) -> Option<NationId> {
        self.player_id.read().ok().and_then(|guard| *guard)
    }

    fn try_recv_turn(&self) -> Option<Turn> {
        // For remote, turn reception is handled by receive_messages_system
        // which processes NetMessage protocol
        None
    }
}

/// Pending intent tracking info
pub struct PendingIntent {
    pub intent: Intent,
    pub sent_turn: u64,
}

/// Intent tracker for drop detection
pub struct IntentTracker {
    next_id: u64,
    pending: HashMap<u64, PendingIntent>,
    pending_ordered: VecDeque<u64>,
    turn_buffer_size: usize,
}

impl IntentTracker {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            pending: HashMap::new(),
            pending_ordered: VecDeque::new(),
            turn_buffer_size: 5, // 500ms buffer at 100ms/turn
        }
    }

    pub fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn track_sent(&mut self, intent_id: u64, intent: Intent, current_turn: u64) {
        self.pending.insert(intent_id, PendingIntent { intent, sent_turn: current_turn });
        self.pending_ordered.push_back(intent_id);
    }

    pub fn confirm_intent(&mut self, intent_id: u64) -> bool {
        self.pending.remove(&intent_id).is_some()
    }

    pub fn expire_old(&mut self, current_turn: u64) -> Vec<PendingIntent> {
        let mut expired = Vec::new();

        while let Some(&id) = self.pending_ordered.front() {
            if let Some(pending) = self.pending.get(&id) {
                // Check if still within buffer window
                if current_turn.saturating_sub(pending.sent_turn) <= self.turn_buffer_size as u64 {
                    break;
                }
                // Expired - remove and warn
                expired.push(self.pending.remove(&id).unwrap());
            }
            // Either expired or already confirmed - remove from queue
            self.pending_ordered.pop_front();
        }

        expired
    }

    pub fn turn_buffer_size(&self) -> usize {
        self.turn_buffer_size
    }
}

impl Default for IntentTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Unified connection resource
///
/// This resource abstracts away local vs remote transport mechanisms.
/// Game systems interact with this single resource regardless of network mode.
#[derive(Resource)]
pub struct Connection {
    backend: Box<dyn ConnectionBackend>,
    tracker: IntentTracker,
}

impl Connection {
    pub fn new(backend: Box<dyn ConnectionBackend>) -> Self {
        Self { backend, tracker: IntentTracker::new() }
    }

    pub fn new_local(backend: LocalBackend) -> Self {
        Self::new(Box::new(backend))
    }

    pub fn new_remote(backend: RemoteBackend) -> Self {
        Self::new(Box::new(backend))
    }

    /// Send an intent with automatic tracking
    pub fn send_intent(&mut self, intent: Intent, current_turn: u64) {
        let intent_id = self.tracker.next_id();
        self.tracker.track_sent(intent_id, intent.clone(), current_turn);
        self.backend.send_intent(intent_id, intent);
    }

    /// Confirm receipt of an intent by ID
    pub fn confirm_intent(&mut self, intent_id: u64) -> bool {
        let confirmed = self.tracker.confirm_intent(intent_id);
        if confirmed {
            debug!("Confirmed intent {}", intent_id);
        }
        confirmed
    }

    /// Check for expired intents that weren't received
    pub fn check_expired(&mut self, current_turn: u64) -> Vec<PendingIntent> {
        self.tracker.expire_old(current_turn)
    }

    /// Get player ID for this connection
    pub fn player_id(&self) -> Option<NationId> {
        self.backend.player_id()
    }

    /// Try to receive a turn (works for local backend)
    pub fn try_recv_turn(&self) -> Option<Turn> {
        self.backend.try_recv_turn()
    }

    /// Get the turn buffer size for warning messages
    pub fn turn_buffer_size(&self) -> usize {
        self.tracker.turn_buffer_size()
    }

    /// Get mutable access to backend (for downcasting)
    pub fn backend_mut(&mut self) -> &mut dyn ConnectionBackend {
        &mut *self.backend
    }
}

/// Helper trait for downcasting backends
pub trait AsRemoteBackend {
    fn as_remote(&self) -> Option<&RemoteBackend>;
    fn as_remote_mut(&mut self) -> Option<&mut RemoteBackend>;
}

impl AsRemoteBackend for dyn ConnectionBackend {
    fn as_remote(&self) -> Option<&RemoteBackend> {
        // This is a simplified approach - in production you'd use Any trait
        None
    }

    fn as_remote_mut(&mut self) -> Option<&mut RemoteBackend> {
        // This is a simplified approach - in production you'd use Any trait
        None
    }
}
