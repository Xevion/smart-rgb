//! Networking and multiplayer synchronization
//!
//! This module provides the core networking infrastructure for the game:
//! - Shared protocol and data structures
//! - Client-side connection and systems
//! - Server-side turn generation and coordination

// Public modules
pub mod client;
pub mod server;

// Flattened public modules
mod protocol;
mod types;
pub use protocol::*;
pub use types::*;
