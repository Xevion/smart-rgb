use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use futures::lock::Mutex;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tracing::{debug, error, warn};

use super::types::{BatchCaptureRequest, BatchEvent, TelemetryConfig, TelemetryEvent};
use super::user_id::UserIdType;
use crate::spawn_task;

#[cfg(not(target_arch = "wasm32"))]
use super::user_id::get_or_create_user_id;

type HmacSha256 = Hmac<Sha256>;

/// Build an HTTP client with appropriate DNS resolver for the platform.
///
/// On non-WASM targets, attempts to use Hickory DNS with DoH support.
/// Falls back to default client if DoH initialization fails.
#[cfg(not(target_arch = "wasm32"))]
fn build_http_client() -> reqwest::Client {
    match reqwest::Client::builder().dns_resolver(Arc::new(crate::dns::HickoryDnsResolver::new())).build() {
        Ok(client) => {
            debug!("HTTP client initialized with DoH resolver");
            client
        }
        Err(e) => {
            warn!("Failed to build HTTP client with DoH: {}, using default", e);
            reqwest::Client::new()
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn build_http_client() -> reqwest::Client {
    reqwest::Client::new()
}

/// A simple telemetry client that batches events and sends them to PostHog.
///
/// This client works on both native and WASM targets by using reqwest
/// with appropriate feature flags.
#[derive(Clone)]
pub struct TelemetryClient {
    config: TelemetryConfig,
    client: reqwest::Client,
    /// Distinct ID for this client instance (anonymous user ID)
    distinct_id: String,
    /// Lightweight properties attached to every event
    default_properties: HashMap<String, serde_json::Value>,
    /// Event buffer for batching
    buffer: Arc<Mutex<Vec<TelemetryEvent>>>,
    /// Whether the flush task has been started
    flush_task_started: Arc<AtomicBool>,
    /// Track in-flight batch sends (native only)
    #[cfg(not(target_arch = "wasm32"))]
    in_flight_sends: Arc<tokio::sync::Mutex<Vec<tokio::task::JoinHandle<()>>>>,
}

impl TelemetryClient {
    /// Create a new telemetry client with the given configuration.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn new(config: TelemetryConfig) -> Self {
        let (distinct_id, id_type) = get_or_create_user_id();
        debug!("Telemetry client initialized (user ID type: {})", id_type.as_str());

        let default_properties = build_default_properties(id_type);

        Self { config, client: build_http_client(), distinct_id, default_properties, buffer: Arc::new(Mutex::new(Vec::new())), flush_task_started: Arc::new(AtomicBool::new(false)), in_flight_sends: Arc::new(tokio::sync::Mutex::new(Vec::new())) }
    }

    /// Create a new telemetry client with a pre-loaded user ID.
    ///
    /// This is used on WASM where user ID loading is async.
    pub fn new_with_user_id(config: TelemetryConfig, distinct_id: String, id_type: UserIdType) -> Self {
        debug!("Telemetry client initialized (user ID type: {})", id_type.as_str());

        let default_properties = build_default_properties(id_type);

        Self {
            config,
            client: build_http_client(),
            distinct_id,
            default_properties,
            buffer: Arc::new(Mutex::new(Vec::new())),
            flush_task_started: Arc::new(AtomicBool::new(false)),
            #[cfg(not(target_arch = "wasm32"))]
            in_flight_sends: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        }
    }

    /// Start a background task that periodically flushes events.
    /// This ensures events are sent even if the batch size isn't reached.
    ///
    /// Only starts once, subsequent calls are no-ops.
    fn ensure_flush_task_started(&self) {
        // Check if already started (fast path)
        if self.flush_task_started.load(Ordering::Acquire) {
            return;
        }

        // Try to start the task (only one thread will succeed)
        if self.flush_task_started.compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire).is_err() {
            // Another thread beat us to it
            return;
        }

        // We won the race, start the task
        let client = self.clone();
        let interval_secs = self.config.flush_interval_secs;

        spawn_task(async move {
            #[cfg(not(target_arch = "wasm32"))]
            {
                use std::time::Duration;
                let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
                loop {
                    interval.tick().await;
                    client.flush().await;
                }
            }
            #[cfg(target_arch = "wasm32")]
            {
                use gloo_timers::future::TimeoutFuture;
                loop {
                    TimeoutFuture::new((interval_secs * 1000) as u32).await;
                    client.flush().await;
                }
            }
        });
        debug!("Started periodic flush task (interval: {}s)", interval_secs);
    }

    /// Track a telemetry event. Events are buffered and sent in batches.
    pub async fn track(&self, event: TelemetryEvent) {
        // Ensure the periodic flush task is running (lazy start)
        self.ensure_flush_task_started();

        debug!("Buffering telemetry event: {}", event.event);

        let mut buffer = self.buffer.lock().await;
        buffer.push(event);

        // Check if we should flush based on batch size
        if buffer.len() >= self.config.batch_size {
            debug!("Batch size reached ({}), flushing events", buffer.len());
            let events_to_send = buffer.drain(..).collect::<Vec<_>>();
            drop(buffer); // Release lock before async operation

            // Spawn a task to send in the background (non-blocking)
            let client = self.clone();

            #[cfg(not(target_arch = "wasm32"))]
            {
                let handle = tokio::spawn(async move {
                    client.send_batch(events_to_send).await;
                });
                // Track the in-flight send and clean up completed tasks
                let mut in_flight = self.in_flight_sends.lock().await;
                in_flight.retain(|h| !h.is_finished());
                in_flight.push(handle);
            }

            #[cfg(target_arch = "wasm32")]
            spawn_task(async move {
                client.send_batch(events_to_send).await;
            });
        }
    }

    /// Manually flush all buffered events.
    ///
    /// This method waits for all in-flight sends to complete, then sends any remaining buffered events.
    pub async fn flush(&self) {
        // First, wait for all in-flight background sends to complete
        #[cfg(not(target_arch = "wasm32"))]
        {
            let handles = {
                let mut in_flight = self.in_flight_sends.lock().await;
                in_flight.drain(..).collect::<Vec<_>>()
            };

            if !handles.is_empty() {
                debug!("Waiting for {} in-flight batch sends to complete", handles.len());
                for handle in handles {
                    let _ = handle.await;
                }
            }
        }

        // Then flush any remaining buffered events
        let events_to_send = {
            let mut buffer = self.buffer.lock().await;

            if buffer.is_empty() {
                return;
            }

            let events = buffer.drain(..).collect::<Vec<_>>();
            debug!("Flushing {} buffered events", events.len());
            events
        };

        // Send synchronously (wait for completion)
        self.send_batch(events_to_send).await;
    }

    /// Generate HMAC-SHA256 signature for request payload.
    ///
    /// This prevents tampering and verifies request integrity.
    fn sign_payload(&self, payload: &[u8]) -> String {
        let mut mac = HmacSha256::new_from_slice(self.config.signing_key.as_bytes()).expect("HMAC can take key of any size");
        mac.update(payload);

        // Convert to hex string
        let result = mac.finalize();
        let bytes = result.into_bytes();
        hex::encode(bytes)
    }

    /// Send a batch of events to PostHog.
    async fn send_batch(&self, events: Vec<TelemetryEvent>) {
        if events.is_empty() {
            return;
        }

        let batch_events: Vec<BatchEvent> = events
            .into_iter()
            .map(|mut event| {
                // Merge default properties with event properties
                // Event properties take precedence over defaults
                for (key, value) in &self.default_properties {
                    event.properties.entry(key.clone()).or_insert(value.clone());
                }

                BatchEvent { event: event.event, properties: event.properties, distinct_id: self.distinct_id.clone() }
            })
            .collect();

        let payload = BatchCaptureRequest { api_key: self.config.api_key.clone(), batch: batch_events };

        // Serialize payload to JSON bytes
        let payload_json = match serde_json::to_vec(&payload) {
            Ok(json) => json,
            Err(e) => {
                error!("Failed to serialize telemetry payload: {}", e);
                return;
            }
        };

        // Generate signature
        let signature = self.sign_payload(&payload_json);
        let url = format!("https://{}/batch", self.config.api_host);

        // Send request with signature header
        match self.client.post(&url).header("X-Request-Signature", signature).header("Content-Type", "application/json").body(payload_json).send().await {
            Ok(response) => {
                let status = response.status();
                if status.is_success() {
                    debug!("Telemetry batch sent successfully");
                } else {
                    let body = response.text().await.unwrap_or_default();
                    warn!("PostHog returned status {}: {}", status, body);
                }
            }
            Err(e) => {
                error!("Failed to send telemetry batch: {}", e);
                if let Some(source) = e.source() {
                    error!("Caused by: {}", source);
                }
            }
        }
    }

    /// Get the distinct ID for this client (useful for debugging)
    pub fn distinct_id(&self) -> &str {
        &self.distinct_id
    }

    /// Get the user ID type for this client (useful for debugging)
    pub fn user_id_type(&self) -> Option<&str> {
        self.default_properties.get("user_id_type").and_then(|v| v.as_str())
    }
}

/// Build the default properties that are attached to every event.
fn build_default_properties(id_type: UserIdType) -> HashMap<String, serde_json::Value> {
    use crate::build_info;
    use serde_json::Value;

    let mut props = HashMap::new();

    let platform = if cfg!(target_arch = "wasm32") {
        "browser"
    } else if cfg!(target_os = "windows") {
        "desktop-windows"
    } else if cfg!(target_os = "macos") {
        "desktop-macos"
    } else if cfg!(target_os = "linux") {
        "desktop-linux"
    } else {
        "desktop-unknown"
    };

    props.insert("platform".to_string(), Value::String(platform.to_string()));
    props.insert("build_version".to_string(), Value::String(build_info::VERSION.to_string()));
    props.insert("build_commit".to_string(), Value::String(build_info::git_commit_short().to_string()));
    props.insert("user_id_type".to_string(), Value::String(id_type.as_str().to_string()));

    props
}
