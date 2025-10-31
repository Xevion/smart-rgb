import { useEffect, useState, useRef, useCallback } from "react";
import { useGameBridge } from "@/shared/api";
import type { AttackEntry, AttacksUpdatePayload, LeaderboardEntry, UnsubscribeFn } from "@/shared/api/types";
import { AttackRow } from "@/shared/components/game/AttackRow";
import { AnimatePresence } from "motion/react";

// Timing constants
const WAIT_BEFORE_DISMISS_MS = 450;
const WAIT_AFTER_HOVER_MS = 250;
const REASSIGNMENT_WINDOW_MS = 2000;

type AttackPhase = "active" | "finished" | "exiting";

interface AttackRowState {
    rowId: string; // Frontend-only UUID for this display row
    currentAttack: AttackEntry | null; // null if attack has ended
    phase: AttackPhase;
    isHovered: boolean;
    firstAppearanceTime: number; // Timestamp for stable sorting
    finishedAt: number | null; // When attack ended (for reassignment window)
    dismissTimer: NodeJS.Timeout | null;
}

export function Attacks({ onNationHover }: { onNationHover: (nationId: number | null) => void }) {
    const gameBridge = useGameBridge();
    const [playerMap, setPlayerMap] = useState<Map<number, LeaderboardEntry>>(new Map());
    const [attackRows, setAttackRows] = useState<Map<string, AttackRowState>>(new Map());

    // Track current backend attack IDs to detect removals
    const currentBackendAttackIds = useRef<Set<number>>(new Set());

    // Start exit animation for a row
    const startExitAnimation = useCallback((rowId: string) => {
        setAttackRows((prevRows) => {
            const newRows = new Map(prevRows);
            const row = newRows.get(rowId);
            if (!row) return prevRows;

            // Clear any existing timer
            if (row.dismissTimer) {
                clearTimeout(row.dismissTimer);
            }

            // Transition to exiting phase
            newRows.set(rowId, {
                ...row,
                phase: "exiting",
                dismissTimer: null,
            });

            // Remove row after exit animation completes (300ms)
            setTimeout(() => {
                setAttackRows((prev) => {
                    const next = new Map(prev);
                    next.delete(rowId);
                    return next;
                });
            }, 300);

            return newRows;
        });
    }, []);

    // Find a row that can be reassigned to a new attack
    const findReassignableRow = useCallback(
        (targetId: number | null, rows: Map<string, AttackRowState>, now: number): string | null => {
            for (const [rowId, state] of rows) {
                // Only reassign if:
                // 1. Row is in 'finished' phase (not 'exiting')
                // 2. Same target
                // 3. Attack ended within reassignment window
                if (state.phase !== "finished") continue;

                const previousTarget = state.currentAttack?.target_nation_id ?? null;
                if (previousTarget !== targetId) continue;

                if (state.finishedAt === null) continue;
                const timeSinceEnd = now - state.finishedAt;
                if (timeSinceEnd > REASSIGNMENT_WINDOW_MS) continue;

                return rowId;
            }
            return null;
        },
        [],
    );

    // Reconcile backend attacks with frontend row states
    const reconcileAttacks = useCallback(
        (payload: AttacksUpdatePayload) => {
            const newBackendAttackIds = new Set(payload.entries.map((a) => a.id));

            setAttackRows((prevRows) => {
                const newRows = new Map(prevRows);
                const now = Date.now();

                // Process each backend attack
                for (const attack of payload.entries) {
                    // Check if we already have a row displaying this attack
                    let existingRowId: string | null = null;

                    for (const [rowId, rowState] of newRows) {
                        if (rowState.currentAttack?.id === attack.id) {
                            existingRowId = rowId;
                            break;
                        }
                    }

                    if (existingRowId) {
                        // Update existing row
                        const row = newRows.get(existingRowId)!;
                        newRows.set(existingRowId, {
                            ...row,
                            currentAttack: attack,
                            phase: "active",
                            finishedAt: null,
                        });
                    } else {
                        // Try to find a reassignable row
                        const reassignableRowId = findReassignableRow(attack.target_nation_id, newRows, now);

                        if (reassignableRowId) {
                            // Reassign existing row to new attack
                            const row = newRows.get(reassignableRowId)!;
                            if (row.dismissTimer) {
                                clearTimeout(row.dismissTimer);
                            }
                            newRows.set(reassignableRowId, {
                                ...row,
                                currentAttack: attack,
                                phase: "active",
                                isHovered: false,
                                finishedAt: null,
                                dismissTimer: null,
                            });
                        } else {
                            // Create new row
                            const rowId = crypto.randomUUID();
                            newRows.set(rowId, {
                                rowId,
                                currentAttack: attack,
                                phase: "active",
                                isHovered: false,
                                firstAppearanceTime: now,
                                finishedAt: null,
                                dismissTimer: null,
                            });
                        }
                    }
                }

                // Handle rows whose attacks have ended
                for (const [rowId, rowState] of newRows) {
                    if (rowState.currentAttack && !newBackendAttackIds.has(rowState.currentAttack.id)) {
                        // Attack ended
                        if (rowState.phase === "active") {
                            // Transition to finished
                            const shouldStartTimer = !rowState.isHovered;
                            const timer = shouldStartTimer
                                ? setTimeout(() => startExitAnimation(rowId), WAIT_BEFORE_DISMISS_MS)
                                : null;

                            newRows.set(rowId, {
                                ...rowState,
                                currentAttack: rowState.currentAttack,
                                phase: "finished",
                                finishedAt: now,
                                dismissTimer: timer,
                            });
                        }
                    }
                }

                currentBackendAttackIds.current = newBackendAttackIds;
                return newRows;
            });
        },
        [findReassignableRow, startExitAnimation],
    );

    // Handle hover state changes
    const handleRowHover = useCallback(
        (rowId: string, isHovered: boolean) => {
            setAttackRows((prevRows) => {
                const newRows = new Map(prevRows);
                const row = newRows.get(rowId);
                if (!row) return prevRows;

                if (isHovered) {
                    // Cancel any pending dismiss timer
                    if (row.dismissTimer) {
                        clearTimeout(row.dismissTimer);
                    }
                    newRows.set(rowId, {
                        ...row,
                        isHovered: true,
                        dismissTimer: null,
                    });
                } else {
                    // Unhovered
                    newRows.set(rowId, {
                        ...row,
                        isHovered: false,
                    });

                    // If attack is finished, start short timer before dismissing
                    if (row.phase === "finished") {
                        const timer = setTimeout(() => startExitAnimation(rowId), WAIT_AFTER_HOVER_MS);
                        newRows.set(rowId, {
                            ...newRows.get(rowId)!,
                            dismissTimer: timer,
                        });
                    }
                }

                return newRows;
            });
        },
        [startExitAnimation],
    );

    useEffect(() => {
        if (!gameBridge) return;

        let unsubscribeAttacks: UnsubscribeFn = () => {};
        let unsubscribeLeaderboard: UnsubscribeFn = () => {};

        // Subscribe to leaderboard snapshots to get player names/colors
        unsubscribeLeaderboard = gameBridge.onLeaderboardSnapshot((snapshot) => {
            setPlayerMap((prevMap) => {
                // Start with existing entries to preserve eliminated players
                const newMap = new Map(prevMap);

                // Add/update entries from the snapshot
                let hasChanges = false;
                for (const entry of snapshot.entries) {
                    const prevEntry = prevMap.get(entry.id);
                    if (!prevEntry || prevEntry.name !== entry.name || prevEntry.color !== entry.color || prevEntry.tile_count !== entry.tile_count || prevEntry.troops !== entry.troops) {
                        hasChanges = true;
                    }
                    newMap.set(entry.id, entry);
                }

                // Only update if there were actual changes
                return hasChanges ? newMap : prevMap;
            });
        });

        // Subscribe to attacks updates
        unsubscribeAttacks = gameBridge.onAttacksUpdate((payload) => {
            reconcileAttacks(payload);
        });

        return () => {
            unsubscribeAttacks();
            unsubscribeLeaderboard();

            // Clean up all timers on unmount
            setAttackRows((rows) => {
                rows.forEach((row) => {
                    if (row.dismissTimer) {
                        clearTimeout(row.dismissTimer);
                    }
                });
                return rows;
            });
        };
    }, [gameBridge, reconcileAttacks]);

    // Sort rows by first appearance time (newest first)
    const sortedRows = Array.from(attackRows.values()).sort((a, b) => b.firstAppearanceTime - a.firstAppearanceTime);

    // Keep container mounted even when empty to allow exit animations
    return (
        <div
            className="select-none text-[13px] sm:text-[10px] md:text-[11.5px] lg:text-[12px] xl:text-[12px] flex flex-col gap-0.5"
            onMouseLeave={() => onNationHover(null)}
        >
            <AnimatePresence>
                {sortedRows.map((row) => (
                    <AttackRow
                        key={row.rowId}
                        rowState={row}
                        playerMap={playerMap}
                        onNationHover={onNationHover}
                        onHoverChange={(hovered) => handleRowHover(row.rowId, hovered)}
                    />
                ))}
            </AnimatePresence>
        </div>
    );
}
