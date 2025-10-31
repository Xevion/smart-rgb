// Protocol message types for frontend-backend communication
// These mirror the Rust enums defined in crates/borders-core/src/ui/protocol.rs

import type {
    AttacksUpdatePayload,
    GameOutcome,
    LeaderboardSnapshot,
    ShipsUpdatePayload,
} from "@/shared/api/types";

// Type aliases for glam-compatible vector types (match Rust glam serialization as arrays [x, y])
type Vec2 = [number, number];
type U16Vec2 = [number, number];

// Input event types (mirror Rust types from borders-core)
export type MouseButton = "Left" | "Middle" | "Right" | "Back" | "Forward";
export type ButtonState = "Pressed" | "Released";
export type KeyCode = "KeyW" | "KeyA" | "KeyS" | "KeyD" | "KeyC" | "Digit1" | "Digit2" | "Space" | "Escape";

// Input events sent from frontend to backend
export type InputEvent =
    | {
          MouseButton: {
              button: MouseButton;
              state: ButtonState;
              tile: U16Vec2 | null;
              world_pos: Vec2;
          };
      }
    | {
          MouseMotion: {
              tile: U16Vec2 | null;
              world_pos: Vec2;
          };
      }
    | {
          KeyEvent: {
              key: KeyCode;
              state: ButtonState;
          };
      };

// Raw SpawnCountdown from backend (uses snake_case like Rust)
interface RawSpawnCountdown {
    started_at_ms: number;
    duration_secs: number;
}

// Messages sent from backend to frontend
export type BackendMessage =
    | { msg_type: "LeaderboardSnapshot" } & LeaderboardSnapshot
    | { msg_type: "AttacksUpdate" } & AttacksUpdatePayload
    | { msg_type: "ShipsUpdate" } & ShipsUpdatePayload
    | { msg_type: "GameEnded"; outcome: GameOutcome }
    | { msg_type: "SpawnPhaseUpdate"; countdown: RawSpawnCountdown | null }
    | { msg_type: "SpawnPhaseEnded" }
    | { msg_type: "HighlightNation"; nation_id: number | null };

// Messages sent from frontend to backend
export type FrontendMessage =
    | { msg_type: "StartGame" }
    | { msg_type: "QuitGame" }
    | { msg_type: "SetAttackRatio"; ratio: number };

// Binary message types for unified binary channel
// Mirrors the Rust BinaryMessageType enum in protocol.rs
// IMPORTANT: Constants must match Rust values exactly
export enum BinaryMessageType {
    Init = 0, // Initial game state (terrain + territory + nation palette)
    Delta = 1, // Territory delta (changed tiles only)
}

// Analytics event properties
export type AnalyticsProperties = Record<string, string | number | boolean | null | undefined>;
