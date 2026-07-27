import { readFileSync } from "node:fs";

export interface LibraryInfo {
  name: string;
  version: string;
  url: string;
}

const LIBRARY_HOMEPAGES: Record<string, string> = {
  react: "https://react.dev",
  cesium: "https://cesium.com/platform/cesiumjs/",
  "@tauri-apps/api": "https://tauri.app",
  i18next: "https://www.i18next.com",
  "lucide-react": "https://lucide.dev",
  zustand: "https://github.com/pmndrs/zustand",
};

/** App version + third-party library list shown in the About modal, read at build time so it can't drift from package.json/tauri.conf.json. */
export function getAppMetadata(): { version: string; libraries: LibraryInfo[] } {
  const { version } = JSON.parse(
    readFileSync(new URL("../../src-tauri/tauri.conf.json", import.meta.url), "utf-8"),
  ) as { version: string };
  const { dependencies } = JSON.parse(
    readFileSync(new URL("./package.json", import.meta.url), "utf-8"),
  ) as { dependencies: Record<string, string> };

  const libraries = Object.entries(LIBRARY_HOMEPAGES).map(([name, url]) => ({
    name,
    version: dependencies[name]?.replace(/^[\^~]/, "") ?? "",
    url,
  }));

  return { version, libraries };
}
