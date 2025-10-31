import type {
    AttacksUpdatePayload,
    GameOutcome,
    LeaderboardSnapshot,
    ShipsUpdatePayload,
    SpawnPhaseUpdate,
    UnsubscribeFn,
} from "@/shared/api/types";
import type { Transport } from "@/shared/api/Transport";
import type {
    BackendMessage,
    InputEvent,
    MouseButton,
    KeyCode,
    AnalyticsProperties,
} from "@/shared/api/messages";
import type { GameRenderer, PaletteData, TerrainData } from "@/shared/render/GameRenderer";
import { CallbackManager } from "@/shared/utils/CallbackManager";
import { decodeInitBinary } from "@/shared/utils/binaryDecoding";

export interface RenderInitData {
    palette: PaletteData; // Nation palette
    terrain_palette: PaletteData; // Terrain palette
    terrain: TerrainData;
    initial_territories: {
        turn: number;
        territories: { indices: Uint32Array; ownerIds: Uint16Array };
    };
}

interface QueuedRenderMessage {
    _type: "binary";
    data: Uint8Array;
    binaryType: "init" | "delta";
}

/**
 * Game API bridge that connects the UI to the game backend.
 *
 * This class provides:
 * - Callback management per message type
 * - Renderer integration & binary decoding
 * - Message buffering & initialization sequencing
 * - Typed API methods (startGame, onLeaderboardSnapshot, etc.)
 */
export class GameBridge {
    private transport: Transport;
    private renderer: GameRenderer | null = null;
    private snapshotCallbacks = new CallbackManager<LeaderboardSnapshot>();
    private gameEndCallbacks = new CallbackManager<GameOutcome>();
    private attacksUpdateCallbacks = new CallbackManager<AttacksUpdatePayload>();
    private shipsUpdateCallbacks = new CallbackManager<ShipsUpdatePayload>();
    private spawnPhaseUpdateCallbacks = new CallbackManager<SpawnPhaseUpdate>();
    private spawnPhaseEndedCallbacks = new CallbackManager<void>();
    private renderInitCallbacks = new CallbackManager<RenderInitData>();
    private queuedRenderMessages: QueuedRenderMessage[] = [];

    constructor(transport: Transport) {
        this.transport = transport;

        // Subscribe to JSON messages from backend
        this.transport.onJson((message) => this.handleBackendMessage(message));

        // Subscribe to binary data from backend
        this.transport.onBinary((data, type) => this.handleBinaryData(data, type));
    }

    private handleBackendMessage(message: BackendMessage): void {
        // Messages use msg_type tag from serde
        switch (message.msg_type) {
            case "LeaderboardSnapshot":
                this.snapshotCallbacks.notify(message as LeaderboardSnapshot);
                break;

            case "AttacksUpdate":
                this.attacksUpdateCallbacks.notify(message as AttacksUpdatePayload);
                break;

            case "ShipsUpdate":
                this.shipsUpdateCallbacks.notify(message as ShipsUpdatePayload);
                // Also forward to renderer for visual updates
                if (this.renderer) {
                    this.renderer.updateShips(message as ShipsUpdatePayload);
                }
                break;

            case "GameEnded":
                this.gameEndCallbacks.notify(message.outcome as GameOutcome);
                break;

            case "SpawnPhaseUpdate":
                const update: SpawnPhaseUpdate = {
                    countdown: message.countdown
                        ? {
                              startedAtMs: message.countdown.started_at_ms,
                              durationSecs: message.countdown.duration_secs,
                          }
                        : null,
                };
                this.spawnPhaseUpdateCallbacks.notify(update);
                break;

            case "SpawnPhaseEnded":
                this.spawnPhaseEndedCallbacks.notify();
                break;

            case "HighlightNation":
                this.renderer?.setHighlightedNation(message.nation_id ?? null);
                break;

            default:
                // Exhaustiveness check - this should never be reached
                console.warn("Unknown backend message type:", (message as { msg_type: string }).msg_type);
                break;
        }
    }

    private handleBinaryData(data: Uint8Array, type: "init" | "delta"): void {
        if (type === "init") {
            // Decode complete initialization data (terrain + territory + nation palette)
            const decoded = decodeInitBinary(data);
            if (!decoded) {
                console.error("Failed to decode init binary data");
                return;
            }

            // Assemble complete initialization data
            const completeInitData: RenderInitData = {
                palette: {
                    colors: decoded.nationPalette,
                },
                terrain_palette: {
                    colors: decoded.terrain.palette,
                },
                terrain: {
                    size: {
                        x: decoded.terrain.width,
                        y: decoded.terrain.height,
                    },
                    terrain_data: decoded.terrain.tileIds,
                },
                initial_territories: {
                    turn: 0,
                    territories: {
                        indices: decoded.territory.indices,
                        ownerIds: decoded.territory.ownerIds,
                    },
                },
            };

            console.log("Complete initialization data ready:", completeInitData);

            // Notify subscribers
            this.renderInitCallbacks.notify(completeInitData);
            return;
        }

        // For delta updates, need renderer to be ready
        if (!this.renderer) {
            this.queuedRenderMessages.push({ _type: "binary", data, binaryType: type });
            return;
        }

        // Apply delta
        this.renderer.updateBinaryDelta(data);
    }

