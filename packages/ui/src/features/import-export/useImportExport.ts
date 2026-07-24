import { parseKml, parseKmz, resolveNetworkLinks, serializeKml, serializeKmz } from "@webglobe/core";
import type { UseLibraryResult } from "../folders/useLibrary.js";

export interface ImportSummary {
  placemarksImported: number;
  foldersImported: number;
  warnings: string[];
}

function downloadBlob(bytes: BlobPart, filename: string, mimeType: string): void {
  const blob = new Blob([bytes], { type: mimeType });
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  link.click();
  URL.revokeObjectURL(url);
}

/** File I/O glue: reads/writes via the browser File/Blob APIs (works in both the browser tab and the Tauri webview). */
export function useImportExport(library: UseLibraryResult) {
  return {
    async importFile(file: File): Promise<ImportSummary> {
      const isKmz = file.name.toLowerCase().endsWith(".kmz");
      const isKml = file.name.toLowerCase().endsWith(".kml");
      if (!isKmz && !isKml) {
        throw new Error(`"${file.name}" is not a .kml or .kmz file.`);
      }

      let result;
      try {
        result = isKmz
          ? await parseKmz(new Uint8Array(await file.arrayBuffer()))
          : parseKml(await file.text());
        if (result.networkLinks.length > 0) {
          result = await resolveNetworkLinks(result);
        }
      } catch (err) {
        throw new Error(
          `Could not read "${file.name}": ${err instanceof Error ? err.message : String(err)}`,
        );
      }

      if (result.folders.length === 0 && result.placemarks.length === 0) {
        throw new Error(
          result.warnings.length > 0
            ? `"${file.name}" had nothing importable. ${result.warnings.join(" ")}`
            : `"${file.name}" contained no folders or placemarks.`,
        );
      }

      await library.importPlaces(result);
      return {
        placemarksImported: result.placemarks.length,
        foldersImported: result.folders.length,
        warnings: result.warnings,
      };
    },
    async exportKml(): Promise<void> {
      const data = await library.exportAll();
      downloadBlob(
        serializeKml(data),
        "webglobe-export.kml",
        "application/vnd.google-earth.kml+xml",
      );
    },
    async exportKmz(): Promise<void> {
      const data = await library.exportAll();
      const bytes = await serializeKmz(data);
      downloadBlob(
        bytes as unknown as BlobPart,
        "webglobe-export.kmz",
        "application/vnd.google-earth.kmz",
      );
    },
  };
}
