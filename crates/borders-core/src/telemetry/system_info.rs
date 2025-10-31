//! System information collection for analytics.
//!
//! Collects platform-specific system information for telemetry purposes.

use serde_json::Value;
use std::collections::HashMap;

/// Detailed system information collected once at session start.
#[derive(Debug, Clone)]
pub struct SystemInfo {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub cpu_brand: Option<String>,
    pub cpu_cores: Option<usize>,
    pub total_memory_mb: Option<u64>,
}

impl SystemInfo {
    /// Collect system information for the current platform.
    pub fn collect() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            Self::collect_native()
        }

        #[cfg(target_arch = "wasm32")]
        {
            Self::collect_wasm()
        }
    }

    /// Convert system info to a HashMap for inclusion in telemetry events.
    pub fn to_properties(&self) -> HashMap<String, Value> {
        let mut props = HashMap::new();
        props.insert("os_name".to_string(), Value::String(self.os_name.clone()));
        props.insert("os_version".to_string(), Value::String(self.os_version.clone()));
        props.insert("arch".to_string(), Value::String(self.arch.clone()));

        if let Some(brand) = &self.cpu_brand {
            props.insert("cpu_brand".to_string(), Value::String(brand.clone()));
        }
        if let Some(cores) = self.cpu_cores {
            props.insert("cpu_cores".to_string(), Value::Number(cores.into()));
        }
        if let Some(mem) = self.total_memory_mb {
            props.insert("total_memory_mb".to_string(), Value::Number(mem.into()));
        }

        props
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn collect_native() -> Self {
        use sysinfo::System;

        let mut sys = System::new_all();
        sys.refresh_all();

        let os_name = System::name().unwrap_or_else(|| "Unknown".to_string());
        let os_version = System::os_version().unwrap_or_else(|| "Unknown".to_string());
        let arch = std::env::consts::ARCH.to_string();

        let cpu_brand = sys.cpus().first().map(|cpu| cpu.brand().to_string());
        let cpu_cores = sys.cpus().len();
        let total_memory_mb = sys.total_memory() / 1024 / 1024;

        Self { os_name, os_version, arch, cpu_brand, cpu_cores: Some(cpu_cores), total_memory_mb: Some(total_memory_mb) }
    }

    #[cfg(target_arch = "wasm32")]
    fn collect_wasm() -> Self {
        use wasm_bindgen::JsValue;

        // In web workers, use the global scope instead of window
        let global = js_sys::global();
        let navigator = js_sys::Reflect::get(&global, &JsValue::from_str("navigator")).expect("navigator should be available");

        // Call methods using Reflect to work with both Navigator and WorkerNavigator
        let user_agent = js_sys::Reflect::get(&navigator, &JsValue::from_str("userAgent")).ok().and_then(|v| v.as_string()).unwrap_or_default();
        let platform = js_sys::Reflect::get(&navigator, &JsValue::from_str("platform")).ok().and_then(|v| v.as_string()).unwrap_or_default();

        let (os_name, os_version) = parse_user_agent(&user_agent);
        let arch = platform;

        let cpu_cores = js_sys::Reflect::get(&navigator, &JsValue::from_str("hardwareConcurrency")).ok().and_then(|v| v.as_f64()).and_then(|f| if f > 0.0 { Some(f as usize) } else { None });

        let device_memory = js_sys::Reflect::get(&navigator, &JsValue::from_str("deviceMemory")).ok().and_then(|v| v.as_f64()).map(|gb| (gb * 1024.0) as u64);

        Self { os_name, os_version, arch, cpu_brand: None, cpu_cores, total_memory_mb: device_memory }
    }
}

/// Parse user agent string to extract OS name and version.
#[cfg(target_arch = "wasm32")]
fn parse_user_agent(ua: &str) -> (String, String) {
    if ua.contains("Windows NT 10.0") {
        ("Windows".to_string(), "10/11".to_string())
    } else if ua.contains("Windows NT 6.3") {
        ("Windows".to_string(), "8.1".to_string())
    } else if ua.contains("Windows NT 6.2") {
        ("Windows".to_string(), "8".to_string())
    } else if ua.contains("Windows NT 6.1") {
        ("Windows".to_string(), "7".to_string())
    } else if ua.contains("Mac OS X") {
        let version = ua.split("Mac OS X ").nth(1).and_then(|s| s.split(')').next()).unwrap_or("Unknown");
        ("macOS".to_string(), version.replace('_', "."))
    } else if ua.contains("Android") {
        let version = ua.split("Android ").nth(1).and_then(|s| s.split(';').next()).unwrap_or("Unknown");
        ("Android".to_string(), version.to_string())
    } else if ua.contains("Linux") {
        ("Linux".to_string(), "Unknown".to_string())
    } else if ua.contains("iOS") || ua.contains("iPhone") || ua.contains("iPad") {
        let version = ua.split("OS ").nth(1).and_then(|s| s.split(' ').next()).unwrap_or("Unknown");
        ("iOS".to_string(), version.replace('_', "."))
    } else {
        ("Unknown".to_string(), "Unknown".to_string())
    }
}

/// Get browser name and version from user agent.
#[cfg(target_arch = "wasm32")]
pub fn get_browser_info() -> (String, String) {
    use wasm_bindgen::JsValue;

    // In web workers, use the global scope instead of window
    let global = js_sys::global();
    let navigator = js_sys::Reflect::get(&global, &JsValue::from_str("navigator")).expect("navigator should be available");

    // Call methods using Reflect to work with both Navigator and WorkerNavigator
    let ua = js_sys::Reflect::get(&navigator, &JsValue::from_str("userAgent")).ok().and_then(|v| v.as_string()).unwrap_or_default();

    if ua.contains("Edg/") {
        let version = ua.split("Edg/").nth(1).and_then(|s| s.split(' ').next()).unwrap_or("Unknown");
        ("Edge".to_string(), version.to_string())
    } else if ua.contains("Chrome/") {
        let version = ua.split("Chrome/").nth(1).and_then(|s| s.split(' ').next()).unwrap_or("Unknown");
        ("Chrome".to_string(), version.to_string())
    } else if ua.contains("Firefox/") {
        let version = ua.split("Firefox/").nth(1).and_then(|s| s.split(' ').next()).unwrap_or("Unknown");
        ("Firefox".to_string(), version.to_string())
    } else if ua.contains("Safari/") && !ua.contains("Chrome") {
        let version = ua.split("Version/").nth(1).and_then(|s| s.split(' ').next()).unwrap_or("Unknown");
        ("Safari".to_string(), version.to_string())
    } else {
        ("Unknown".to_string(), "Unknown".to_string())
    }
}
