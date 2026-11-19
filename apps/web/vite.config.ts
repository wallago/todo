import react from "@vitejs/plugin-react";
import { defineConfig } from "vitest/config";
import { serviceWorker } from "./vite-sw.ts";

export default defineConfig({
  plugins: [react(), serviceWorker()],
  test: {
    environment: "jsdom",
    setupFiles: ["./src/setup-tests.ts"],
  },
});
