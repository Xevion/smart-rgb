/// <reference types="vite/client" />

declare const __DESKTOP__: boolean;
declare const __APP_VERSION__: string;
declare const __GIT_COMMIT__: string;
declare const __BUILD_TIME__: string;

declare module "@wasm/borders.js" {
    export { default } from "../pkg/borders";
    export * from "../pkg/borders";
}

// vite-imagetools support
declare module "*&imagetools" {
    const out: any;
    export default out;
}
