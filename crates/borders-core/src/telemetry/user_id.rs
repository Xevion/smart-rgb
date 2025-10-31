use tracing::debug;
use uuid::Uuid;

#[cfg(not(target_arch = "wasm32"))]
use tracing::warn;

/// Type of user ID that was generated or loaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserIdType {
    /// ID was loaded from storage (existing user)
    Existing,
    /// ID was generated from hardware components
    Hardware,
    /// ID was newly generated random UUID
    New,
}

impl UserIdType {
    pub fn as_str(&self) -> &'static str {
        match self {
            UserIdType::Existing => "existing",
            UserIdType::Hardware => "hardware",
            UserIdType::New => "new",
        }
    }
}

/// Get or create a persistent user ID (sync version for native platforms).
///
/// This function attempts to identify the user through multiple strategies:
/// 1. Stored UUID (persisted across runs, most reliable)
/// 2. Hardware-based ID (hashed for privacy, then stored for future use)
/// 3. Generate new UUID (if nothing exists)
///
/// Returns a tuple of (user_id, id_type).
#[cfg(not(target_arch = "wasm32"))]
pub fn get_or_create_user_id() -> (String, UserIdType) {
    // Try to load stored ID first (most reliable)
    if let Some(stored_id) = load_stored_id() {
        debug!("Using stored user ID");
        return (stored_id, UserIdType::Existing);
    }

    // Try hardware-based ID
    if let Some(hw_id) = get_hardware_id() {
        debug!("Generated hardware-based user ID");
        // Store it for future reliability
        if let Err(e) = store_user_id(&hw_id) {
            warn!("Failed to store hardware-based user ID: {}", e);
        }
        return (hw_id, UserIdType::Hardware);
    }

    // Generate and store new ID
    let new_id = Uuid::new_v4().to_string();
    debug!("Generated new user ID");

    if let Err(e) = store_user_id(&new_id) {
        warn!("Failed to store new user ID: {}", e);
    }

    (new_id, UserIdType::New)
}

/// Get or create a persistent user ID (async version for WASM).
///
/// This function attempts to identify the user through multiple strategies:
/// 1. Stored UUID in localStorage (via main thread, persisted across runs)
/// 2. Generate new UUID (if nothing exists)
///
/// Returns a tuple of (user_id, id_type).
#[cfg(target_arch = "wasm32")]
pub async fn get_or_create_user_id_async() -> (String, UserIdType) {
    // Try to load from localStorage via main thread
    if let Some(stored_id) = load_from_localstorage().await {
        debug!("Loaded user ID from localStorage");
        return (stored_id, UserIdType::Existing);
    }

    // Generate and store new ID
    let new_id = Uuid::new_v4().to_string();
    debug!("Generated new user ID");

    // Try to store it (fire and forget)
    store_user_id(&new_id).ok();

    (new_id, UserIdType::New)
}

/// Attempt to get a hardware-based identifier.
///
/// Uses machineid-rs to build a stable ID from hardware components.
/// The ID is hashed with SHA256 for privacy.
///
/// Only available on native platforms (not WASM).
#[cfg(not(target_arch = "wasm32"))]
fn get_hardware_id() -> Option<String> {
    use machineid_rs::{Encryption, HWIDComponent, IdBuilder};

    match IdBuilder::new(Encryption::SHA256).add_component(HWIDComponent::SystemID).add_component(HWIDComponent::CPUCores).build("iron-borders") {
        Ok(id) => {
            debug!("Successfully generated hardware ID");
            Some(id)
        }
        Err(e) => {
            warn!("Failed to generate hardware ID: {}", e);
            None
        }
    }
}

/// Hardware IDs are not available on WASM.
#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
fn get_hardware_id() -> Option<String> {
    None
}

/// Load a previously stored user ID from platform-specific storage.
#[cfg(not(target_arch = "wasm32"))]
fn load_stored_id() -> Option<String> {
    #[cfg(windows)]
    {
        load_from_registry()
    }

    #[cfg(not(windows))]
    {
        load_from_file()
    }
}

/// Store a user ID to platform-specific storage.
#[cfg(not(target_arch = "wasm32"))]
fn store_user_id(id: &str) -> Result<(), String> {
    #[cfg(windows)]
    {
        store_to_registry(id)
    }

    #[cfg(not(windows))]
    {
        store_to_file(id)
    }
}

