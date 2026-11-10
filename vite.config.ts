import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  clearScreen: false,
  build: {
    outDir: mode === "e2e" ? "dist-e2e" : "dist",
    rollupOptions: { output: { manualChunks: { charts: ["recharts"] } } },
  },
  server: { port: 1420, strictPort: true },
  test: {
    environment: "jsdom",
    setupFiles: ["./tests/ui/setup.ts"],
    include: ["tests/ui/**/*.test.{ts,tsx}"],
  },
}));
