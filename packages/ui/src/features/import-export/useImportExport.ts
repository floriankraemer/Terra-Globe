import { useTranslation } from "react-i18next";
import {
  parseKml,
  parseKmz,
  resolveNetworkLinks,
  serializeKml,
  serializeKmz,
} from "@terra-globe/core";
import type { UseLibraryResult } from "../folders/useLibrary.js";
import { downloadBlob } from "../../lib/downloadBlob.js";

export interface ImportSummary {
  placemarksImported: number;
  foldersImported: number;
  warnings: string[];
}

/** File I/O glue: reads/writes via the browser File/Blob APIs (works in both the browser tab and the Tauri webview). */
export function useImportExport(
  library: UseLibraryResult,
  wrap: <T>(action: () => Promise<T>) => Promise<T> = (action) => action(),
) {
  const { t } = useTranslation();
  return {
    async importFile(file: File): Promise<ImportSummary> {
      const isKmz = file.name.toLowerCase().endsWith(".kmz");
      const isKml = file.name.toLowerCase().endsWith(".kml");
      if (!isKmz && !isKml) {
        throw new Error(t("importExport.notKmlOrKmz", { fileName: file.name }));
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
          t("importExport.readError", {
            fileName: file.name,
            error: err instanceof Error ? err.message : String(err),
          }),
        );
      }

      if (result.folders.length === 0 && result.placemarks.length === 0) {
        throw new Error(
          result.warnings.length > 0
            ? t("importExport.nothingImportable", {
                fileName: file.name,
                warnings: result.warnings.join(" "),
              })
            : t("importExport.noFoldersOrPlacemarks", { fileName: file.name }),
        );
      }

      await wrap(() => library.importPlaces(result));
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
        "terra-globe-export.kml",
        "application/vnd.google-earth.kml+xml",
      );
    },
    async exportKmz(): Promise<void> {
      const data = await library.exportAll();
      const bytes = await serializeKmz(data);
      downloadBlob(
        bytes as unknown as BlobPart,
        "terra-globe-export.kmz",
        "application/vnd.google-earth.kmz",
      );
    },
  };
}
