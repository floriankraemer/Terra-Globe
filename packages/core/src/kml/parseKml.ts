import { XMLParser } from "fast-xml-parser";
import { createFolder, type Folder } from "../domain/folder.js";
import type { GeoPoint } from "../domain/geometry.js";
import { createPlacemark, type Placemark } from "../domain/placemark.js";
import type { Style } from "../domain/style.js";
import { kmlToDomainGeometry, type KmlGeometryElement } from "./mapping/geometryKmlMapping.js";
import { kmlDataToStyle } from "./mapping/styleKmlMapping.js";

export interface ParseKmlResult {
  folders: Folder[];
  placemarks: Placemark[];
  styles: Style[];
  /** Human-readable notes about placemarks that were skipped or downgraded during import. */
  warnings: string[];
}

const FORCE_ARRAY = new Set([
  "Folder",
  "Placemark",
  "Style",
  "Data",
  "innerBoundaryIs",
  "Point",
  "Polygon",
  "LineString",
]);

const parser = new XMLParser({
  ignoreAttributes: false,
  attributeNamePrefix: "@_",
  isArray: (name) => FORCE_ARRAY.has(name),
});

function asArray<T>(value: T | T[] | undefined): T[] {
  if (value === undefined) return [];
  return Array.isArray(value) ? value : [value];
}

function parseCoordinates(text: string): GeoPoint[] {
  return text
    .trim()
    .split(/\s+/)
    .map((triple) => {
      const [lon, lat, altitude] = triple.split(",").map(Number);
      const point: GeoPoint = { lon: lon!, lat: lat! };
      if (altitude !== undefined && !Number.isNaN(altitude)) point.altitude = altitude;
      return point;
    });
}

function parseExtendedData(node: Record<string, unknown>): Record<string, string> {
  const entries = asArray(node.ExtendedData as { Data?: unknown } | undefined)
    .flatMap((ext) =>
      asArray((ext as { Data?: unknown }).Data as Record<string, unknown>[] | undefined),
    )
    .map((d) => [String(d["@_name"]), String(d.value ?? "")] as const);
  return Object.fromEntries(entries);
}

interface ParsedGeometryEntry {
  element: KmlGeometryElement;
  rings: GeoPoint[][];
}

function parsePolygonNode(polygon: {
  outerBoundaryIs: { LinearRing: { coordinates: string } };
  innerBoundaryIs?: { LinearRing: { coordinates: string } }[];
}): ParsedGeometryEntry {
  const outer = parseCoordinates(polygon.outerBoundaryIs.LinearRing.coordinates);
  const inner = asArray(polygon.innerBoundaryIs).map((ib) =>
    parseCoordinates(ib.LinearRing.coordinates),
  );
  return { element: "Polygon", rings: [outer, ...inner] };
}

/**
 * Returns every geometry this placemark node resolves to (usually one, but
 * a MultiGeometry can contain several). Unsupported geometry types
 * (LineString, GroundOverlay, gx:Track, ...) contribute nothing - the caller
 * is responsible for warning when the result is empty, not throwing.
 */
function parseGeometries(node: Record<string, unknown>): ParsedGeometryEntry[] {
  const results: ParsedGeometryEntry[] = [];

  for (const point of asArray(node.Point as { coordinates: string }[] | undefined)) {
    results.push({ element: "Point", rings: [parseCoordinates(point.coordinates)] });
  }
  for (const polygon of asArray(
    node.Polygon as
      | {
          outerBoundaryIs: { LinearRing: { coordinates: string } };
          innerBoundaryIs?: { LinearRing: { coordinates: string } }[];
        }[]
      | undefined,
  )) {
    results.push(parsePolygonNode(polygon));
  }
  if (node.LinearRing) {
    const ring = node.LinearRing as { coordinates: string };
    results.push({ element: "Polygon", rings: [parseCoordinates(ring.coordinates)] });
  }
  for (const lineString of asArray(node.LineString as { coordinates: string }[] | undefined)) {
    results.push({ element: "LineString", rings: [parseCoordinates(lineString.coordinates)] });
  }
  if (node.MultiGeometry) {
    results.push(...parseGeometries(node.MultiGeometry as Record<string, unknown>));
  }

  return results;
}

