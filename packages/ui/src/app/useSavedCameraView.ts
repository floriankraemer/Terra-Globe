import { useEffect } from "react";
import type * as Cesium from "cesium";
import { applyCameraView, getCameraView, type CameraView } from "@terra-globe/map";

const NUMERIC_FIELDS = ["longitude", "latitude", "height", "heading", "pitch", "roll"] as const;

/** Parses a stored camera view, returning null for missing/corrupt/malformed data. */
export function parseCameraView(raw: string | null): CameraView | null {
  if (raw === null) return null;
  try {
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) return null;
    const record = parsed as Record<string, unknown>;
    if (!NUMERIC_FIELDS.every((field) => Number.isFinite(record[field]))) return null;
    return Object.fromEntries(
      NUMERIC_FIELDS.map((field) => [field, record[field] as number]),
    ) as unknown as CameraView;
  } catch {
    return null;
  }
}

/**
 * Restores the camera pose saved from the last session (if any), then keeps
 * saving the current pose to localStorage whenever the camera comes to rest.
 * Saving on moveEnd - rather than on beforeunload/pagehide - survives a
 * crash or force-quit, since it never depends on an unload event actually
 * firing.
 */
export function useSavedCameraView(
  viewer: Cesium.Viewer | null,
  storageKey = "terra-globe:cameraView",
): void {
  useEffect(() => {
    if (!viewer) return;
    const activeViewer = viewer;

    const stored = parseCameraView(window.localStorage.getItem(storageKey));
    if (stored) applyCameraView(activeViewer, stored);

    function save(): void {
      try {
        window.localStorage.setItem(storageKey, JSON.stringify(getCameraView(activeViewer)));
      } catch {
        // localStorage unavailable (e.g. private browsing) - view just won't persist.
      }
    }

    activeViewer.camera.moveEnd.addEventListener(save);
    return () => {
      activeViewer.camera.moveEnd.removeEventListener(save);
    };
  }, [viewer, storageKey]);
}
