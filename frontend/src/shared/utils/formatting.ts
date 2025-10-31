// Format troop count for human-readable display
// 100 => "100", 12,493 => "12.4k", 980,455 => "980k"
export function formatTroopCount(count: number): string {
    if (count < 1000) return count.toString();
    if (count < 1000000) {
        const k = count / 1000;
        return k % 1 === 0 ? `${k}k` : `${k.toFixed(1)}k`;
    }
    const m = count / 1000000;
    return m % 1 === 0 ? `${m}M` : `${m.toFixed(1)}M`;
}
