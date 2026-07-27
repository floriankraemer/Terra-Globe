import { readFileSync } from "node:fs";
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import cesium from "vite-plugin-cesium";

const { version } = JSON.parse(
  readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf-8"),
) as { version: string };

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), cesium()],
  clearScreen: false,
  server: {
    strictPort: true,
  },
  define: {
    __APP_VERSION__: JSON.stringify(version),
  },
});
