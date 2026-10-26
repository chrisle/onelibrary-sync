import { defineConfig } from "vitest/config";
import { fileURLToPath, URL } from "node:url";

export default defineConfig({
  resolve: { alias: { "@": fileURLToPath(new URL("./src", import.meta.url)) } },
  test: {
    environment: "node",
    // Tests that mount a component opt into jsdom with a docblock.
    include: ["src/**/*.test.ts", "src/**/*.test.tsx"],
    reporters: "dot",
  },
});
