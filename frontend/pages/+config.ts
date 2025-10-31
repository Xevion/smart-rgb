import vikeReact from "vike-react/config";

export default {
    extends: [vikeReact],

    // Enable pre-rendering for static site generation
    prerender: true,

    // Global head configuration
    title: "Iron Borders",
    description: "Strategic Territory Control",
    // Disable React StrictMode to avoid double-mounting issues with PixiJS
    reactStrictMode: false,
};
