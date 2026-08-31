import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";

export default defineConfig({
  plugins: [react(), tailwindcss()],
  resolve: {
    alias: {
      "@": fileURLToPath(new URL("./src", import.meta.url)),
    },
  },
  test: {
    environment: "jsdom",
    globals: false,
    setupFiles: ["src/test/setup.ts"],
    exclude: ["node_modules", "dist", "tests/e2e/**"],
    // React 19 exports `act` only from its development build; keep component
    // tests on the development export condition instead of production.
    env: { NODE_ENV: "development" },
  },
});
