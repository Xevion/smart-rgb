import type { Transport, JsonCallback, UnsubscribeFn } from "@/shared/api/Transport";
import type { FrontendMessage, InputEvent, AnalyticsProperties } from "@/shared/api/messages";

/**
 * WASM transport implementation using Web Worker for message passing.
 *
 * Pure transport layer that:
 * - Manages worker lifecycle
 * - Sends/receives JSON messages
 * - Receives binary data (Uint8Array)
 * - Does NOT handle callbacks, rendering, or state management
 *
 * Worker handles initialization and message buffering automatically.
 */
export class WasmTransport implements Transport {
    private worker: Worker;
    private backendMessageCallbacks: Set<JsonCallback> = new Set();
    private binaryCallbacks: Set<(data: Uint8Array, type: "init" | "delta") => void> = new Set();

    constructor() {
        this.worker = new Worker(new URL("./game.worker.ts", import.meta.url), {
            type: "module",
        });

        this.worker.addEventListener("message", (e) => {
            const { type, payload } = e.data;

            switch (type) {
                case "backend:message":
                    this.backendMessageCallbacks.forEach((callback) => callback(payload));
                    break;

                case "backend:binary_init":
                    this.binaryCallbacks.forEach((callback) => callback(payload, "init"));
                    break;

                case "backend:binary_delta":
                    this.binaryCallbacks.forEach((callback) => callback(payload, "delta"));
                    break;

                case "ERROR":
                    console.error("Worker error:", payload);
                    break;

                default:
                    console.warn("Unknown worker message type:", type);
            }
        });
    }

    sendJson(message: FrontendMessage | InputEvent): void {
        this.worker.postMessage({
            type: "MESSAGE",
            payload: message,
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
        this.worker.postMessage({
            type: "RENDER_INPUT",
            payload: event,
        });
    }

    destroy(): void {
        this.worker.terminate();
        this.backendMessageCallbacks.clear();
        this.binaryCallbacks.clear();
    }

    sendAnalytics(event: string, properties: AnalyticsProperties): void {
        this.worker.postMessage({
            type: "ANALYTICS_EVENT",
            payload: {
                event,
                properties,
            },
        });
    }
}
