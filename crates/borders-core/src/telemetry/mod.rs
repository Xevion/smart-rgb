//! Telemetry module for tracking analytics events.
//!
//! This module provides a simple, cross-platform telemetry client that works
//! on both native (Tauri) and WASM targets. Events are batched and sent to
//! PostHog via HTTP in a non-blocking manner.

mod client;

mod system_info;
mod types;
mod user_id;

pub use client::*;
pub use system_info::*;
pub use types::*;
pub use user_id::*;

use once_cell::sync::OnceCell;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global telemetry client instance.
static TELEMETRY_CLIENT: OnceCell<TelemetryClient> = OnceCell::new();

/// Session start timestamp in milliseconds since epoch (for calculating session duration).
static SESSION_START_MS: AtomicU64 = AtomicU64::new(0);

/// Initialize the global telemetry client with the given configuration.
///
/// This should be called once at application startup.
/// On WASM, this is async to load the user ID from IndexedDB.
pub async fn init(config: TelemetryConfig) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let client = TelemetryClient::new(config);
        if TELEMETRY_CLIENT.set(client).is_err() {
            tracing::warn!("Telemetry client already initialized");
        }
    }

    #[cfg(target_arch = "wasm32")]
    {
        let (user_id, id_type) = get_or_create_user_id_async().await;
        let client = TelemetryClient::new_with_user_id(config, user_id, id_type);
        if TELEMETRY_CLIENT.set(client).is_err() {
            tracing::warn!("Telemetry client already initialized");
        }
    }
}

/// Get a reference to the global telemetry client.
///
/// Returns None if the client hasn't been initialized yet.
pub fn client() -> Option<&'static TelemetryClient> {
    TELEMETRY_CLIENT.get()
}

/// Track a telemetry event using the global client.
///
/// This is a convenience function that will do nothing if the client
/// hasn't been initialized.
pub async fn track(event: TelemetryEvent) {
    if let Some(client) = client() {
        client.track(event).await;
    }
}

/// Track a session start event with detailed system information.
///
/// Should be called once after telemetry initialization.
pub async fn track_session_start() {
    // Record session start time for duration calculation
    let now_ms = current_time_ms();
    SESSION_START_MS.store(now_ms, Ordering::Relaxed);

    let system_info = SystemInfo::collect();
    let mut event = TelemetryEvent::new("session_start");

    for (key, value) in system_info.to_properties() {
        event.properties.insert(key, value);
    }

    #[cfg(target_arch = "wasm32")]
    {
        let (browser_name, browser_version) = system_info::get_browser_info();
        event.properties.insert("browser_name".to_string(), serde_json::Value::String(browser_name));
        event.properties.insert("browser_version".to_string(), serde_json::Value::String(browser_version));
    }

    track(event).await;
}

/// Track a session end event with session duration.
///
/// Should be called when the application is closing.
pub async fn track_session_end() {
    let start_ms = SESSION_START_MS.load(Ordering::Relaxed);
    if start_ms == 0 {
        tracing::warn!("Session end tracked but no session start found");
        return;
    }

    let now_ms = current_time_ms();
    let duration_ms = now_ms.saturating_sub(start_ms);
    let duration_secs = duration_ms / 1000;

    let event = TelemetryEvent::new("session_end").with_property("session_duration_ms", duration_ms).with_property("session_duration_secs", duration_secs);

    track(event).await;
}

/// Get current time in milliseconds since Unix epoch.
fn current_time_ms() -> u64 {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64
    }

    #[cfg(target_arch = "wasm32")]
    {
        (js_sys::Date::now()) as u64
    }
}
