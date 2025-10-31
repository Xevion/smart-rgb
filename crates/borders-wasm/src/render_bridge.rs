//! WASM-specific frontend transport using JavaScript callbacks
//!
//! This module provides the WASM implementation of FrontendTransport,
//! sending render messages and UI events through JavaScript callbacks to the browser frontend.

use std::cell::RefCell;
use std::collections::VecDeque;

use borders_core::game::input::InputEvent;
use borders_core::ui::FrontendTransport;
use borders_core::ui::protocol::{BackendMessage, BinaryMessageType, FrontendMessage, encode_binary_message};
use wasm_bindgen::prelude::*;

// Thread-local storage for callbacks and inbound messages
thread_local! {
    static BACKEND_MESSAGE_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    static BINARY_CALLBACK: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    pub(crate) static INBOUND_MESSAGES: RefCell<VecDeque<FrontendMessage>> = const { RefCell::new(VecDeque::new()) };
}

/// Register a callback for backend JSON messages (BackendMessage protocol)
#[wasm_bindgen]
pub fn register_backend_message_callback(callback: js_sys::Function) {
    BACKEND_MESSAGE_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

/// Register a callback for binary data (terrain/territory init and deltas)
/// Callback receives a single Uint8Array with envelope: [type:1][payload:N]
#[wasm_bindgen]
pub fn register_binary_callback(callback: js_sys::Function) {
    BINARY_CALLBACK.with(|cb| {
        *cb.borrow_mut() = Some(callback);
    });
}

/// Unified message handler for all frontend->backend communication
/// Handles FrontendMessage (JSON) payloads
#[wasm_bindgen]
pub fn send_message(msg: JsValue) -> Result<(), JsValue> {
    // Try to deserialize as FrontendMessage
    if let Ok(message) = serde_wasm_bindgen::from_value::<FrontendMessage>(msg) {
        INBOUND_MESSAGES.with(|messages_cell| {
            messages_cell.borrow_mut().push_back(message);
        });
        return Ok(());
    }

    Err(JsValue::from_str("Failed to deserialize message as FrontendMessage"))
}

/// Handle input events from the frontend (separate from FrontendMessage protocol)
/// This mirrors the desktop's Tauri command for architectural consistency
#[wasm_bindgen]
pub fn handle_render_input(event: JsValue) -> Result<(), JsValue> {
    let input_event = serde_wasm_bindgen::from_value::<InputEvent>(event).map_err(|e| JsValue::from_str(&format!("Failed to deserialize InputEvent: {}", e)))?;

    // Send to input queue
    let sender = crate::bridge::get_input_sender().ok_or_else(|| JsValue::from_str("Input sender not initialized"))?;

    sender.send(input_event).map_err(|e| JsValue::from_str(&format!("Failed to send input event: {}", e)))
}

/// WASM-specific frontend transport using JavaScript callbacks
#[derive(Clone)]
pub struct WasmTransport;

impl FrontendTransport for WasmTransport {
    fn send_backend_message(&self, message: &BackendMessage) -> Result<(), String> {
        BACKEND_MESSAGE_CALLBACK.with(|cb_cell| {
            if let Some(cb) = cb_cell.borrow().as_ref() {
                match serde_wasm_bindgen::to_value(message) {
                    Ok(js_payload) => {
                        let this = JsValue::null();
                        if let Err(e) = cb.call1(&this, &js_payload) {
                            return Err(format!("Backend message callback failed: {:?}", e));
                        }
                        Ok(())
                    }
                    Err(e) => Err(format!("Failed to serialize backend message: {}", e)),
                }
            } else {
                Err("No backend message callback registered".to_string())
            }
        })
    }

    fn send_binary(&self, msg_type: BinaryMessageType, payload: Vec<u8>) -> Result<(), String> {
        BINARY_CALLBACK.with(|cb_cell| {
            if let Some(cb) = cb_cell.borrow().as_ref() {
                // Encode with type tag envelope
                let data = encode_binary_message(msg_type, payload);
                let uint8_array = js_sys::Uint8Array::from(&data[..]);
                let this = JsValue::null();
                if let Err(e) = cb.call1(&this, &uint8_array) {
                    return Err(format!("Binary callback failed: {:?}", e));
                }
                Ok(())
            } else {
                Err("No binary callback registered".to_string())
            }
        })
    }

    fn try_recv_frontend_message(&self) -> Option<FrontendMessage> {
        INBOUND_MESSAGES.with(|messages_cell| messages_cell.borrow_mut().pop_front())
    }
}
