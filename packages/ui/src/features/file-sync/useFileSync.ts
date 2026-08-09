import { useCallback, useEffect, useRef, useState } from "react";
import {
  parseKml,
  parseKmz,
  resolveNetworkLinks,
  serializeKml,
  serializeKmz,
} from "@terra-globe/core";
import type { LibrarySnapshot } from "../folders/useLibrary.js";
import { isTauri } from "../../platform/isTauri.js";

const AUTOSAVE_DEBOUNCE_MS = 1500;
const KML_KMZ_FILTER = [{ name: "KML/KMZ", extensions: ["kml", "kmz"] }];

export interface LoadResult {
  filePath: string;
  snapshot: LibrarySnapshot;
  warnings: string[];
}

export interface UseFileSyncResult {
  filePath: string | null;
  dirty: boolean;
  saveError: string | null;
  /** Wraps a library mutation: after it resolves, marks the file dirty and (if autosave is on and a file is loaded) schedules a debounced write. */
  wrap: <T>(action: () => Promise<T>) => Promise<T>;
  /** Opens the native file picker, reads and parses the chosen file. Does not itself replace the library - the caller restores the returned snapshot, then calls onLoaded. */
  loadFromDisk: () => Promise<LoadResult | null>;
  /** Records the path/format of a file the caller has just finished loading and clears the dirty flag. */
  onLoaded: (path: string) => void;
  /** Writes the current library to filePath, or opens a Save As dialog first if nothing has been loaded/saved yet. */
  saveNow: () => Promise<void>;
}

interface Library {
  exportAll: () => Promise<LibrarySnapshot>;
}

function detectFormat(path: string): "kml" | "kmz" | null {
  const lower = path.toLowerCase();
  if (lower.endsWith(".kmz")) return "kmz";
  if (lower.endsWith(".kml")) return "kml";
  return null;
}

/**
 * Tracks the currently loaded KML/KMZ file path and autosaves the library
 * back to it on every change - Tauri desktop only. In the browser build
 * (no real filesystem write access) this is an inert stub so App.tsx needs
 * no isTauri() branching at call sites.
 */
export function useFileSync(library: Library, autoSave: boolean): UseFileSyncResult {
  const tauri = isTauri();
  const [filePath, setFilePath] = useState<string | null>(null);
  const [dirty, setDirty] = useState(false);
  const [saveError, setSaveError] = useState<string | null>(null);

  const filePathRef = useRef(filePath);
  filePathRef.current = filePath;
  const autoSaveRef = useRef(autoSave);
  autoSaveRef.current = autoSave;
  const debounceRef = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  // Only one write in flight at a time: a debounce firing while a manual/exit
  // save is already writing queues at most one follow-up run instead of
  // racing two writes to the same path.
  // ponytail: single mutex per loaded file, fine for one document per window;
  // revisit if multi-window/multi-document editing is ever added.
  const savingRef = useRef<Promise<void> | null>(null);
  const pendingRef = useRef(false);

  useEffect(() => () => clearTimeout(debounceRef.current), []);

  const saveNow = useCallback(async (): Promise<void> => {
    if (!tauri) return;
    if (savingRef.current) {
      pendingRef.current = true;
      return savingRef.current;
    }
    const run = async (): Promise<void> => {
      try {
        let path = filePathRef.current;
        let format = path ? detectFormat(path) : null;
        if (!path || !format) {
          const { save } = await import("@tauri-apps/plugin-dialog");
          const picked = await save({ filters: KML_KMZ_FILTER });
          if (!picked) return; // user cancelled the Save As dialog
          path = picked;
          format = detectFormat(path) ?? "kml";
        }
        const snapshot = await library.exportAll();
        const bytes =
          format === "kmz"
            ? await serializeKmz(snapshot)
            : new TextEncoder().encode(serializeKml(snapshot));
        const { invoke } = await import("@tauri-apps/api/core");
        await invoke("write_binary_file", { path, contents: Array.from(bytes) });
        setFilePath(path);
        setDirty(false);
        setSaveError(null);
      } catch (err) {
        setSaveError(err instanceof Error ? err.message : String(err));
      }
    };
    const promise = run();
    savingRef.current = promise;
    await promise;
    savingRef.current = null;
    if (pendingRef.current) {
      pendingRef.current = false;
      await saveNow();
    }
  }, [tauri, library]);

  const scheduleAutosave = useCallback(() => {
    clearTimeout(debounceRef.current);
    if (!autoSaveRef.current || !filePathRef.current) return;
    debounceRef.current = setTimeout(() => void saveNow(), AUTOSAVE_DEBOUNCE_MS);
  }, [saveNow]);

  const wrap = useCallback(
    async <T>(action: () => Promise<T>): Promise<T> => {
      const result = await action();
      if (tauri) {
        setDirty(true);
        scheduleAutosave();
      }
      return result;
    },
    [tauri, scheduleAutosave],
  );

  const loadFromDisk = useCallback(async (): Promise<LoadResult | null> => {
    if (!tauri) return null;
    const { open } = await import("@tauri-apps/plugin-dialog");
    const picked = await open({ multiple: false, filters: KML_KMZ_FILTER });
    if (!picked || Array.isArray(picked)) return null;
    const format = detectFormat(picked);
    if (!format) return null;

    const { invoke } = await import("@tauri-apps/api/core");
    const bytes = await invoke<number[]>("read_binary_file", { path: picked });
    const data = new Uint8Array(bytes);
    let result = format === "kmz" ? await parseKmz(data) : parseKml(new TextDecoder().decode(data));
    if (result.networkLinks.length > 0) {
      result = await resolveNetworkLinks(result);
    }
    return {
      filePath: picked,
      snapshot: {
        folders: result.folders,
        placemarks: result.placemarks,
        styles: result.styles,
        screenOverlays: result.screenOverlays,
      },
      warnings: result.warnings,
    };
  }, [tauri]);

  const onLoaded = useCallback((path: string) => {
    setFilePath(path);
    setDirty(false);
    setSaveError(null);
  }, []);

  return { filePath, dirty, saveError, wrap, loadFromDisk, onLoaded, saveNow };
}
