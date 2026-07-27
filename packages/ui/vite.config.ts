import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import cesium from "vite-plugin-cesium";
import { getAppMetadata } from "./appMetadata.js";

const { version, libraries } = getAppMetadata();

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [react(), cesium()],
  clearScreen: false,
  server: {
    strictPort: true,
  },
  define: {
    __APP_VERSION__: JSON.stringify(version),
    __APP_LIBRARIES__: JSON.stringify(libraries),
  },
});
