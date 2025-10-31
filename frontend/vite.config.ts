import { defineConfig, HmrContext } from "vite";
import { resolve } from "path";
import { readFileSync, existsSync } from "fs";
import { execSync } from "child_process";

// Plugins
import react from "@vitejs/plugin-react";
import vike from "vike/plugin";
import { imagetools } from "vite-imagetools";
import tailwindcss from "@tailwindcss/vite";

const host = process.env.TAURI_DEV_HOST;

// Read version from workspace Cargo.toml
function getVersionFromCargoToml(): string {
    const cargoToml = readFileSync(resolve(__dirname, "../Cargo.toml"), "utf-8");
    const versionMatch = cargoToml.match(/^\[workspace\.package\][\s\S]*?^version\s*=\s*"(.+?)"$/m);
    if (!versionMatch) {
        throw new Error("Failed to find version in workspace Cargo.toml");
    }
    return versionMatch[1];
}

// Read git commit from .source-commit file or git command
function getGitCommit(): string {
    const sourceCommitPath = resolve(__dirname, "../.source-commit");
    if (existsSync(sourceCommitPath)) {
        return readFileSync(sourceCommitPath, "utf-8").trim();
    }

    // Fallback to git command for local development
    try {
        return execSync("git rev-parse HEAD", { encoding: "utf-8", cwd: resolve(__dirname, "..") }).trim();
    } catch {
        return "unknown";
    }
}

// Get current build time in UTC (ISO 8601 format)
function getBuildTime(): string {
    return new Date().toISOString();
}

const fullReloadAlways = {
    name: "full-reload-always",
    handleHotUpdate(context: HmrContext): void {
        context.server.ws.send({ type: "full-reload" });
    },
};

// https://vite.dev/config/
export default defineConfig(({ mode }) => ({
    base: process.env.GITHUB_PAGES === "true" ? "/borde.rs/" : "/",
    css: {
        transformer: "lightningcss",
    },
    plugins: [vike(), react(), imagetools(), fullReloadAlways, tailwindcss()],

    define: {
        __DESKTOP__: JSON.stringify(mode !== "browser"),
        __APP_VERSION__: JSON.stringify(getVersionFromCargoToml()),
        __GIT_COMMIT__: JSON.stringify(getGitCommit()),
        __BUILD_TIME__: JSON.stringify(getBuildTime()),
    },

    resolve: {
        alias: {
            "@wasm": resolve(__dirname, "pkg"),
            "@": resolve(__dirname, "src"),
        },
    },

    build: {
        outDir: mode === "browser" ? "dist/browser" : "dist",
        cssMinify: "lightningcss",
        sourcemap: process.env.VITE_DEBUG_BUILD === "true",
    },

    // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
    //
    // Prevents Vite from obscuring rust errors
    clearScreen: false,
    // Tauri expects a fixed port, fail if that port is not available
    server: {
        // Browser mode uses port 1421, desktop mode uses port 1420
        port: mode === "browser" ? 1421 : 1420,
        strictPort: true,
        host: host || false,
        hmr: host
            ? {
                  protocol: "ws",
                  host,
                  port: mode === "browser" ? 1422 : 1421,
              }
            : undefined,
        watch: {
            // tell Vite to ignore watching the crates
            ignored: ["**/crates/**"],
        },
    },
}));
