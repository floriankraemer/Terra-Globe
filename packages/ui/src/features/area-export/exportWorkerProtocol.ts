import type { RectangleBounds } from "@terra-globe/core";

/**
 * Main<->hidden-worker-window IPC protocol for desktop area export (see useAreaExport.ts and
 * ExportWorkerApp.tsx). Tauri events are JSON-serialized, so binary PNG bytes cross as base64
 * (see blobToBase64/base64ToBlob below) rather than as a Blob/ArrayBuffer directly.
 */

/** Emitted by the worker window once its Cesium viewer and place library have finished loading. */
export const EXPORT_WORKER_READY_EVENT = "area-export-worker:ready";
/** Emitted by the main window to ask the worker to render an export. */
export const EXPORT_WORKER_REQUEST_EVENT = "area-export-worker:request";
/** Emitted by the worker window per rendered tile. */
export const EXPORT_WORKER_PROGRESS_EVENT = "area-export-worker:progress";
/** Emitted by the worker window once the export finishes (success or failure). */
export const EXPORT_WORKER_RESULT_EVENT = "area-export-worker:result";

export interface ExportWorkerRequest {
  requestId: string;
  bounds: RectangleBounds;
  scaleDenominator: number;
  dpiValue: number;
}

export interface ExportWorkerProgress {
  requestId: string;
  done: number;
  total: number;
}

/** Carries ExportTooLargeError's structured fields across the JSON event boundary (instanceof doesn't survive serialization). */
export interface ExportWorkerTooLarge {
  pixelWidth: number;
  pixelHeight: number;
  maxDimensionPx: number;
  maxMegapixels: number;
}

export type ExportWorkerResult =
  | { requestId: string; pngBase64: string }
  | { requestId: string; error: string; tooLarge?: ExportWorkerTooLarge };

/** Encodes a Blob's bytes as a plain base64 string (no `data:` URI prefix). */
export async function blobToBase64(blob: Blob): Promise<string> {
  const bytes = new Uint8Array(await blob.arrayBuffer());
  let binary = "";
  for (const byte of bytes) binary += String.fromCharCode(byte);
  return btoa(binary);
}

/** Reverses blobToBase64: decodes a base64 string back into a Blob with the given MIME type. */
export function base64ToBlob(base64: string, mimeType: string): Blob {
  const binary = atob(base64);
  const bytes = new Uint8Array(binary.length);
  for (let i = 0; i < binary.length; i++) bytes[i] = binary.charCodeAt(i);
  return new Blob([bytes], { type: mimeType });
}
