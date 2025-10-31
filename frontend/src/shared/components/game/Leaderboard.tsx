import { useEffect, useMemo, useState, useRef } from "react";
import { useGameBridge } from "@/shared/api";
import type { LeaderboardSnapshot, UnsubscribeFn } from "@/shared/api/types";
import * as motion from "motion/react-client";
import type { Transition } from "motion/react";

// Smart precision algorithm for percentage display
function calculatePrecision(percentages: number[]): number {
    if (percentages.length === 0) return 0;

    // Find the minimum non-zero difference between consecutive percentages
    const sorted = [...percentages].sort((a, b) => b - a);
    let minDiff = Infinity;

    for (let i = 0; i < sorted.length - 1; i++) {
        const diff = sorted[i] - sorted[i + 1];
        if (diff > 0) {
            minDiff = Math.min(minDiff, diff);
        }
    }

    // If all percentages are the same, use 0 decimal places
    if (minDiff === Infinity) return 0;

    // Determine precision based on the minimum difference
    if (minDiff >= 0.1) return 0; // 0.1% or more difference -> 0 decimals
    if (minDiff >= 0.01) return 1; // 0.01% or more difference -> 1 decimal
    return 2; // 0.001% or more difference -> 2 decimals (max precision)
}

const VISIBLE_TOP_N = 8;
const RENDERED_BUFFER = 15;

const transition: Transition = {
    type: "tween",
    duration: 0.5,
    ease: "easeInOut",
};

function focusNation(nationId: number) {
    console.log("Focus nation:", nationId);
}

