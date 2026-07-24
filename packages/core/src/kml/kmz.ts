import { strFromU8, strToU8, unzipSync, zipSync } from "fflate";
import type { Placemark } from "../domain/placemark.js";
import type { Style } from "../domain/style.js";
import {
  bytesToDataUrl,
  decodeDataUrl,
  extensionForMimeType,
  normalizeAssetPath,
} from "./kmzAssets.js";
import { parseKml, type ParseKmlResult } from "./parseKml.js";
import { serializeKml, type SerializeKmlInput } from "./serializeKml.js";

const DOC_KML_ENTRY = "doc.kml";

function isLocalAssetRef(href: string): boolean {
  return !/^(https?:|data:)/i.test(href);
}

/**
 * Rewrites style/placemark hrefs that point at an entry inside the KMZ
 * archive (custom icons, GroundOverlay images, Model links) into self-
 * contained data: URLs, using the sibling archive entries collected during
 * unzip. Anything already absolute (http(s)/data) is left untouched.
 */
function resolveKmzAssetRefs(
  result: ParseKmlResult,
  assets: Record<string, Uint8Array>,
): ParseKmlResult {
  const resolve = (href: string): string => {
    if (!isLocalAssetRef(href)) return href;
    const path = normalizeAssetPath(href);
    const bytes = assets[path];
    return bytes ? bytesToDataUrl(bytes, path) : href;
  };

  const styles: Style[] = result.styles.map((style) =>
    style.iconUrl ? { ...style, iconUrl: resolve(style.iconUrl) } : style,
  );

  const placemarks: Placemark[] = result.placemarks.map((placemark) => {
    if (placemark.geometry.type === "GroundOverlay") {
      return {
        ...placemark,
        geometry: { ...placemark.geometry, imageUrl: resolve(placemark.geometry.imageUrl) },
      };
    }
    if (placemark.geometry.type === "Model") {
      return {
        ...placemark,
        geometry: { ...placemark.geometry, modelUri: resolve(placemark.geometry.modelUri) },
      };
    }
    return placemark;
  });

  const screenOverlays = result.screenOverlays.map((overlay) => ({
    ...overlay,
    imageUrl: resolve(overlay.imageUrl),
  }));

  return { ...result, styles, placemarks, screenOverlays };
}

/**
 * Reverses `resolveKmzAssetRefs`: pulls any data: URL back out into a real
 * archive entry (under `files/`) so the exported KMZ is a normal archive
 * with relative hrefs, not giant inline data URIs - real-world KML tools
 * expect the former.
 */
function extractKmzAssets(input: SerializeKmlInput): {
  input: SerializeKmlInput;
  assets: Record<string, Uint8Array>;
} {
  const assets: Record<string, Uint8Array> = {};
  let counter = 0;

  const extract = (url: string): string => {
    const decoded = decodeDataUrl(url);
    if (!decoded) return url;
    const path = `files/asset-${counter++}.${extensionForMimeType(decoded.mimeType)}`;
    assets[path] = decoded.bytes;
    return path;
  };

  const styles = input.styles.map((style) =>
    style.iconUrl?.startsWith("data:") ? { ...style, iconUrl: extract(style.iconUrl) } : style,
  );

  const placemarks = input.placemarks.map((placemark) => {
    if (placemark.geometry.type === "GroundOverlay" && placemark.geometry.imageUrl.startsWith("data:")) {
      return {
        ...placemark,
        geometry: { ...placemark.geometry, imageUrl: extract(placemark.geometry.imageUrl) },
      };
    }
    if (placemark.geometry.type === "Model" && placemark.geometry.modelUri.startsWith("data:")) {
      return {
        ...placemark,
        geometry: { ...placemark.geometry, modelUri: extract(placemark.geometry.modelUri) },
      };
    }
    return placemark;
  });

  const screenOverlays = (input.screenOverlays ?? []).map((overlay) =>
    overlay.imageUrl.startsWith("data:") ? { ...overlay, imageUrl: extract(overlay.imageUrl) } : overlay,
  );

  return { input: { ...input, styles, placemarks, screenOverlays }, assets };
}

export async function serializeKmz(input: SerializeKmlInput): Promise<Uint8Array> {
  const { input: rewritten, assets } = extractKmzAssets(input);
  const xml = serializeKml(rewritten);
  return zipSync({ [DOC_KML_ENTRY]: strToU8(xml), ...assets });
}

export async function parseKmz(bytes: Uint8Array): Promise<ParseKmlResult> {
  const entries = unzipSync(bytes);
  const docPath = DOC_KML_ENTRY in entries ? DOC_KML_ENTRY : Object.keys(entries).find((p) => /\.kml$/i.test(p));
  if (!docPath) {
    throw new Error(`KMZ archive is missing a ${DOC_KML_ENTRY} entry`);
  }
  const assets: Record<string, Uint8Array> = {};
  for (const [path, bytes] of Object.entries(entries)) {
    if (path !== docPath) assets[path] = bytes;
  }
  const result = parseKml(strFromU8(entries[docPath]!));
  return resolveKmzAssetRefs(result, assets);
}
