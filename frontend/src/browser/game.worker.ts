import init, { register_backend_message_callback, register_binary_callback, send_message, handle_render_input, track_analytics_event, signal_callbacks_ready } from "@wasm/borders";
import { BinaryMessageType } from "@/shared/api/messages";
import { decodeBinaryEnvelope } from "@/shared/utils/binaryDecoding";

/** Analytics event payload */
interface AnalyticsEvent {
    event: string;
    properties: Record<string, unknown>;
}

/** Messages sent from main thread to worker */
type WorkerMessage =
    /** Send FrontendMessage to backend (game commands like StartGame, QuitGame) */
    | { type: "MESSAGE"; payload: unknown }
    /** Send RenderInputEvent to backend (clicks, hovers, keypresses) */
    | { type: "RENDER_INPUT"; payload: unknown }
    /** Track analytics event (separate from game protocol) */
    | { type: "ANALYTICS_EVENT"; payload: AnalyticsEvent };

/**
 * Messages sent from worker to main thread.
 * These notify the transport layer of backend events.
 */
type WorkerResponse =
    /** JSON message from backend (BackendMessage protocol) */
    | { type: "backend:message"; payload: unknown }
    /** Binary initialization data (terrain + territory) */
    | { type: "backend:binary_init"; payload: Uint8Array }
    /** Binary territory delta update */
    | { type: "backend:binary_delta"; payload: Uint8Array }
    /** Worker encountered an error */
    | { type: "ERROR"; payload: { message: string } };

/** Helper to post typed messages back to main thread */
function postResponse(response: WorkerResponse): void {
    self.postMessage(response);
}

/** Buffer for messages received before WASM is ready */
const messageQueue: WorkerMessage[] = [];
let wasmReady = false;

/** Initialize WASM on worker load */
init()
    .then(() => {
        // Register callback for JSON messages from backend (BackendMessage)
        register_backend_message_callback((backendMessage: unknown) => {
            postResponse({ type: "backend:message", payload: backendMessage });
        });

        // Register unified callback for binary data (receives enveloped data)
        register_binary_callback((data: Uint8Array) => {
            // Parse envelope to extract message type and payload
            const decoded = decodeBinaryEnvelope(data);
            if (!decoded) {
                console.error("Worker: Failed to decode binary envelope");
                return;
            }

            // Post typed message to main thread based on envelope type
            if (decoded.type === BinaryMessageType.Init) {
                postResponse({ type: "backend:binary_init", payload: decoded.payload });
            } else {
                postResponse({ type: "backend:binary_delta", payload: decoded.payload });
            }
        });

        // Signal WASM that callbacks are ready (unblocks game initialization)
        signal_callbacks_ready();

        wasmReady = true;
        console.log("WASM module initialized");

        // Process queued messages
        while (messageQueue.length > 0) {
            const message = messageQueue.shift()!;
            processMessage(message);
        }
    })
    .catch((error) => {
        console.error("WASM module initialization failed:", error);
        postResponse({
            type: "ERROR",
            payload: { message: error instanceof Error ? error.message : String(error) },
        });
    });

/** Process a worker message (either from queue or directly) */
function processMessage(message: WorkerMessage): void {
    try {
        switch (message.type) {
            case "MESSAGE":
                send_message(message.payload);
                break;

            case "RENDER_INPUT":
                handle_render_input(message.payload);
                break;

            case "ANALYTICS_EVENT":
                track_analytics_event(message.payload);
                break;

            default: {
                const _exhaustive: never = message;
                console.warn("Unknown worker message type:", (_exhaustive as WorkerMessage).type);
            }
        }
    } catch (error) {
        console.error("Worker error:", error);
        postResponse({
            type: "ERROR",
            payload: { message: error instanceof Error ? error.message : String(error) },
        });
    }
}

/** Handle incoming messages - queue if not ready, process if ready */
self.addEventListener("message", ({ data: message }: MessageEvent<WorkerMessage>) => {
    if (!wasmReady) {
        messageQueue.push(message);
    } else {
        processMessage(message);
    }
});
