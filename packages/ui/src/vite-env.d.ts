/// <reference types="vite/client" />

/** App version, injected at build time from src-tauri/tauri.conf.json - see vite.config.ts. */
declare const __APP_VERSION__: string;

/** Third-party libraries shown in the About modal, injected at build time - see appMetadata.ts. */
declare const __APP_LIBRARIES__: { name: string; version: string; url: string }[];
