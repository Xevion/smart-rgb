// Shared type definitions for game API
// These mirror the Rust types used in both desktop and WASM builds

export type LeaderboardEntry = {
    id: number;
    name: string;
    color: string; // Hex color without alpha, e.g. "0A44FF"
    tile_count: number;
    troops: number;
    territory_percent: number;
    rank: number;          // Current rank (1-indexed, updates every tick)
    display_order: number; // Visual position (0-indexed, updates every 3rd tick)
};

export type LeaderboardSnapshot = {
    turn: number;
    total_land_tiles: number;
    entries: LeaderboardEntry[];
    local_nation_id: number;
};

export type GameOutcome = "Victory" | "Defeat";

export type AttackEntry = {
    id: number;
    attacker_nation_id: number;
    target_nation_id: number | null; // null for unclaimed territory
    troops: number;
    is_outgoing: boolean;
};

export type AttacksUpdatePayload = {
    turn: number;
    entries: AttackEntry[];
};

export type SpawnCountdown = {
    startedAtMs: number; // Unix epoch milliseconds
    durationSecs: number;
};

export type SpawnPhaseUpdate = {
    countdown: SpawnCountdown | null; // null = waiting, non-null = countdown active
};

export type ShipUpdateVariant =
    | {
          type: "Create";
          id: number;
          owner_nation_id: number;
          path: number[];
          troops: number;
      }
    | {
          type: "Move";
          id: number;
          current_path_index: number;
      }
    | {
          type: "Destroy";
          id: number;
      };

export type ShipsUpdatePayload = {
    turn: number;
    updates: ShipUpdateVariant[];
};

export type UnsubscribeFn = () => void;
