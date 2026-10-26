import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import process from "node:process";
import packageJson from "./package.json" with { type: "json" };

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
  // The release workflow sets package.json's version from the tag before it
  // builds (scripts/sync-version.mjs).
  define: { __APP_VERSION__: JSON.stringify(packageJson.version) },
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  clearScreen: false,
  build: {
    // WKWebView (macOS 13+) and WebView2 (Edge 110+) are the only targets.
    target: ["safari16", "edge110"],
    sourcemap: false,
  },
  server: {
    // Beside rbxport's 1420 and RBXport Restore's 1430, so all can run in
    // development at once.
    port: 1440,
    strictPort: true,
    host: host || false,
    hmr: host ? { protocol: "ws", host, port: 1441 } : undefined,
    watch: { ignored: ["**/src-tauri/**", "**/target/**"] },
  },
});
