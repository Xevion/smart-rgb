import { useCallback, useRef } from "react";

/**
 * Creates a throttled callback with trailing edge behavior.
 * Ensures the last call is executed after the throttle period elapses.
 *
 * @param callback - The function to throttle
 * @param delay - The throttle delay in milliseconds
 * @returns A throttled version of the callback
 */
export function useThrottledCallback<T extends (...args: any[]) => void>(callback: T, delay: number): T {
    const lastCallTimeRef = useRef<number>(0);
    const pendingTimeoutRef = useRef<number | null>(null);
    const callbackRef = useRef(callback);

    // Keep callback ref updated
    callbackRef.current = callback;

    const throttledCallback = useCallback(
        (...args: Parameters<T>) => {
            const now = Date.now();
            const timeSinceLastCall = now - lastCallTimeRef.current;

            const executeCallback = () => {
                callbackRef.current(...args);
            };

            if (timeSinceLastCall >= delay) {
                // Execute immediately if throttle period has elapsed
                executeCallback();
                lastCallTimeRef.current = now;

                // Clear any pending timeout since we just executed
                if (pendingTimeoutRef.current !== null) {
                    clearTimeout(pendingTimeoutRef.current);
                    pendingTimeoutRef.current = null;
                }
            } else {
                // Schedule execution after the remaining throttle period
                if (pendingTimeoutRef.current !== null) {
                    clearTimeout(pendingTimeoutRef.current);
                }

                const remainingTime = delay - timeSinceLastCall;
                pendingTimeoutRef.current = window.setTimeout(() => {
                    executeCallback();
                    lastCallTimeRef.current = Date.now();
                    pendingTimeoutRef.current = null;
                }, remainingTime);
            }
        },
        [delay],
    ) as T;

    // Cleanup on unmount
    const cleanupRef = useRef(() => {
        if (pendingTimeoutRef.current !== null) {
            clearTimeout(pendingTimeoutRef.current);
            pendingTimeoutRef.current = null;
        }
    });

    // Update cleanup function
    cleanupRef.current = () => {
        if (pendingTimeoutRef.current !== null) {
            clearTimeout(pendingTimeoutRef.current);
            pendingTimeoutRef.current = null;
        }
    };

    return throttledCallback;
}
