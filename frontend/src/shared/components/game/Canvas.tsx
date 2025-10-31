import React, { useEffect, useRef, useState } from "react";
import { GameRenderer } from "@/shared/render";
import { useGameBridge, type RenderInitData } from "@/shared/api";
import { useThrottledCallback } from "@/shared/hooks";

interface GameCanvasProps {
    className?: string;
    initialState?: RenderInitData | null;
    onRendererReady?: (renderer: GameRenderer | null) => void;
    onNationHover?: (nationId: number | null) => void;
}

export const GameCanvas: React.FC<GameCanvasProps> = ({ className, initialState, onRendererReady, onNationHover }) => {
    const canvasRef = useRef<HTMLCanvasElement>(null);
    const rendererRef = useRef<GameRenderer | null>(null);
    const [isInitialized, setIsInitialized] = useState(false);
    const lastHoveredNationRef = useRef<number | null>(null);
    const lastHoveredTileRef = useRef<number | null>(null);
    const gameBridge = useGameBridge();

    // Initialize renderer once initial state is available
    useEffect(() => {
        if (!canvasRef.current || !initialState || rendererRef.current) return;

        let cancelled = false;

        // Create renderer with all required data
        GameRenderer.create({
            canvas: canvasRef.current,
            terrainPalette: initialState.terrain_palette,
            terrain: initialState.terrain,
            nationPalette: initialState.palette,
            initialTerritories: {
                turn: initialState.initial_territories.turn,
                territories: initialState.initial_territories.territories,
            },
        })
            .then(async (renderer) => {
                if (cancelled) {
                    renderer.destroy();
                    return;
                }

                rendererRef.current = renderer;
                setIsInitialized(true);

                // Register renderer with API for receiving updates
                if (gameBridge && typeof gameBridge.setRenderer === "function") {
                    await gameBridge.setRenderer(renderer);
                }

                // Notify parent that renderer is ready
                onRendererReady?.(renderer);
            })
            .catch((err: unknown) => {
                if (!cancelled) {
                    console.error("Failed to initialize GameRenderer:", err);
                }
            });

        // Cleanup
        return () => {
            cancelled = true;
            if (rendererRef.current) {
                rendererRef.current.destroy();
                rendererRef.current = null;
                onRendererReady?.(null);
            }
            setIsInitialized(false);
        };
    }, [initialState, gameBridge, onRendererReady]);

    // Handle canvas clicks using native event listener (after renderer initialization)
    // React onClick doesn't work reliably when native listeners are attached to canvas
    useEffect(() => {
        const canvas = canvasRef.current;
        if (!canvas || !rendererRef.current || !gameBridge) return;

        const handleCanvasClick = (e: MouseEvent) => {
            if (!rendererRef.current || !gameBridge) return;

            // Ignore clicks that were camera drags
            if (rendererRef.current.hadCameraInteraction()) {
                return;
            }

            const rect = canvas.getBoundingClientRect();
            const screenX = e.clientX - rect.left;
            const screenY = e.clientY - rect.top;

            if (typeof gameBridge.sendMapClick === "function") {
                // Convert screen → world once, then world → tile coords (avoid wasteful round-trip)
                const worldPos = rendererRef.current.screenToWorld(screenX, screenY);
                const tileCoords = rendererRef.current.coordinateMapper.worldToTileCoords(worldPos.x, worldPos.y);

                gameBridge.sendMapClick({
                    tile: tileCoords ? [tileCoords.x, tileCoords.y] : null,
                    world_pos: [worldPos.x, worldPos.y],
                    button: e.button,
                });
            }
        };

        canvas.addEventListener("click", handleCanvasClick);
        return () => canvas.removeEventListener("click", handleCanvasClick);
    }, [gameBridge, isInitialized]);

    // Handle mouse move (for hover) - throttled to reduce IPC overhead
    const handleMouseMove = useThrottledCallback((e: React.MouseEvent<HTMLCanvasElement>) => {
        if (!rendererRef.current) return;

        const rect = canvasRef.current!.getBoundingClientRect();
        const screenX = e.clientX - rect.left;
        const screenY = e.clientY - rect.top;

        // Convert screen → world once, then world → tile coords (avoid wasteful round-trip)
        const worldPos = rendererRef.current.screenToWorld(screenX, screenY);
        const tileCoords = rendererRef.current.coordinateMapper.worldToTileCoords(worldPos.x, worldPos.y);
        if (tileCoords === null) return;

        const tileIndex = rendererRef.current.coordinateMapper.coordsToTileIndex(tileCoords.x, tileCoords.y);

        // Handle nation hover detection
        if (onNationHover) {
            const nationId = rendererRef.current.getNationAtTile(tileIndex);
            if (nationId !== lastHoveredNationRef.current) {
                lastHoveredNationRef.current = nationId;
                onNationHover(nationId);
            }
        }

        // Only send map hover if tile changed
        if (tileIndex !== lastHoveredTileRef.current) {
            lastHoveredTileRef.current = tileIndex;

            if (gameBridge && typeof gameBridge.sendMapHover === "function") {
                // We already have worldPos and tileCoords from above
                gameBridge.sendMapHover({
                    tile: [tileCoords.x, tileCoords.y],
                    world_pos: [worldPos.x, worldPos.y],
                });
            }
        }
    }, 10);

    // Handle keyboard input
    useEffect(() => {
        if (!gameBridge) return;

        const handleKeyDown = (e: KeyboardEvent) => {
            if (typeof gameBridge.sendKeyPress === "function") {
                gameBridge.sendKeyPress({
                    key: e.code,
                    pressed: true,
                });
            }
        };

        const handleKeyUp = (e: KeyboardEvent) => {
            if (typeof gameBridge.sendKeyPress === "function") {
                gameBridge.sendKeyPress({
                    key: e.code,
                    pressed: false,
                });
            }
        };

        window.addEventListener("keydown", handleKeyDown);
        window.addEventListener("keyup", handleKeyUp);

        return () => {
            window.removeEventListener("keydown", handleKeyDown);
            window.removeEventListener("keyup", handleKeyUp);
        };
    }, [gameBridge]);

    return (
        <div className={className}>
            <canvas
                ref={canvasRef}
                onMouseMove={handleMouseMove}
                onContextMenu={(e) => e.preventDefault()}
                style={{
                    display: "block",
                    width: "100%",
                    height: "100%",
                    cursor: isInitialized ? undefined : "wait",
                }}
            />
        </div>
    );
};
