import { useCallback, useEffect, useRef, useState, ReactNode } from "react";
import { useGameBridge } from "@/shared/api";
import type { LeaderboardSnapshot } from "@/shared/api/types";
import { formatTroopCount } from "@/shared/utils/formatting";

const MIN_PERCENTAGE = 1;
const MAX_PERCENTAGE = 100;
const DEFAULT_PERCENTAGE = 50;
const WHEEL_DELTA = 5;
const MIN_STEP_PERCENTAGE = 5; // Jump to 5% from 1% to avoid tiny increments at the low end

interface AttackControlsProps {
    children?: ReactNode;
}

export function AttackControls({ children }: AttackControlsProps) {
    const gameBridge = useGameBridge();
    const [percentage, setPercentage] = useState(DEFAULT_PERCENTAGE);
    const [playerTroops, setPlayerTroops] = useState<number | null>(null);
    const containerRef = useRef<HTMLDivElement>(null);
    const sliderRef = useRef<HTMLDivElement>(null);
    const isDraggingRef = useRef(false);
    const isHoveringRef = useRef(false);
    // Track last sent value to avoid redundant network calls when rounding yields same percentage
    const lastSentPercentageRef = useRef<number | null>(null);

    const sendAttackRatio = useCallback(
        (percent: number) => {
            if (percent !== lastSentPercentageRef.current) {
                lastSentPercentageRef.current = percent;
                gameBridge?.sendAttackRatio(percent / 100);
            }
        },
        [gameBridge],
    );

    const updatePercentage = useCallback((newPercent: number) => {
        const clampedPercent = Math.max(MIN_PERCENTAGE, Math.min(MAX_PERCENTAGE, Math.round(newPercent)));
        setPercentage(clampedPercent);
        return clampedPercent;
    }, []);

    const calculatePercentageFromPosition = useCallback(
        (clientX: number): number => {
            if (!sliderRef.current) return percentage;

            const rect = sliderRef.current.getBoundingClientRect();
            const x = clientX - rect.left;
            return (x / rect.width) * 100;
        },
        [percentage],
    );

    const handleWheelChange = useCallback(
        (deltaY: number) => {
            const delta = deltaY > 0 ? -WHEEL_DELTA : WHEEL_DELTA;
            // Skip directly to 5% when scrolling up from minimum to avoid awkward 1%→6% jump
            const newPercent = percentage === MIN_PERCENTAGE && delta > 0 ? MIN_STEP_PERCENTAGE : percentage + delta;
            const clampedPercent = updatePercentage(newPercent);
            sendAttackRatio(clampedPercent);
        },
        [percentage, updatePercentage, sendAttackRatio],
    );

    const handleMouseDown = useCallback(
        (e: React.MouseEvent<HTMLDivElement>) => {
            e.preventDefault();
            isDraggingRef.current = true;
            const rawPercent = calculatePercentageFromPosition(e.clientX);
            updatePercentage(rawPercent);
        },
        [calculatePercentageFromPosition, updatePercentage],
    );

    const handleTouchStart = useCallback(
        (e: React.TouchEvent<HTMLDivElement>) => {
            e.preventDefault();
            const touch = e.touches[0];
            if (!touch) return;

            isDraggingRef.current = true;
            const rawPercent = calculatePercentageFromPosition(touch.clientX);
            updatePercentage(rawPercent);
        },
        [calculatePercentageFromPosition, updatePercentage],
    );

    const handleMouseEnter = useCallback(() => {
        isHoveringRef.current = true;
    }, []);

    const handleMouseLeave = useCallback(() => {
        isHoveringRef.current = false;
    }, []);

    // Subscribe to leaderboard snapshots to track player's troop count
    useEffect(() => {
        if (!gameBridge) return;

        const unsubscribe = gameBridge.onLeaderboardSnapshot((snapshot: LeaderboardSnapshot) => {
            const playerEntry = snapshot.entries.find((entry) => entry.id === snapshot.local_nation_id);
            setPlayerTroops(playerEntry?.troops ?? null);
        });

        return () => unsubscribe();
    }, [gameBridge]);

    useEffect(() => {
        const handleLocalWheel = (e: WheelEvent) => {
            // Allow scroll without Shift when hovering over controls
            if (!isHoveringRef.current && !e.shiftKey) return;
            e.preventDefault();
            e.stopPropagation();
            handleWheelChange(e.deltaY);
        };

        const container = containerRef.current;
        if (container) {
            container.addEventListener("wheel", handleLocalWheel, { passive: false });
        }

        return () => {
            if (container) {
                container.removeEventListener("wheel", handleLocalWheel);
            }
        };
    }, [handleWheelChange]);

    useEffect(() => {
        const handleMouseMove = (e: MouseEvent) => {
            if (!isDraggingRef.current) return;
            const rawPercent = calculatePercentageFromPosition(e.clientX);
            updatePercentage(rawPercent);
        };

        const handleMouseUp = () => {
            if (isDraggingRef.current) {
                isDraggingRef.current = false;
                sendAttackRatio(percentage);
            }
        };

        const handleTouchMove = (e: TouchEvent) => {
            if (!isDraggingRef.current) return;
            e.preventDefault();
            const touch = e.touches[0];
            if (!touch) return;
            const rawPercent = calculatePercentageFromPosition(touch.clientX);
            updatePercentage(rawPercent);
        };

        const handleTouchEnd = () => {
            if (isDraggingRef.current) {
                isDraggingRef.current = false;
                sendAttackRatio(percentage);
            }
        };

        const handleGlobalWheel = (e: WheelEvent) => {
            if (!e.shiftKey) return;
            e.preventDefault();
            e.stopPropagation();
            handleWheelChange(e.deltaY);
        };

        // Use window-level listeners to continue drag/scroll operations even when cursor leaves slider
        window.addEventListener("mousemove", handleMouseMove);
        window.addEventListener("mouseup", handleMouseUp);
        window.addEventListener("touchmove", handleTouchMove, { passive: false });
        window.addEventListener("touchend", handleTouchEnd);
        window.addEventListener("wheel", handleGlobalWheel, { passive: false });

        return () => {
            window.removeEventListener("mousemove", handleMouseMove);
            window.removeEventListener("mouseup", handleMouseUp);
            window.removeEventListener("touchmove", handleTouchMove);
            window.removeEventListener("touchend", handleTouchEnd);
            window.removeEventListener("wheel", handleGlobalWheel);
        };
    }, [calculatePercentageFromPosition, updatePercentage, sendAttackRatio, percentage, handleWheelChange]);

    return (
        // Click-through container: pointer-events-none allows clicks to pass through to the game canvas.
        // Interactive child has pointer-events-auto to re-enable user interaction.
        <div
            ref={containerRef}
            className="absolute bottom-3 left-3 z-10 select-none opacity-95 hover:opacity-100 transition-opacity duration-400 text-base sm:text-xs md:text-sm pointer-events-none"
            onMouseEnter={handleMouseEnter}
            onMouseLeave={handleMouseLeave}
        >
            <div className="flex flex-col pointer-events-auto">
                <div className="flex flex-row items-end gap-2 min-w-96">
                    <div className="text-shadow-sm font-medium uppercase opacity-60 text-xs shrink-0 whitespace-nowrap pl-1 pb-0.5 text-white tracking-wider">
                        Troops
                    </div>
                    <div className="flex flex-1 flex-col items-end gap-0.5 mb-1">{children}</div>
                </div>
                <div className="w-104">
                    <div
                        ref={sliderRef}
                        className="bg-black/40 rounded-md relative cursor-pointer overflow-hidden border border-white/10 h-11"
                        onMouseDown={handleMouseDown}
                        onTouchStart={handleTouchStart}
                    >
                        <div
                            className="h-full absolute left-0 top-0 rounded-sm bg-blue-600/65"
                            style={{ width: `${percentage}%` }}
                        />
                        <div className="absolute left-3 top-1/2 -translate-y-1/2 text-lg font-medium tabular-nums text-white z-10 opacity-85 drop-shadow-md">
                            {percentage}%
                        </div>
                        {playerTroops !== null && (
                            <div className="absolute right-3 top-1/2 -translate-y-1/2 text-lg font-medium tabular-nums text-white z-10 opacity-85 drop-shadow-md">
                                {formatTroopCount(Math.floor((playerTroops * percentage) / 100))}
                            </div>
                        )}
                    </div>
                </div>
            </div>
        </div>
    );
}
