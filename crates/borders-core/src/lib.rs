pub mod build_info;
pub mod game;
pub mod networking;
pub mod telemetry;
pub mod time;
pub mod ui;

use std::future::Future;

/// Spawn an async task on the appropriate runtime for the platform.
///
/// On native targets, uses tokio::spawn for multi-threaded execution.
/// On WASM targets, uses wasm_bindgen_futures::spawn_local for browser integration.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + Send + 'static,
{
    tokio::spawn(future);
}

#[cfg(target_arch = "wasm32")]
pub fn spawn_task<F>(future: F)
where
    F: Future<Output = ()> + 'static,
{
    wasm_bindgen_futures::spawn_local(future);
}

#[cfg(not(target_arch = "wasm32"))]
pub mod dns;

pub mod prelude {
    //! Prelude module for convenient imports in tests and examples
    pub use crate::game::*;
    pub use crate::networking::*;
    pub use crate::spawn_task;
    pub use crate::time::*;

    // Re-export common external dependencies
    pub use bevy_ecs::prelude::*;
    pub use glam::*;
}
