use borders_core::game::NationId;
use borders_core::networking::NetMessage;
use flume::Sender;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use tracing::error;

/// Connection information for a client
#[derive(Debug, Clone)]
pub struct ClientConnection {
    pub id: NationId,
    pub tx: Sender<NetMessage>,
}

/// Registry for managing client connections and broadcasting messages
pub struct ServerRegistry {
    connections: Arc<RwLock<HashMap<NationId, ClientConnection>>>,
    next_nation_id: Arc<RwLock<u16>>,
}

impl Default for ServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerRegistry {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            next_nation_id: Arc::new(RwLock::new(1)), // Start from 1, 0 reserved
        }
    }

    /// Add a new client connection and return assigned nation ID
    pub fn add_client(&self, tx: Sender<NetMessage>) -> NationId {
        let mut next_id = self.next_nation_id.write().unwrap();
        let nation_id = NationId::new_unchecked(*next_id);
        *next_id += 1;

        let connection = ClientConnection { id: nation_id, tx };
        self.connections.write().unwrap().insert(nation_id, connection);

        nation_id
    }

    /// Remove a client connection
    pub fn remove_client(&self, nation_id: NationId) {
        self.connections.write().unwrap().remove(&nation_id);
    }

    /// Broadcast a message to all connected clients
    pub fn broadcast(&self, message: NetMessage) {
        let connections = self.connections.read().unwrap();
        for connection in connections.values() {
            if let Err(e) = connection.tx.send(message.clone()) {
                error!("Failed to send message to client {}: {}", connection.id, e);
            }
        }
    }

    /// Get the number of connected clients
    pub fn client_count(&self) -> usize {
        self.connections.read().unwrap().len()
    }
}