function parsePlacemarks(
  node: Record<string, unknown>,
  folderId: string | null,
  warnings: string[],
): Placemark[] {
  const name = String(node.name ?? "");
  let geometries: ParsedGeometryEntry[];
  try {
    geometries = parseGeometries(node);
  } catch {
    geometries = [];
  }

  if (geometries.length === 0) {
    warnings.push(`Skipped placemark "${name}": no supported geometry (Point/Polygon) found.`);
    return [];
  }

  const extendedData = parseExtendedData(node);
  const styleUrl = node.styleUrl as string | undefined;
  const placemarks: Placemark[] = [];

  geometries.forEach(({ element, rings }, index) => {
    const entryName = geometries.length > 1 && index > 0 ? `${name} (${index + 1})` : name;
    try {
      const geometry = kmlToDomainGeometry(element, rings, extendedData);
      const placemark = createPlacemark({
        name: entryName,
        description: node.description !== undefined ? String(node.description) : undefined,
        folderId,
        geometry,
        styleId: styleUrl ? styleUrl.replace(/^#/, "") : null,
        visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
      });
      placemarks.push({
        ...placemark,
        id:
          geometries.length > 1
            ? `${String(node["@_id"] ?? placemark.id)}-${index}`
            : String(node["@_id"] ?? placemark.id),
        createdAt: extendedData["webglobe:createdAt"] ?? placemark.createdAt,
        updatedAt: extendedData["webglobe:updatedAt"] ?? placemark.updatedAt,
      });
    } catch (err) {
      warnings.push(
        `Skipped placemark "${entryName}": ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  });

  return placemarks;
}

function parseFolder(
  node: Record<string, unknown>,
  parentId: string | null,
  folders: Folder[],
  placemarks: Placemark[],
  warnings: string[],
): void {
  const extendedData = parseExtendedData(node);
  const folder = createFolder({
    name: String(node.name ?? ""),
    parentId,
    order:
      extendedData["webglobe:order"] !== undefined ? Number(extendedData["webglobe:order"]) : 0,
    visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
  });
  const resolved: Folder = {
    ...folder,
    id: String(node["@_id"] ?? folder.id),
    createdAt: extendedData["webglobe:createdAt"] ?? folder.createdAt,
    updatedAt: extendedData["webglobe:updatedAt"] ?? folder.updatedAt,
  };
  folders.push(resolved);

  for (const child of asArray(node.Folder as Record<string, unknown>[] | undefined)) {
    parseFolder(child, resolved.id, folders, placemarks, warnings);
  }
  for (const child of asArray(node.Placemark as Record<string, unknown>[] | undefined)) {
    placemarks.push(...parsePlacemarks(child, resolved.id, warnings));
  }
}

export function parseKml(xml: string): ParseKmlResult {
  let parsed: { kml?: { Document?: Record<string, unknown> } & Record<string, unknown> };
  try {
    parsed = parser.parse(xml) as typeof parsed;
  } catch (err) {
    throw new Error(`Failed to parse KML: ${err instanceof Error ? err.message : String(err)}`);
  }
  if (!parsed.kml) {
    throw new Error("Failed to parse KML: no root <kml> element found.");
  }
  const container = parsed.kml.Document ?? parsed.kml;

  const folders: Folder[] = [];
  const placemarks: Placemark[] = [];
  const warnings: string[] = [];

  const styles: Style[] = asArray(container.Style as Record<string, unknown>[] | undefined).map(
    (styleNode) => {
      const lineStyle = styleNode.LineStyle as { color?: string; width?: number } | undefined;
      const polyStyle = styleNode.PolyStyle as
        { color?: string; fill?: number; outline?: number } | undefined;
      const iconStyle = styleNode.IconStyle as
        { scale?: number; Icon?: { href?: string } } | undefined;
      const labelStyle = styleNode.LabelStyle as { color?: string } | undefined;
      // KML spec default when <fill>/<outline> are absent is 1 (true) for both.
      return kmlDataToStyle({
        id: String(styleNode["@_id"]),
        lineColorKml: lineStyle?.color ?? "ff000000",
        lineWidth: lineStyle?.width !== undefined ? Number(lineStyle.width) : 1,
        polyColorKml: polyStyle?.color ?? "ff000000",
        outlineEnabled: polyStyle?.outline === undefined ? true : Number(polyStyle.outline) !== 0,
        filled: polyStyle?.fill === undefined ? true : Number(polyStyle.fill) !== 0,
        iconUrl: iconStyle?.Icon?.href,
        iconScale: iconStyle?.scale !== undefined ? Number(iconStyle.scale) : undefined,
        labelColorKml: labelStyle?.color,
      });
    },
  );

  for (const folderNode of asArray(container.Folder as Record<string, unknown>[] | undefined)) {
    parseFolder(folderNode, null, folders, placemarks, warnings);
  }
  for (const placemarkNode of asArray(
    container.Placemark as Record<string, unknown>[] | undefined,
  )) {
    placemarks.push(...parsePlacemarks(placemarkNode, null, warnings));
  }

  return { folders, placemarks, styles, warnings };
}
