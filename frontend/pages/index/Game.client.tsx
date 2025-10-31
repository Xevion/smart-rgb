import { useState, useEffect } from "react";
import { Attacks } from "@/shared/components/game/AttacksList";
import { AttackControls } from "@/shared/components/game/AttackControls";
import { Leaderboard } from "@/shared/components/game/Leaderboard";
import { GameMenu } from "@/shared/components/game/Menu";
import { SpawnPhaseOverlay } from "@/shared/components/overlays/SpawnPhase";
import { GameCanvas } from "@/shared/components/game/Canvas";
import { GameEndOverlay } from "@/shared/components/overlays/GameEnd";
import type { GameOutcome, LeaderboardSnapshot } from "@/shared/api/types";
import type { GameRenderer } from "@/shared/render/GameRenderer";
import { useGameBridge, type RenderInitData } from "@/shared/api";

interface GameContainerProps {
    onReturnToMenu: () => void;
    onGameReady?: () => void;
}

export function GameContainer({ onReturnToMenu, onGameReady }: GameContainerProps) {
    const [gameOutcome, setGameOutcome] = useState<GameOutcome | null>(null);
    const [spawnPhaseActive, setSpawnPhaseActive] = useState(false);
    const [spawnCountdown, setSpawnCountdown] = useState<{
        startedAtMs: number;
        durationSecs: number;
    } | null>(null);
    const [initialGameState, setInitialGameState] = useState<RenderInitData | null>(null);
    const [initialLeaderboard, setInitialLeaderboard] = useState<LeaderboardSnapshot | null>(null);
    const [renderer, setRenderer] = useState<GameRenderer | null>(null);
    const [highlightedNation, setHighlightedNation] = useState<number | null>(null);
    const gameBridge = useGameBridge();

    // Check for existing game state on mount to recover after reload
    useEffect(() => {
        // Only check for state recovery on desktop
        if (!__DESKTOP__) return;

        gameBridge?.getGameState().then((state) => {
            // State recovery is limited to leaderboard data only
            // Initialization data (terrain, territory, nation palette) is not recoverable after reload
            // The frontend must wait for a fresh game to start
            const leaderboard = state as LeaderboardSnapshot | null;
            if (leaderboard) {
                console.log("Recovered leaderboard state after reload:", leaderboard);
                setInitialLeaderboard(leaderboard);
            }
        });
    }, [gameBridge]);

    // Subscribe to render initialization
    useEffect(() => {
        if (!gameBridge) return;

        const unsubscribe = gameBridge.onRenderInit((renderData) => {
            console.log("RenderInit received:", renderData);
            setInitialGameState(renderData);
            onGameReady?.();
        });

        return () => unsubscribe();
    }, [gameBridge, onGameReady]);

    // Start the game on mount
    useEffect(() => {
        if (!gameBridge) return;

        gameBridge.startGame();
        gameBridge.track("game_started", {
            mode: "singleplayer",
        });
    }, [gameBridge]);

    // Subscribe to spawn phase events
    useEffect(() => {
        if (!gameBridge) return;

        const unsubUpdate = gameBridge.onSpawnPhaseUpdate((update) => {
            setSpawnPhaseActive(true);
            setSpawnCountdown(update.countdown);
        });

        const unsubEnd = gameBridge.onSpawnPhaseEnded(() => {
            setSpawnPhaseActive(false);
            setSpawnCountdown(null);
        });

        return () => {
            unsubUpdate();
            unsubEnd();
        };
    }, [gameBridge]);

    // Subscribe to game end events
    useEffect(() => {
        if (!gameBridge) return;

        const unsubscribe = gameBridge.onGameEnded((outcome) => {
            console.log("Game outcome received:", outcome);
            setGameOutcome(outcome);
            setSpawnPhaseActive(false);
            gameBridge.track("game_ended", {
                outcome: outcome.toString().toLowerCase(),
            });
        });

        return () => unsubscribe();
    }, [gameBridge]);

    useEffect(() => {
        if (!renderer) return;
        if (!gameBridge) return;

        // Track renderer initialization with GPU info
        const rendererInfo = renderer.getRendererInfo();
        gameBridge.track("renderer_initialized", rendererInfo);
    }, [renderer, gameBridge]);

    // Sync highlighted nation with renderer
    useEffect(() => {
        renderer?.setHighlightedNation(highlightedNation);
    }, [highlightedNation, renderer]);

    const handleExit = () => {
        gameBridge?.quitGame();
        setGameOutcome(null);
        onReturnToMenu();
    };

    return (
        <>
            <GameCanvas
                className="fixed top-0 left-0 w-screen h-screen pointer-events-auto"
                initialState={initialGameState}
                onRendererReady={setRenderer}
                onNationHover={setHighlightedNation}
            />

            {initialGameState && (
                <div className="fixed top-0 left-0 w-full h-full pointer-events-none flex flex-col">
                    <SpawnPhaseOverlay isVisible={spawnPhaseActive} countdown={spawnCountdown} />

                    <div className="relative flex-1">
                        {spawnPhaseActive && (
                            <div className="absolute top-16 left-1/2 -translate-x-1/2 text-center select-none w-full px-4 z-50">
                                <h1 className="font-oswald text-5xl leading-4 font-bold text-white drop-shadow-lg tracking-wide">
                                    Pick Your Spawn
                                </h1>
                                <p className="font-sans font-medium mx-auto mt-6 text-lg text-white drop-shadow-md max-w-xl leading-relaxed">
                                    Click anywhere on the map to place your starting territory.
                                    <br />
                                    You can change your mind before the timer expires.
                                </p>
                            </div>
                        )}

                        <Leaderboard
                            initialSnapshot={initialLeaderboard}
                            highlightedNation={highlightedNation}
                            onNationHover={setHighlightedNation}
                        />
                        <AttackControls>
                            <Attacks onNationHover={setHighlightedNation} />
                        </AttackControls>
                        <GameMenu
                            onExit={handleExit}
                            onSettings={() => {
                                // TODO: Implement settings
                            }}
                        />

                        {gameOutcome && (
                            <GameEndOverlay outcome={gameOutcome} onSpectate={() => setGameOutcome(null)} onExit={handleExit} />
                        )}
                    </div>
                </div>
            )}
        </>
    );
}