#[cfg(target_arch = "wasm32")]
fn store_user_id(id: &str) -> Result<(), String> {
    use wasm_bindgen::JsValue;
    use web_sys::BroadcastChannel;

    let channel = BroadcastChannel::new("user_id_storage").ok().ok_or("Failed to create channel")?;
    let msg = format!(r#"{{"action":"save","id":"{}"}}"#, id);
    channel.post_message(&JsValue::from_str(&msg)).ok().ok_or("Failed to post")?;
    Ok(())
}

#[cfg(windows)]
fn load_from_registry() -> Option<String> {
    use winreg::RegKey;
    use winreg::enums::*;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    match hkcu.open_subkey("Software\\Iron Borders\\ClientCache") {
        Ok(key) => match key.get_value::<String, _>("sid") {
            Ok(id) => {
                debug!("Loaded user ID from registry");
                Some(id)
            }
            Err(_) => None,
        },
        Err(_) => None,
    }
}

#[cfg(windows)]
fn store_to_registry(id: &str) -> Result<(), String> {
    use winreg::RegKey;
    use winreg::enums::*;

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let (key, _) = hkcu.create_subkey("Software\\Iron Borders\\ClientCache").map_err(|e| format!("Failed to create registry key: {}", e))?;

    key.set_value("sid", &id).map_err(|e| format!("Failed to set registry value: {}", e))?;

    debug!("Stored user ID to registry");
    Ok(())
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn load_from_file() -> Option<String> {
    use directories::ProjectDirs;
    use std::fs;

    let proj_dirs = ProjectDirs::from("", "", "iron-borders")?;
    let data_dir = proj_dirs.data_dir();
    let file_path = data_dir.join("client.dat");

    match fs::read_to_string(&file_path) {
        Ok(id) => {
            debug!("Loaded user ID from file: {:?}", file_path);
            Some(id.trim().to_string())
        }
        Err(_) => None,
    }
}

#[cfg(all(not(target_arch = "wasm32"), not(windows)))]
fn store_to_file(id: &str) -> Result<(), String> {
    use directories::ProjectDirs;
    use std::fs;

    let proj_dirs = ProjectDirs::from("", "", "iron-borders").ok_or("Failed to get project directories")?;

    let data_dir = proj_dirs.data_dir();

    // Create directory if it doesn't exist
    fs::create_dir_all(data_dir).map_err(|e| format!("Failed to create data directory: {}", e))?;

    let file_path = data_dir.join("client.dat");

    fs::write(&file_path, id).map_err(|e| format!("Failed to write user ID file: {}", e))?;

    debug!("Stored user ID to file: {:?}", file_path);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
async fn load_from_localstorage() -> Option<String> {
    use gloo_timers::future::TimeoutFuture;
    use std::sync::Arc;
    use std::sync::Mutex;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::prelude::*;
    use web_sys::{BroadcastChannel, MessageEvent};

    let channel = BroadcastChannel::new("user_id_storage").ok()?;
    let result = Arc::new(Mutex::new(None));
    let result_clone = result.clone();

    let callback = Closure::wrap(Box::new(move |event: MessageEvent| {
        if let Some(data) = event.data().as_string()
            && let Ok(parsed) = js_sys::JSON::parse(&data)
            && let Some(obj) = parsed.dyn_ref::<js_sys::Object>()
            && let Ok(action) = js_sys::Reflect::get(obj, &JsValue::from_str("action"))
            && action.as_string().as_deref() == Some("load_response")
            && let Ok(id_val) = js_sys::Reflect::get(obj, &JsValue::from_str("id"))
            && let Some(id) = id_val.as_string()
        {
            *result_clone.lock().unwrap() = Some(id);
        }
    }) as Box<dyn FnMut(_)>);

    channel.set_onmessage(Some(callback.as_ref().unchecked_ref()));

    // Send load request
    let msg = r#"{"action":"load"}"#;
    channel.post_message(&JsValue::from_str(msg)).ok()?;

    // Wait up to 100ms for response
    TimeoutFuture::new(100).await;

    callback.forget();

    result.lock().unwrap().clone()
}
