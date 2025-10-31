import type { GameBridge } from "@/shared/api/GameBridge";

/**
 * Get the platform-specific GameBridge implementation.
 */
export async function getBridge(): Promise<GameBridge> {
    const platform = await (__DESKTOP__ ? import("@/desktop") : import("@/browser"));
    return platform.bridge;
}

// Re-export all API types and interfaces for convenient imports
export * from "@/shared/api/types";
export * from "@/shared/api/GameBridge";
export * from "@/shared/api/GameBridgeContext";
