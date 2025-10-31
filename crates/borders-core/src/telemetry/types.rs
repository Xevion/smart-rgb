use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Represents a telemetry event to be sent to PostHog.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryEvent {
    /// Unique event identifier (e.g., "app_started", "game_ended")
    pub event: String,

    /// Properties associated with this event
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub properties: HashMap<String, serde_json::Value>,
}

impl TelemetryEvent {
    pub fn new(event: impl Into<String>) -> Self {
        Self { event: event.into(), properties: HashMap::new() }
    }

    pub fn with_property(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.properties.insert(key.into(), value.into());
        self
    }
}

/// Configuration for the telemetry client.
#[derive(Debug, Clone)]
pub struct TelemetryConfig {
    /// PostHog API key
    pub api_key: String,

    /// API host (e.g., "observe.borders.xevion.dev")
    pub api_host: String,

    /// Batch size - send events when this many are queued
    pub batch_size: usize,

    /// Flush interval in seconds
    pub flush_interval_secs: u64,

    /// HMAC signing key for request integrity verification
    pub signing_key: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        // In development: send often with small batch size for fast feedback
        // In production: batch events but flush periodically to avoid losing data
        #[cfg(debug_assertions)]
        let (batch_size, flush_interval_secs) = (2, 5);
        #[cfg(not(debug_assertions))]
        let (batch_size, flush_interval_secs) = (10, 45);

        Self {
            api_key: "phc_VmL3M9Sn9hBCpNRExnKLWOZqlYO5SXSUkAAwl3gXJek".to_string(),
            api_host: "observe.xevion.dev".to_string(),
            batch_size,
            flush_interval_secs,
            // HMAC-SHA256 signing key for request integrity
            signing_key: "borders_telemetry_hmac_key_v1_2025".to_string(),
        }
    }
}

/// PostHog batch capture request payload
#[derive(Debug, Serialize)]
pub(crate) struct BatchCaptureRequest {
    pub api_key: String,
    pub batch: Vec<BatchEvent>,
}

#[derive(Debug, Serialize)]
pub(crate) struct BatchEvent {
    pub event: String,
    pub properties: HashMap<String, serde_json::Value>,
    pub distinct_id: String,
}