    startGame(): void {
        this.transport.sendJson({ msg_type: "StartGame" });
    }

    quitGame(): void {
        this.transport.sendJson({ msg_type: "QuitGame" });
    }

    onLeaderboardSnapshot(callback: (data: LeaderboardSnapshot) => void): UnsubscribeFn {
        return this.snapshotCallbacks.subscribe(callback);
    }

    onGameEnded(callback: (outcome: GameOutcome) => void): UnsubscribeFn {
        return this.gameEndCallbacks.subscribe(callback);
    }

    onAttacksUpdate(callback: (data: AttacksUpdatePayload) => void): UnsubscribeFn {
        return this.attacksUpdateCallbacks.subscribe(callback);
    }

    onShipsUpdate(callback: (data: ShipsUpdatePayload) => void): UnsubscribeFn {
        return this.shipsUpdateCallbacks.subscribe(callback);
    }

    onSpawnPhaseUpdate(callback: (update: SpawnPhaseUpdate) => void): UnsubscribeFn {
        return this.spawnPhaseUpdateCallbacks.subscribe(callback);
    }

    onSpawnPhaseEnded(callback: () => void): UnsubscribeFn {
        return this.spawnPhaseEndedCallbacks.subscribe(callback);
    }

    onRenderInit(callback: (data: RenderInitData) => void): UnsubscribeFn {
        return this.renderInitCallbacks.subscribe(callback);
    }

    setRenderer(renderer: GameRenderer): void {
        this.renderer = renderer;

        // Replay any queued binary delta messages that arrived before renderer was ready
        if (this.queuedRenderMessages.length > 0) {
            for (const message of this.queuedRenderMessages) {
                if (message._type === "binary") {
                    this.handleBinaryData(message.data, message.binaryType);
                }
            }
            this.queuedRenderMessages = [];
        }
    }

    sendMapClick(data: { tile: [number, number] | null; world_pos: [number, number]; button: number }): void {
        // Map button number to MouseButton enum
        const buttonMap: { [key: number]: MouseButton } = {
            0: "Left",
            1: "Middle",
            2: "Right",
            3: "Back",
            4: "Forward",
        };
        const button = buttonMap[data.button] || "Left";

        const event: InputEvent = {
            MouseButton: {
                button,
                state: "Released", // Map clicks are releases
                tile: data.tile,
                world_pos: data.world_pos,
            },
        };
        // Transport implementations may have specialized methods for render input
        if ("sendRenderInput" in this.transport) {
            (this.transport as { sendRenderInput: (event: InputEvent) => void }).sendRenderInput(event);
        }
    }

    sendMapHover(data: { tile: [number, number] | null; world_pos: [number, number] }): void {
        const event: InputEvent = {
            MouseMotion: {
                tile: data.tile,
                world_pos: data.world_pos,
            },
        };
        if ("sendRenderInput" in this.transport) {
            (this.transport as { sendRenderInput: (event: InputEvent) => void }).sendRenderInput(event);
        }
    }

    sendKeyPress(data: { key: string; pressed: boolean }): void {
        // Map string to KeyCode enum (ignore unknown keys)
        const validKeys: { [key: string]: KeyCode } = {
            KeyW: "KeyW",
            KeyA: "KeyA",
            KeyS: "KeyS",
            KeyD: "KeyD",
            KeyC: "KeyC",
            Digit1: "Digit1",
            Digit2: "Digit2",
            Space: "Space",
            Escape: "Escape",
        };

        const keyCode = validKeys[data.key];
        if (!keyCode) {
            // Ignore unknown keys (backend will skip them)
            return;
        }

        const event: InputEvent = {
            KeyEvent: {
                key: keyCode,
                state: data.pressed ? "Pressed" : "Released",
            },
        };
        if ("sendRenderInput" in this.transport) {
            (this.transport as { sendRenderInput: (event: InputEvent) => void }).sendRenderInput(event);
        }
    }

    sendAttackRatio(ratio: number): void {
        this.transport.sendJson({
            msg_type: "SetAttackRatio",
            ratio: ratio,
        });
    }

    async getGameState(): Promise<unknown> {
        // WASM doesn't persist state across reloads - always start fresh
        // Tauri could implement this if needed
        return null;
    }

    track(event: string, properties?: AnalyticsProperties): void {
        this.transport.sendAnalytics(event, properties || {});
    }
}
