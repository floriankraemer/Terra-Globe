import { useEffect, useRef, useState } from "react";
import * as Cesium from "cesium";
import {
  captureAreaImage,
  computeExportPlan,
  createViewer,
  ExportTooLargeError,
  MAX_TILE_DIMENSION_PX,
} from "@terra-globe/map";
import { useLibrary } from "../features/folders/useLibrary.js";
import {
  blobToBase64,
  EXPORT_WORKER_PROGRESS_EVENT,
  EXPORT_WORKER_READY_EVENT,
  EXPORT_WORKER_REQUEST_EVENT,
  EXPORT_WORKER_RESULT_EVENT,
  type ExportWorkerRequest,
} from "../features/area-export/exportWorkerProtocol.js";

// Mirrors useAreaExport.ts's tileDimensionPx() - see the comment there.
const maxTextureSize = (): number =>
  (Cesium as unknown as { ContextLimits: { maximumTextureSize: number } }).ContextLimits
    .maximumTextureSize;
const tileDimensionPx = (): number => Math.min(maxTextureSize(), MAX_TILE_DIMENSION_PX);

/**
 * Renders in a hidden, desktop-only Tauri window (see useAreaExport.ts's Tauri branch). It mounts
 * its own Cesium viewer and place library so area-export tile rendering never resizes or steals
 * render-loop time from the main window's visible map.
 */
export function ExportWorkerApp() {
  const containerRef = useRef<HTMLDivElement>(null);
  const [viewer, setViewer] = useState<Cesium.Viewer | null>(null);
  const library = useLibrary(viewer);

  useEffect(() => {
    if (!containerRef.current) return;
    const handle = createViewer(containerRef.current);
    setViewer(handle.viewer);
    return () => {
      handle.destroy();
      setViewer(null);
    };
  }, []);

  useEffect(() => {
    if (!library.ready) return;
    void import("@tauri-apps/api/event").then(({ emit }) => emit(EXPORT_WORKER_READY_EVENT));
  }, [library.ready]);

  useEffect(() => {
    if (!viewer) return;
    const activeViewer = viewer;
    let unlisten: (() => void) | undefined;
    let cancelled = false;

    void import("@tauri-apps/api/event").then(({ listen, emit }) => {
      listen<ExportWorkerRequest>(EXPORT_WORKER_REQUEST_EVENT, (event) => {
        void handleRequest(event.payload);
      }).then((fn) => {
        if (cancelled) fn();
        else unlisten = fn;
      });

      async function handleRequest(request: ExportWorkerRequest): Promise<void> {
        try {
          const tileDim = tileDimensionPx();
          const plan = computeExportPlan(
            request.bounds,
            request.scaleDenominator,
            request.dpiValue,
            tileDim,
          );
          const blob = await captureAreaImage(
            activeViewer,
            request.bounds,
            plan,
            tileDim,
            (done, total) =>
              void emit(EXPORT_WORKER_PROGRESS_EVENT, {
                requestId: request.requestId,
                done,
                total,
              }),
          );
          const pngBase64 = await blobToBase64(blob);
          await emit(EXPORT_WORKER_RESULT_EVENT, { requestId: request.requestId, pngBase64 });
        } catch (err) {
          const message = err instanceof Error ? err.message : String(err);
          const tooLarge =
            err instanceof ExportTooLargeError
              ? {
                  pixelWidth: err.pixelWidth,
                  pixelHeight: err.pixelHeight,
                  maxDimensionPx: err.maxDimensionPx,
                  maxMegapixels: err.maxMegapixels,
                }
              : undefined;
          await emit(EXPORT_WORKER_RESULT_EVENT, {
            requestId: request.requestId,
            error: message,
            tooLarge,
          });
        }
      }
    });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [viewer]);

  return <div ref={containerRef} style={{ position: "fixed", inset: 0 }} />;
}
