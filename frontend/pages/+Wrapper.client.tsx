import { useEffect, useState, type ReactNode } from "react";
import { GameBridgeProvider } from "@/shared/api";
import { getBridge, type GameBridge } from "@/shared/api";

// Start loading bridge immediately (not waiting for useEffect)
const bridgePromise = getBridge();

export default function Wrapper({ children }: { children: ReactNode }) {
    const [bridge, setBridge] = useState<GameBridge | null>(null);

    // Dynamically import the appropriate platform implementation
    useEffect(() => {
        bridgePromise.then(setBridge);
    }, []);

    // Browser-specific setup (must be before early return to satisfy Rules of Hooks)
    useEffect(() => {
        if (!__DESKTOP__) {
            // Handle user ID storage from worker
            const userIdChannel = new BroadcastChannel("user_id_storage");

            userIdChannel.onmessage = (event) => {
                try {
                    const msg = JSON.parse(event.data as string);
                    if (msg.action === "save") {
                        localStorage.setItem("app_session_id", msg.id);
                    } else if (msg.action === "load") {
                        const id = localStorage.getItem("app_session_id");
                        if (id) {
                            userIdChannel.postMessage(JSON.stringify({ action: "load_response", id }));
                        }
                    }
                } catch {}
            };

            return () => {
                userIdChannel.close();
            };
        }
    }, []);

    return bridge ? <GameBridgeProvider bridge={bridge}>{children}</GameBridgeProvider> : null;
}
