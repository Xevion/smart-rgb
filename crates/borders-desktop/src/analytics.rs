use borders_core::telemetry::{self, TelemetryEvent};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tauri::Manager;

/// Analytics event from the frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyticsEventPayload {
    pub event: String,
    #[serde(default)]
    pub properties: HashMap<String, serde_json::Value>,
}

/// Tauri command to track analytics events from the frontend
#[tauri::command]
pub async fn track_analytics_event(payload: AnalyticsEventPayload) -> Result<(), String> {
    tracing::debug!("Tracking analytics event: {}", payload.event);

    let event = TelemetryEvent { event: payload.event, properties: payload.properties };

    // Track the event asynchronously (Tauri handles the async context)
    telemetry::track(event).await;

    Ok(())
}

/// Tauri command to flush pending analytics events
#[tauri::command]
pub async fn flush_analytics() -> Result<(), String> {
    if let Some(client) = telemetry::client() {
        client.flush().await;
        Ok(())
    } else {
        Err("Telemetry client not initialized".to_string())
    }
}

/// Tauri command to request app exit
///
/// Simply closes the window - analytics flush happens in ExitRequested event handler
#[tauri::command]
pub async fn request_exit(app_handle: tauri::AppHandle) -> Result<(), String> {
    tracing::debug!("Exit requested via command");

    // Close the window (will trigger ExitRequested event → analytics flush)
    if let Some(window) = app_handle.get_webview_window("main") {
        window.close().map_err(|e| e.to_string())?;
    }

    Ok(())
}
