//! WASM-JS bridge for game communication
//!
//! This module provides shared state and utilities for WASM bindings.

use std::collections::HashMap;
use std::sync::Mutex;
use wasm_bindgen::prelude::{JsValue, wasm_bindgen};

// Global input sender (needs to be accessible from WASM bindings)
lazy_static::lazy_static! {
    static ref INPUT_SENDER: Mutex<Option<flume::Sender<borders_core::game::input::InputEvent>>> =
        Mutex::new(None);
}

/// Set the global input sender (called during game initialization)
pub fn set_input_sender(sender: flume::Sender<borders_core::game::input::InputEvent>) {
    *INPUT_SENDER.lock().unwrap() = Some(sender);
}

/// Get the global input sender (for WASM bindings to send input)
pub fn get_input_sender() -> Option<flume::Sender<borders_core::game::input::InputEvent>> {
    INPUT_SENDER.lock().unwrap().clone()
}

/// Track an analytics event (separate from game protocol)
#[wasm_bindgen]
pub fn track_analytics_event(event: JsValue) -> Result<(), JsValue> {
    #[derive(serde::Deserialize)]
    struct AnalyticsEventPayload {
        event: String,
        #[serde(default)]
        properties: HashMap<String, serde_json::Value>,
    }

    let payload: AnalyticsEventPayload = serde_wasm_bindgen::from_value(event).map_err(|e| JsValue::from_str(&format!("Failed to deserialize analytics event: {}", e)))?;

    let telemetry_event = borders_core::telemetry::TelemetryEvent { event: payload.event, properties: payload.properties };

    // Spawn a task to track the event asynchronously
    wasm_bindgen_futures::spawn_local(async move {
        borders_core::telemetry::track(telemetry_event).await;
    });

    Ok(())
}
