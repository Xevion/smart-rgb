import type { UnsubscribeFn } from "@/shared/api/types";

/**
 * Generic callback manager for event subscriptions.
 * Handles registration, notification, and cleanup of callback functions.
 */
export class CallbackManager<T = void> {
    private callbacks: Array<T extends void ? () => void : (data: T) => void> = [];

    /**
     * Subscribe to events with a callback.
     * Returns an unsubscribe function to remove this specific callback.
     */
    subscribe(callback: T extends void ? () => void : (data: T) => void): UnsubscribeFn {
        this.callbacks.push(callback);

        return () => {
            const index = this.callbacks.indexOf(callback);
            if (index !== -1) {
                this.callbacks.splice(index, 1);
            }
        };
    }

    /**
     * Notify all subscribed callbacks with data.
     * For void callbacks, call with no arguments.
     */
    notify(data?: T): void {
        if (data === undefined) {
            // Handle void callbacks
            this.callbacks.forEach((callback) => (callback as () => void)());
        } else {
            // Handle callbacks with data
            this.callbacks.forEach((callback) => (callback as (data: T) => void)(data));
        }
    }

    /**
     * Clear all callbacks (used for cleanup/destroy).
     */
    clear(): void {
        this.callbacks = [];
    }
}
