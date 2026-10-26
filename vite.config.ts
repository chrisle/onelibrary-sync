import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { fileURLToPath, URL } from "node:url";
import process from "node:process";

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [react()],
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