export function Leaderboard({
    initialSnapshot,
    highlightedNation,
    onNationHover,
}: {
    initialSnapshot?: LeaderboardSnapshot | null;
    highlightedNation: number | null;
    onNationHover: (nationId: number | null) => void;
}) {
    const gameBridge = useGameBridge();
    const [collapsed, setCollapsed] = useState(false);
    const [snapshot, setSnapshot] = useState<LeaderboardSnapshot | null>(initialSnapshot || null);
    const [status, setStatus] = useState<"loading" | "waiting" | "ready" | "error">(initialSnapshot ? "ready" : "waiting");
    const [containerHeight, setContainerHeight] = useState<number | null>(null);
    const tableRef = useRef<HTMLTableElement>(null);

    useEffect(() => {
        if (!gameBridge) return;

        let unsubscribe: UnsubscribeFn = () => {};

        // Subscribe to leaderboard snapshots
        try {
            unsubscribe = gameBridge.onLeaderboardSnapshot((snapshotData) => {
                setSnapshot(snapshotData);
                setStatus("ready");
            });
        } catch (error) {
            console.warn("Failed to subscribe to leaderboard snapshots:", error);
            setStatus("error");
        }

        return () => {
            unsubscribe();
        };
    }, [gameBridge]);

    const { topRows, playerEntry, playerInTopN } = useMemo(() => {
        if (!snapshot) {
            return {
                topRows: [],
                playerEntry: null,
                playerInTopN: false,
            };
        }

        // Sort entries by display_order for visual positioning
        const sortedEntries = [...snapshot.entries].sort((a, b) => a.display_order - b.display_order);

        // Render top 15 rows for animation buffer (only top 8 will be visible)
        const topRows = sortedEntries.slice(0, RENDERED_BUFFER);

        // Find player and check if they're in visible top N
        const playerEntry = sortedEntries.find((e) => e.id === snapshot.local_nation_id);
        const visibleTopEntries = sortedEntries.slice(0, VISIBLE_TOP_N);
        const playerInTopN = playerEntry ? visibleTopEntries.some((e) => e.id === playerEntry.id) : false;

        return {
            topRows,
            playerEntry,
            playerInTopN,
        };
    }, [snapshot]);

    const precision = useMemo(() => {
        if (!snapshot || topRows.length === 0) return 0;
        const percentages = topRows.map((r) => r.territory_percent);
        return calculatePrecision(percentages);
    }, [snapshot, topRows]);

    // Dynamically calculate container height based on actual row height
    useEffect(() => {
        if (!tableRef.current || topRows.length === 0) return;

        // Wait for next frame to ensure rows are rendered
        requestAnimationFrame(() => {
            const firstRow = tableRef.current?.querySelector("tr");
            if (firstRow) {
                const rowHeight = firstRow.getBoundingClientRect().height;
                // 8 visible rows with small buffer
                setContainerHeight(rowHeight * VISIBLE_TOP_N + 1);
            }
        });
    }, [topRows.length]);

    function renderRow(
        entry: typeof playerEntry,
        isPlayer: boolean,
        isHighlighted: boolean,
        clickBehavior: "focus" | "collapse" | "expand" | "none",
        showActualRank = false, // If true, show entry.rank; otherwise show display_order + 1
        animate = true,
    ) {
        if (!entry) return null;

        const isClickable = clickBehavior !== "none";
        const className = `leading-7 transition-colors duration-150 ${
            isClickable ? "cursor-pointer hover:bg-white/[0.06]" : "cursor-default"
        } ${isPlayer ? "text-white" : "text-white/75"} ${isHighlighted ? "!bg-white/[0.12]" : ""}`;

        const handleClick = () => {
            if (clickBehavior === "none") return;

            switch (clickBehavior) {
                case "focus":
                    focusNation(entry.id);
                    break;
                case "collapse":
                    setCollapsed(true);
                    break;
                case "expand":
                    setCollapsed(false);
                    break;
            }
        };

        const displayedRank = showActualRank ? entry.rank : entry.display_order + 1;

        const content = (
            <>
                <td className="pl-3 pr-2 text-center whitespace-nowrap w-12 min-w-12 tracking-tighter">{displayedRank}</td>
                <td className="pr-3 text-left overflow-hidden text-ellipsis whitespace-nowrap w-[55%]">
                    <div className="flex items-center gap-2">
                        <div className="size-3 rounded-full shrink-0 shadow-md" style={{ backgroundColor: `#${entry.color}` }} />
                        <span>{entry.name}</span>
                    </div>
                </td>
                <td className="px-3 text-right whitespace-nowrap w-[20%]">{`${(entry.territory_percent * 100).toFixed(precision)}%`}</td>
                <td className="pl-3 pr-4 text-right whitespace-nowrap w-[20%] min-w-20">{entry.troops.toLocaleString()}</td>
            </>
        );

        if (!animate) {
            return (
                <tr
                    key={entry.id}
                    className={className}
                    onClick={handleClick}
                    onMouseEnter={() => onNationHover(entry.id)}
                    onMouseLeave={() => onNationHover(null)}
                >
                    {content}
                </tr>
            );
        }

        return (
            <motion.tr
                key={entry.id}
                layout
                layoutId={`nation-${entry.id}`}
                transition={transition}
                className={className}
                onClick={handleClick}
                onMouseEnter={() => onNationHover(entry.id)}
                onMouseLeave={() => onNationHover(null)}
            >
                {content}
            </motion.tr>
        );
    }

    return (
        // pointer-events-auto: Required if this component is rendered inside a pointer-events-none container
        <div
            className="absolute top-0 left-3 z-10 text-sm select-none pointer-events-auto opacity-60 hover:opacity-100 transition-opacity duration-400 max-xl:text-xs max-[800px]:text-[0.72rem] max-[600px]:text-[0.625rem]"
            onMouseLeave={() => onNationHover(null)}
        >
            <div className="bg-slate-900/60 rounded-bl-lg rounded-br-lg w-96">
                {status === "ready" && snapshot ? (
                    <>
                        <div
                            className="overflow-hidden"
                            style={collapsed && playerEntry ? undefined : containerHeight ? { maxHeight: `${containerHeight}px` } : undefined}
                        >
                            <table className="w-96 border-collapse" ref={tableRef}>
                                <tbody>
                                    {collapsed && playerEntry
                                        ? renderRow(playerEntry, true, highlightedNation === playerEntry.id, "expand", true, false)
                                        : topRows.map((r) => {
                                              const isPlayer = r.id === snapshot.local_nation_id;
                                              const isHighlighted = highlightedNation === r.id;
                                              return renderRow(r, isPlayer, isHighlighted, isPlayer ? "none" : "focus");
                                          })}
                                </tbody>
                            </table>
                        </div>

                        {!collapsed && playerEntry && (
                            <div className="bg-slate-900/65 rounded-lg">
                                <table className="w-96 border-collapse">
                                    <tbody>
                                        {!playerInTopN ? (
                                            renderRow(playerEntry, true, highlightedNation === playerEntry.id, "collapse", true)
                                        ) : (
                                            <tr>
                                                <td colSpan={4} className="text-center text-xs text-white/75 py-0.5">
                                                    <button className="w-full cursor-pointer" onClick={() => setCollapsed(true)}>
                                                        Collapse
                                                    </button>
                                                </td>
                                            </tr>
                                        )}
                                    </tbody>
                                </table>
                            </div>
                        )}
                    </>
                ) : (
                    <div className="p-4 text-center text-slate-400 italic">
                        {status === "loading" && "Loading leaderboard…"}
                        {status === "waiting" && "Waiting for updates…"}
                        {status === "error" && "Error loading leaderboard"}
                    </div>
                )}
            </div>
        </div>
    );
}
