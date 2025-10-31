import type { LeaderboardEntry } from "@/shared/api/types";
import { formatTroopCount } from "@/shared/utils/formatting";
import { match, P } from "ts-pattern";
import * as motion from "motion/react-client";

// Attack row state from AttacksList parent component
interface AttackRowState {
    rowId: string;
    currentAttack: {
        id: number;
        attacker_nation_id: number;
        target_nation_id: number | null;
        troops: number;
        is_outgoing: boolean;
    } | null;
    phase: "active" | "finished" | "exiting";
    isHovered: boolean;
    firstAppearanceTime: number;
    finishedAt: number | null;
    dismissTimer: NodeJS.Timeout | null;
}

// Calculate the background width percentage based on troop count
// Uses power scale (x^0.4): 100 troops = 5%, 1k = 7.9%, 10k = 15.3%, 100k = 33.7%, 1M = 80%
function calculateBackgroundWidth(troops: number): number {
    const minTroops = 100;
    const maxTroops = 1000000;
    const minWidth = 0;
    const maxWidth = 80;

    // Clamp troops to range
    const clampedTroops = Math.max(minTroops, Math.min(maxTroops, troops));

    // Power scale with exponent 0.4 provides gentler progression than logarithmic
    const powerMin = Math.pow(minTroops, 0.4);
    const powerMax = Math.pow(maxTroops, 0.4);
    const powerTroops = Math.pow(clampedTroops, 0.4);

    const normalized = (powerTroops - powerMin) / (powerMax - powerMin);
    return minWidth + normalized * (maxWidth - minWidth);
}

interface AttackRowProps {
    rowState: AttackRowState;
    playerMap: Map<number, LeaderboardEntry>;
    onNationHover?: (nationId: number | null) => void;
    onHoverChange: (isHovered: boolean) => void;
}

export function AttackRow({ rowState, playerMap, onNationHover, onHoverChange }: AttackRowProps) {
    // Use current attack if active, otherwise display last known attack (for finished rows)
    const attack = rowState.currentAttack;

    // Don't render if we have no attack data (shouldn't happen, but safety check)
    if (!attack) return null;

    // For outgoing attacks, show target's name (who we're attacking)
    // For incoming attacks, show attacker's name (who is attacking us)
    const displayPlayerId = attack.is_outgoing ? attack.target_nation_id : attack.attacker_nation_id;

    const displayName = match({ id: displayPlayerId, outgoing: attack.is_outgoing })
        .with({ id: P.number, outgoing: P.boolean }, ({ id }) => playerMap.get(id)?.name || `Error: Player ${id} not found`)
        .with({ id: P.nullish, outgoing: P.boolean }, ({ outgoing }) =>
            outgoing ? "Unclaimed Territory" : "Error: Incoming attack with no target found",
        )
        .exhaustive();

    const backgroundWidth = calculateBackgroundWidth(attack.troops);
    const backgroundColor = attack.is_outgoing ? "rgba(59, 130, 246, 0.5)" : "rgba(239, 68, 68, 0.5)";

    return (
        <motion.div
            layout
            layoutId={`attack-row-${rowState.rowId}`}
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.8 }}
            transition={{
                duration: 0.2,
                layout: { duration: 0.3, ease: "easeInOut" },
            }}
        >
            <button
                className="relative flex justify-between items-center py-2 px-3 border-none bg-slate-900/60 backdrop-blur-sm text-white cursor-pointer transition-all duration-150 text-lg font-inherit w-80 rounded-lg overflow-hidden hover:bg-slate-900/75 opacity-60 hover:opacity-100"
                onClick={() => console.log("Attack clicked:", attack)}
                onMouseEnter={() => {
                    if (displayPlayerId !== null) {
                        onNationHover?.(displayPlayerId);
                    }
                    onHoverChange(true);
                }}
                onMouseLeave={() => {
                    onNationHover?.(null);
                    onHoverChange(false);
                }}
            >
                <div
                    className="absolute top-0 right-0 h-full transition-[width] duration-300"
                    style={{
                        width: `${backgroundWidth}%`,
                        backgroundColor,
                    }}
                />
                <div className="text-left overflow-hidden text-ellipsis whitespace-nowrap flex-1 pr-4 relative z-10">
                    {displayName}
                </div>
                <div className="text-right tabular-nums whitespace-nowrap min-w-20 relative z-10">
                    {formatTroopCount(attack.troops)}
                </div>
            </button>
        </motion.div>
    );
}
