import { listen } from "@tauri-apps/api/event";
import { invoke, Channel } from "@tauri-apps/api/core";
import type { Transport, JsonCallback, UnsubscribeFn } from "@/shared/api/Transport";
import type { BackendMessage, FrontendMessage, InputEvent, AnalyticsProperties } from "@/shared/api/messages";
import { BinaryMessageType } from "@/shared/api/messages";
import { decodeBinaryEnvelope } from "@/shared/utils/binaryDecoding";

/**
 * Tauri transport implementation using Tauri channels for all binary data.
 *
 * Pure transport layer that:
 * - Listens to Tauri events for JSON backend messages
 * - Uses unified channel for all binary data (init + deltas)
 * - Uses invoke for sending messages to backend
 * - Does NOT handle callbacks, rendering, or state management
 */
export class TauriTransport implements Transport {
    private backendMessageCallbacks: Set<JsonCallback> = new Set();
    private binaryCallbacks: Set<(data: Uint8Array, type: "init" | "delta") => void> = new Set();
    private backendMessageUnsubscribe?: () => void;

    constructor() {
        this.setupEventListeners();
        this.setupBinaryChannel();
    }

    private async setupEventListeners() {
        // Listen for JSON messages from backend
        this.backendMessageUnsubscribe = await listen<BackendMessage>("backend:message", (event) => {
            this.backendMessageCallbacks.forEach((callback) => callback(event.payload));
        });
    }

    private async setupBinaryChannel() {
        const channel = new Channel<number[]>();
        channel.onmessage = (data) => {
            const binary = new Uint8Array(data);

            // Parse envelope to extract message type and payload
            const decoded = decodeBinaryEnvelope(binary);
            if (!decoded) {
                console.error("Failed to decode binary envelope");
                return;
            }

            // Dispatch to callbacks with type discrimination
            const type = decoded.type === BinaryMessageType.Init ? "init" : "delta";
            this.binaryCallbacks.forEach((callback) => callback(decoded.payload, type));
        };

        try {
            await invoke("register_binary_channel", { channel });
            console.log("Binary channel registered successfully (handles init + deltas)");
        } catch (err) {
            console.error("Failed to register binary channel:", err);
        }
    }

    sendJson(message: FrontendMessage | InputEvent): void {
        invoke("send_frontend_message", { message }).catch((err) => {
            console.error("Failed to send frontend message:", err);
        });
    }

    onJson(callback: JsonCallback): UnsubscribeFn {
        this.backendMessageCallbacks.add(callback);
        return () => {
            this.backendMessageCallbacks.delete(callback);
        };
    }

    onBinary(callback: (data: Uint8Array, type: "init" | "delta") => void): UnsubscribeFn {
        this.binaryCallbacks.add(callback);
        return () => {
            this.binaryCallbacks.delete(callback);
        };
    }

    sendRenderInput(event: InputEvent): void {
        invoke("handle_render_input", { event }).catch((err) => {
            console.error("Failed to send render input:", err);
        });
    }

    sendAnalytics(event: string, properties: AnalyticsProperties): void {
        invoke("track_analytics_event", {
            payload: {
                event,
                properties,
            },
        }).catch((err) => {
            console.error("Failed to track analytics event:", err);
        });
    }

    destroy(): void {
        if (this.backendMessageUnsubscribe) {
            this.backendMessageUnsubscribe();
            this.backendMessageUnsubscribe = undefined;
        }

        this.backendMessageCallbacks.clear();
        this.binaryCallbacks.clear();
    }
}
