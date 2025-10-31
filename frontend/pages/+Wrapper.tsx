import { type ReactNode } from "react";
// import { GameBridgeProvider } from "@/shared/api";

// Server-side wrapper provides stub implementations for SSG/pre-rendering
// The real implementations are provided by +Wrapper.client.tsx on the client
export default function Wrapper({ children }: { children: ReactNode }) {
    // During SSR/pre-rendering, provide null implementations
    // These will be replaced by real implementations when the client-side wrapper takes over
    // return <GameBridgeProvider bridge={null as any}>{children}</GameBridgeProvider>;
    return <>{children}</>;
}
