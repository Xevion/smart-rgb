//! Server-side networking components
//!
//! This module contains server and local-mode server code:
//! - Turn generation and coordination
//! - Local server control (for single-player mode)

mod coordinator;
mod turn_generator;

pub use coordinator::*;
pub use turn_generator::*;
