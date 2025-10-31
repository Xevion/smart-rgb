import { useState, useEffect } from "react";

interface SpawnPhaseOverlayProps {
    isVisible: boolean;
    countdown: { startedAtMs: number; durationSecs: number } | null;
}

export function SpawnPhaseOverlay({ isVisible, countdown }: SpawnPhaseOverlayProps) {
    const [progress, setProgress] = useState(1.0); // 1.0 = full, 0.0 = empty

    useEffect(() => {
        if (!isVisible || !countdown) {
            setProgress(1.0);
            return;
        }

        const animate = () => {
            const nowMs = Date.now();
            const elapsedMs = nowMs - countdown.startedAtMs;
            const elapsedSecs = elapsedMs / 1000;
            const remaining = Math.max(0, countdown.durationSecs - elapsedSecs);
            const newProgress = remaining / countdown.durationSecs;

            setProgress(newProgress);

            if (newProgress > 0) {
                requestAnimationFrame(animate);
            }
        };

        const frameId = requestAnimationFrame(animate);
        return () => cancelAnimationFrame(frameId);
    }, [isVisible, countdown]);

    // Hide overlay when explicitly hidden or when countdown reaches 0
    if (!isVisible || progress <= 0) return null;

    return (
        // Timeout bar only - sits at top of UI container
        <div className="w-full h-2 pointer-events-none relative bg-black/50 overflow-hidden">
            <div
                className="h-full bg-gradient-to-r from-green-500 to-lime-400"
                style={{ width: `${progress * 100}%` }}
            />
        </div>
    );
}
