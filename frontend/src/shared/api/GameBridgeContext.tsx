import { createContext, useContext, type ReactNode } from "react";
import type { GameBridge } from "@/shared/api/GameBridge";

const GameBridgeContext = createContext<GameBridge | null>(null);

export interface GameBridgeProviderProps {
    bridge: GameBridge;
    children: ReactNode;
}

export function GameBridgeProvider({ bridge, children }: GameBridgeProviderProps) {
    return <GameBridgeContext.Provider value={bridge}>{children}</GameBridgeContext.Provider>;
}

export function useGameBridge(): GameBridge | null {
    return useContext(GameBridgeContext);
}
