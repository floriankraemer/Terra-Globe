import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests",
  timeout: 30_000,
  // Each test drives a real WebGL/Cesium context; too much parallelism causes
  // GPU/CPU contention flakiness (esp. under software rendering in CI/sandboxes).
  workers: 2,
  webServer: {
    command: "pnpm --filter @terra-globe/ui preview -- --port 4173 --strictPort",
    url: "http://localhost:4173",
    reuseExistingServer: !process.env.CI,
    cwd: "../..",
    timeout: 60_000,
  },
  use: {
    baseURL: "http://localhost:4173",
  },
});
