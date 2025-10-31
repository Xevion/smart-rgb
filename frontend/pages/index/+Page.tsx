import { useState, useEffect, Suspense, lazy } from "react";
import { MenuScreen } from "@/shared/components/menu/MenuScreen";
import { useGameBridge } from "@/shared/api";
import "@/shared/styles/global.css";

// Lazy load components that aren't needed immediately
const GameContainer = lazy(() => import("./Game.client").then((m) => ({ default: m.GameContainer })));

function App() {
    const [showMenu, setShowMenu] = useState(true);
    const [isStartingGame, setIsStartingGame] = useState(false);
    const [isGameReady, setIsGameReady] = useState(false);
    const [isHydrated, setIsHydrated] = useState(false);
    const gameBridge = useGameBridge();

    // Prefetch components after initial render
    useEffect(() => {
        setIsHydrated(true);
        import("./Game.client");
    }, []);

    // Track app started on mount
    useEffect(() => {
        if (!gameBridge) return;
        gameBridge.track("app_started", {
            platform: __DESKTOP__ ? "desktop" : "browser",
        });
    }, [gameBridge]);

    const handleStartSingleplayer = () => {
        setIsStartingGame(true);
    };

    const handleGameReady = () => {
        setIsGameReady(true);
    };

    const handleMenuExitComplete = () => {
        setShowMenu(false);
        // Keep isStartingGame and isGameReady - no need to reset them
    };

    const handleReturnToMenu = () => {
        setShowMenu(true);
        setIsStartingGame(false);
        setIsGameReady(false);
    };

    const handleExit = async () => {
        if (__DESKTOP__) {
            const { invoke } = await import("@tauri-apps/api/core");
            await invoke("request_exit").catch((err) => {
                console.error("Failed to request exit:", err);
            });
        }
    };

    return (
        <>
            {/* Menu Screen - pre-renderable, covers everything when visible */}
            {!isHydrated ||
                (showMenu && (
                    <MenuScreen
                        onStartSingleplayer={handleStartSingleplayer}
                        onExit={handleExit}
                        isExiting={isStartingGame}
                        gameReady={isGameReady}
                        onExitComplete={handleMenuExitComplete}
                    />
                ))}

            {/* Game Container - client-only, lazy loaded when game starts */}
            {isHydrated && (isStartingGame || !showMenu) && (
                <Suspense fallback={null}>
                    <GameContainer onReturnToMenu={handleReturnToMenu} onGameReady={handleGameReady} />
                </Suspense>
            )}
        </>
    );
}

export default App;
