/**
 * Core transport interface for frontend-backend communication.
 * This is a pure transport layer that only handles message passing,
 * without any higher-level concerns like callbacks, rendering, or state management.
 */

import type { BackendMessage, FrontendMessage, InputEvent, AnalyticsProperties } from "@/shared/api/messages";

export type JsonCallback = (message: BackendMessage) => void;
export type BinaryCallback = (data: Uint8Array) => void;
export type UnsubscribeFn = () => void;

/**
 * Pure transport interface for bidirectional communication.
 * Implementations handle the platform-specific details (WASM worker, Tauri IPC).
 *
 * - sendJson: Send JSON messages to backend
 * - onJson: Receive JSON messages from backend
 * - onBinary: Receive binary data from backend (init or deltas, tagged with type)
 */
export interface Transport {
    /**
     * Send a JSON message to the backend.
     * Message should be a serializable object.
     */
    sendJson(message: FrontendMessage | InputEvent): void;

    /**
     * Subscribe to JSON messages from the backend.
     * Returns an unsubscribe function.
     */
    onJson(callback: JsonCallback): UnsubscribeFn;

    /**
     * Subscribe to binary data from the backend.
     * Callback receives the data and a type tag ("init" or "delta").
     * Returns an unsubscribe function.
     */
    onBinary(callback: (data: Uint8Array, type: "init" | "delta") => void): UnsubscribeFn;

    /**
     * Cleanup resources when transport is no longer needed.
     */
    destroy?(): void;

    /**
     * Send an analytics event to the backend.
     */
    sendAnalytics(event: string, properties: AnalyticsProperties): void;
}
