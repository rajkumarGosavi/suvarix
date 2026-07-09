import { defineConfig } from "vitest/config";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import path from "node:path";

export default defineConfig({
  plugins: [vue()],
  resolve: {
    alias: { "@": path.resolve(__dirname, "./src") },
  },
  test: {
    environment: "happy-dom",
    globals: true,
    include: ["src/**/*.test.ts"],
    coverage: {
      provider: "v8",
      reportsDirectory: "coverage",
      reporter: ["text", "html", "json-summary"],
      // Only code that has a test harness today — stores + composables.
      // Widen when component/view tests land.
      include: ["src/stores/**/*.ts", "src/composables/**/*.ts"],
      exclude: ["src/**/__tests__/**"],
    },
  },
});
