import { XMLParser } from "fast-xml-parser";
import { createFolder, type Folder } from "../domain/folder.js";
import {
  createGroundOverlayGeometry,
  createModelGeometry,
  type GeoPoint,
} from "../domain/geometry.js";
import {
  createPlacemark,
  type Placemark,
  type PlacemarkRegion,
  type PlacemarkView,
} from "../domain/placemark.js";
import { createScreenOverlay, type ScreenOverlay } from "../domain/screenOverlay.js";
import type { Style } from "../domain/style.js";
import { kmlToDomainGeometry, type KmlGeometryElement } from "./mapping/geometryKmlMapping.js";
import { kmlDataToStyle } from "./mapping/styleKmlMapping.js";

const RESERVED_EXTENDED_DATA_PREFIX = "terra-globe:";

/** An unresolved `<NetworkLink>` reference found during parsing - see resolveNetworkLinks in networkLink.ts. */
export interface KmlNetworkLinkRef {
  folderId: string | null;
  href: string;
}

export interface ParseKmlResult {
  folders: Folder[];
  placemarks: Placemark[];
  styles: Style[];
  screenOverlays: ScreenOverlay[];
  networkLinks: KmlNetworkLinkRef[];
  /** Human-readable notes about placemarks that were skipped or downgraded during import. */
  warnings: string[];
}

const FORCE_ARRAY = new Set([
  "Folder",
  "Placemark",
  "Style",
  "StyleMap",
  "Pair",
  "Data",
  "SchemaData",
  "SimpleData",
  "innerBoundaryIs",
  "Point",
  "Polygon",
  "LineString",
  "GroundOverlay",
  "ScreenOverlay",
  "PhotoOverlay",
  "NetworkLink",
  "gx:Track",
  "gx:MultiTrack",
]);

// "when"/"gx:coord" only need forcing inside gx:Track - TimeStamp/TimeSpan
// also use <when>/<begin>/<end> and must NOT be force-arrayed (isArray is
// keyed by tag name only, not path, so this is scoped via jPath instead).
const parser = new XMLParser({
  ignoreAttributes: false,
  attributeNamePrefix: "@_",
  isArray: (name, jPath) =>
    FORCE_ARRAY.has(name) ||
    ((name === "when" || name === "gx:coord") && jPath.endsWith(`gx:Track.${name}`)),
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
  const extNodes = asArray(
    node.ExtendedData as { Data?: unknown; SchemaData?: unknown } | undefined,
  );
  const dataEntries = extNodes
    .flatMap((ext) =>
      asArray((ext as { Data?: unknown }).Data as Record<string, unknown>[] | undefined),
    )
    .map((d) => [String(d["@_name"]), String(d.value ?? "")] as const);
  const schemaEntries = extNodes
    .flatMap((ext) =>
      asArray(
        (ext as { SchemaData?: unknown }).SchemaData as Record<string, unknown>[] | undefined,
      ),
    )
    .flatMap((sd) => asArray(sd.SimpleData as Record<string, unknown>[] | undefined))
    .map(
      (sd) =>
        [String(sd["@_name"]), String((sd as { "#text"?: unknown })["#text"] ?? sd ?? "")] as const,
    );
  return Object.fromEntries([...dataEntries, ...schemaEntries]);
}

/** Drops terra-globe:* internal round-trip markers, keeping only genuine third-party ExtendedData. */
function nonReservedExtendedData(
  extendedData: Record<string, string>,
): Record<string, string> | undefined {
  const entries = Object.entries(extendedData).filter(
    ([key]) => !key.startsWith(RESERVED_EXTENDED_DATA_PREFIX),
  );
  return entries.length > 0 ? Object.fromEntries(entries) : undefined;
}

interface KmlAbstractViewNode {
  longitude?: number;
  latitude?: number;
  altitude?: number;
  heading?: number;
  tilt?: number;
  roll?: number;
  range?: number;
  altitudeMode?: string;
}

function parseView(node: Record<string, unknown>): PlacemarkView | undefined {
  const camera = node.Camera as KmlAbstractViewNode | undefined;
  const lookAt = node.LookAt as KmlAbstractViewNode | undefined;
  const source = camera ?? lookAt;
  if (!source) return undefined;
  const params: Record<string, number | string> = {};
  for (const [key, value] of Object.entries(source)) {
    if (value !== undefined) params[key] = typeof value === "number" ? value : String(value);
  }
  return { kind: camera ? "Camera" : "LookAt", params };
}

function flattenToRecord(
  value: unknown,
  prefix: string,
  out: Record<string, number | string>,
): void {
  if (value === null || value === undefined) return;
  if (typeof value === "object" && !Array.isArray(value)) {
    for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
      if (key.startsWith("@_")) continue;
      flattenToRecord(child, prefix ? `${prefix}.${key}` : key, out);
    }
    return;
  }
  out[prefix] = typeof value === "number" ? value : String(value);
}

function parseRegion(node: Record<string, unknown>): PlacemarkRegion | undefined {
  if (!node.Region) return undefined;
  const raw: Record<string, number | string> = {};
  flattenToRecord(node.Region, "", raw);
  return { raw };
}

/** Fields shared across Placemark/GroundOverlay/Model feature parsing. */
interface FeatureExtras {
  snippet?: string;
  address?: string;
  phoneNumber?: string;
  timeStamp?: string;
  timeSpanBegin?: string;
  timeSpanEnd?: string;
  view?: PlacemarkView;
  region?: PlacemarkRegion;
}

function parseFeatureExtras(node: Record<string, unknown>): FeatureExtras {
  const timeStamp = node.TimeStamp as { when?: string } | undefined;
  const timeSpan = node.TimeSpan as { begin?: string; end?: string } | undefined;
  const snippet = node.Snippet as string | { "#text"?: string } | undefined;
  return {
    snippet:
      typeof snippet === "string"
        ? snippet
        : snippet?.["#text"] !== undefined
          ? String(snippet["#text"])
          : undefined,
    address: node.address !== undefined ? String(node.address) : undefined,
    phoneNumber: node.phoneNumber !== undefined ? String(node.phoneNumber) : undefined,
    timeStamp: timeStamp?.when,
    timeSpanBegin: timeSpan?.begin,
    timeSpanEnd: timeSpan?.end,
    view: parseView(node),
    region: parseRegion(node),
  };
}

function withoutUndefined<T extends object>(obj: T): T {
  return Object.fromEntries(Object.entries(obj).filter(([, v]) => v !== undefined)) as T;
}

interface ParsedGeometryEntry {
  element: KmlGeometryElement;
  rings: GeoPoint[][];
  tessellate?: boolean;
  extrudeHeight?: number;
  timestamps?: string[];
}

function stampAltitudeMode(rings: GeoPoint[][], mode: string | undefined): GeoPoint[][] {
  if (mode !== "clampToGround" && mode !== "relativeToGround" && mode !== "absolute") return rings;
  return rings.map((ring) => ring.map((p) => ({ ...p, altitudeMode: mode })));
}

function maxAltitude(rings: GeoPoint[][]): number | undefined {
  const altitudes = rings
    .flat()
    .map((p) => p.altitude)
    .filter((a): a is number => a !== undefined);
  return altitudes.length > 0 ? Math.max(...altitudes) : undefined;
}

function parsePolygonNode(polygon: {
  outerBoundaryIs: { LinearRing: { coordinates: string } };
  innerBoundaryIs?: { LinearRing: { coordinates: string } }[];
  tessellate?: number;
  extrude?: number;
  altitudeMode?: string;
}): ParsedGeometryEntry {
  const outer = parseCoordinates(polygon.outerBoundaryIs.LinearRing.coordinates);
  const inner = asArray(polygon.innerBoundaryIs).map((ib) =>
    parseCoordinates(ib.LinearRing.coordinates),
  );
  const rings = stampAltitudeMode([outer, ...inner], polygon.altitudeMode);
  return {
    element: "Polygon",
    rings,
    tessellate: polygon.tessellate !== undefined && Number(polygon.tessellate) !== 0,
    extrudeHeight:
      polygon.extrude !== undefined && Number(polygon.extrude) !== 0
        ? (maxAltitude(rings) ?? 0)
        : undefined,
  };
}

/**
 * Returns every Point/Polygon/LineString geometry this placemark node
 * resolves to (usually one, but a MultiGeometry can contain several).
 * Model is handled separately by the caller (parsePlacemarks) since it
 * doesn't fit the ring-based shape family. Anything else unsupported
 * contributes nothing - the caller warns when the result is empty.
 */
function parseGeometries(node: Record<string, unknown>): ParsedGeometryEntry[] {
  const results: ParsedGeometryEntry[] = [];

  for (const point of asArray(
    node.Point as { coordinates: string; altitudeMode?: string }[] | undefined,
  )) {
    results.push({
      element: "Point",
      rings: stampAltitudeMode([parseCoordinates(point.coordinates)], point.altitudeMode),
    });
  }
  for (const polygon of asArray(
    node.Polygon as
      | {
          outerBoundaryIs: { LinearRing: { coordinates: string } };
          innerBoundaryIs?: { LinearRing: { coordinates: string } }[];
          tessellate?: number;
          extrude?: number;
          altitudeMode?: string;
        }[]
      | undefined,
  )) {
    results.push(parsePolygonNode(polygon));
  }
  if (node.LinearRing) {
    const ring = node.LinearRing as { coordinates: string };
    results.push({ element: "Polygon", rings: [parseCoordinates(ring.coordinates)] });
  }
  for (const lineString of asArray(
    node.LineString as
      { coordinates: string; tessellate?: number; altitudeMode?: string }[] | undefined,
  )) {
    results.push({
      element: "LineString",
      rings: stampAltitudeMode([parseCoordinates(lineString.coordinates)], lineString.altitudeMode),
      tessellate: lineString.tessellate !== undefined && Number(lineString.tessellate) !== 0,
    });
  }
  if (node.MultiGeometry) {
    results.push(...parseGeometries(node.MultiGeometry as Record<string, unknown>));
  }
  for (const track of asArray(node["gx:Track"] as KmlGxTrackNode[] | undefined)) {
    results.push(parseGxTrack(track));
  }
  for (const multiTrack of asArray(
    node["gx:MultiTrack"] as Record<string, unknown>[] | undefined,
  )) {
    results.push(
      ...asArray(multiTrack["gx:Track"] as KmlGxTrackNode[] | undefined).map(parseGxTrack),
    );
  }

  return results;
}

interface KmlGxTrackNode {
  when?: string | string[];
  "gx:coord"?: string | string[];
  altitudeMode?: string;
}

function parseGxTrack(track: KmlGxTrackNode): ParsedGeometryEntry {
  const whens = asArray(track.when);
  const coords = asArray(track["gx:coord"]).map((c) => {
    const [lon, lat, altitude] = c.trim().split(/\s+/).map(Number);
    const point: GeoPoint = { lon: lon!, lat: lat! };
    if (altitude !== undefined && !Number.isNaN(altitude)) point.altitude = altitude;
    return point;
  });
  const rings = stampAltitudeMode([coords], track.altitudeMode);
  return {
    element: "LineString",
    rings,
    timestamps: whens.length === coords.length && coords.length > 0 ? whens : undefined,
  };
}

interface KmlModelNode {
  Location?: { longitude?: number; latitude?: number; altitude?: number };
  Orientation?: { heading?: number; tilt?: number; roll?: number };
  Scale?: { x?: number };
  Link?: { href?: string };
  href?: string;
}

function parseModelNode(model: KmlModelNode) {
  const modelUri = model.Link?.href ?? model.href;
  if (!modelUri) return undefined;
  return createModelGeometry(
    {
      lon: Number(model.Location?.longitude ?? 0),
      lat: Number(model.Location?.latitude ?? 0),
      altitude:
        model.Location?.altitude !== undefined ? Number(model.Location.altitude) : undefined,
    },
    modelUri,
    {
      scale: model.Scale?.x !== undefined ? Number(model.Scale.x) : undefined,
      heading:
        model.Orientation?.heading !== undefined ? Number(model.Orientation.heading) : undefined,
      tilt: model.Orientation?.tilt !== undefined ? Number(model.Orientation.tilt) : undefined,
      roll: model.Orientation?.roll !== undefined ? Number(model.Orientation.roll) : undefined,
    },
  );
}

interface KmlLatLonBoxNode {
  north: number;
  south: number;
  east: number;
  west: number;
  rotation?: number;
}

function parseGroundOverlay(
  node: Record<string, unknown>,
  folderId: string | null,
  warnings: string[],
): Placemark[] {
  const name = String(node.name ?? "");
  const box = node.LatLonBox as KmlLatLonBoxNode | undefined;
  const icon = node.Icon as { href?: string } | undefined;
  if (!box || !icon?.href) {
    warnings.push(`Skipped GroundOverlay "${name}": missing LatLonBox or Icon href.`);
    return [];
  }
  const extendedData = parseExtendedData(node);
  try {
    const geometry = createGroundOverlayGeometry(
      {
        north: Number(box.north),
        south: Number(box.south),
        east: Number(box.east),
        west: Number(box.west),
      },
      icon.href,
      box.rotation !== undefined ? Number(box.rotation) : undefined,
    );
    const placemark = createPlacemark({
      name,
      description: node.description !== undefined ? String(node.description) : undefined,
      folderId,
      geometry,
      visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
      extendedData: nonReservedExtendedData(extendedData),
      ...withoutUndefined(parseFeatureExtras(node)),
    });
    return [
      {
        ...placemark,
        id: String(node["@_id"] ?? placemark.id),
        createdAt: extendedData["terra-globe:createdAt"] ?? placemark.createdAt,
        updatedAt: extendedData["terra-globe:updatedAt"] ?? placemark.updatedAt,
      },
    ];
  } catch (err) {
    warnings.push(
      `Skipped GroundOverlay "${name}": ${err instanceof Error ? err.message : String(err)}`,
    );
    return [];
  }
}

/**
 * PhotoOverlay's cylindrical/spherical photo-viewing behavior needs a new
 * viewer subsystem this app doesn't have. When it carries a LatLonBox
 * (common for simple rectangular photo overlays) it's imported exactly like
 * a GroundOverlay; otherwise it falls back to a Point placemark with the
 * photo URL preserved so nothing is silently lost.
 */
function parsePhotoOverlay(
  node: Record<string, unknown>,
  folderId: string | null,
  warnings: string[],
): Placemark[] {
  if (node.LatLonBox) {
    return parseGroundOverlay(node, folderId, warnings);
  }
  const name = String(node.name ?? "");
  const icon = node.Icon as { href?: string } | undefined;
  const point = asArray(node.Point as { coordinates: string }[] | undefined)[0];
  if (!icon?.href || !point) {
    warnings.push(
      `Skipped PhotoOverlay "${name}": no LatLonBox and no Point/Icon to fall back to.`,
    );
    return [];
  }
  const extendedData = parseExtendedData(node);
  try {
    const geometry = kmlToDomainGeometry("Point", [parseCoordinates(point.coordinates)], {});
    const placemark = createPlacemark({
      name,
      description: node.description !== undefined ? String(node.description) : undefined,
      folderId,
      geometry,
      visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
      extendedData: { ...nonReservedExtendedData(extendedData), "terra-globe:photoUrl": icon.href },
      ...withoutUndefined(parseFeatureExtras(node)),
    });
    return [{ ...placemark, id: String(node["@_id"] ?? placemark.id) }];
  } catch (err) {
    warnings.push(
      `Skipped PhotoOverlay "${name}": ${err instanceof Error ? err.message : String(err)}`,
    );
    return [];
  }
}

interface KmlVec2Node {
  "@_x"?: number;
  "@_y"?: number;
  "@_xunits"?: string;
  "@_yunits"?: string;
}

function parseAnchor(node: KmlVec2Node | undefined, defaults: { x: number; y: number }) {
  const validUnits = new Set(["fraction", "pixels", "insetPixels"]);
  const xUnits = validUnits.has(String(node?.["@_xunits"]))
    ? (node!["@_xunits"] as "fraction")
    : "fraction";
  const yUnits = validUnits.has(String(node?.["@_yunits"]))
    ? (node!["@_yunits"] as "fraction")
    : "fraction";
  return {
    x: node?.["@_x"] !== undefined ? Number(node["@_x"]) : defaults.x,
    y: node?.["@_y"] !== undefined ? Number(node["@_y"]) : defaults.y,
    xUnits,
    yUnits,
  };
}

function parseScreenOverlay(
  node: Record<string, unknown>,
  folderId: string | null,
  warnings: string[],
): ScreenOverlay[] {
  const name = String(node.name ?? "");
  const icon = node.Icon as { href?: string } | undefined;
  if (!icon?.href) {
    warnings.push(`Skipped ScreenOverlay "${name}": missing Icon href.`);
    return [];
  }
  const size = node.size as KmlVec2Node | undefined;
  try {
    return [
      createScreenOverlay({
        name,
        folderId,
        imageUrl: icon.href,
        overlayXY: parseAnchor(node.overlayXY as KmlVec2Node | undefined, { x: 0, y: 1 }),
        screenXY: parseAnchor(node.screenXY as KmlVec2Node | undefined, { x: 0, y: 1 }),
        sizeX: size?.["@_x"] !== undefined ? Number(size["@_x"]) : undefined,
        sizeY: size?.["@_y"] !== undefined ? Number(size["@_y"]) : undefined,
        rotation: node.rotation !== undefined ? Number(node.rotation) : undefined,
        visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
      }),
    ];
  } catch (err) {
    warnings.push(
      `Skipped ScreenOverlay "${name}": ${err instanceof Error ? err.message : String(err)}`,
    );
    return [];
  }
}

function parsePlacemarks(
  node: Record<string, unknown>,
  folderId: string | null,
  warnings: string[],
): Placemark[] {
  const name = String(node.name ?? "");

  if (node.Model) {
    const extendedData = parseExtendedData(node);
    const geometry = parseModelNode(node.Model as KmlModelNode);
    if (!geometry) {
      warnings.push(`Skipped placemark "${name}": Model has no Link href.`);
      return [];
    }
    const styleUrl = node.styleUrl as string | undefined;
    const placemark = createPlacemark({
      name,
      description: node.description !== undefined ? String(node.description) : undefined,
      folderId,
      geometry,
      styleId: styleUrl ? styleUrl.replace(/^#/, "") : null,
      visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
      extendedData: nonReservedExtendedData(extendedData),
      ...withoutUndefined(parseFeatureExtras(node)),
    });
    return [
      {
        ...placemark,
        id: String(node["@_id"] ?? placemark.id),
        createdAt: extendedData["terra-globe:createdAt"] ?? placemark.createdAt,
        updatedAt: extendedData["terra-globe:updatedAt"] ?? placemark.updatedAt,
      },
    ];
  }

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
  const multiGeometryGroup =
    geometries.length > 1 ? String(node["@_id"] ?? `multigeom-${name}`) : undefined;
  const featureExtras = withoutUndefined(parseFeatureExtras(node));
  const arbitraryExtendedData = nonReservedExtendedData(extendedData);

  geometries.forEach(({ element, rings, tessellate, extrudeHeight, timestamps }, index) => {
    const entryName = geometries.length > 1 && index > 0 ? `${name} (${index + 1})` : name;
    try {
      const geometry = kmlToDomainGeometry(element, rings, extendedData);
      if (geometry.type === "LineString") {
        if (tessellate) geometry.tessellate = true;
        if (timestamps) geometry.timestamps = timestamps;
      }
      if (geometry.type === "Polygon") {
        if (tessellate) geometry.tessellate = true;
        if (extrudeHeight !== undefined) geometry.extrudeHeight = extrudeHeight;
      }
      const placemark = createPlacemark({
        name: entryName,
        description: node.description !== undefined ? String(node.description) : undefined,
        folderId,
        geometry,
        styleId: styleUrl ? styleUrl.replace(/^#/, "") : null,
        visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
        extendedData: arbitraryExtendedData,
        multiGeometryGroup,
        ...featureExtras,
      });
      placemarks.push({
        ...placemark,
        id:
          geometries.length > 1
            ? `${String(node["@_id"] ?? placemark.id)}-${index}`
            : String(node["@_id"] ?? placemark.id),
        createdAt: extendedData["terra-globe:createdAt"] ?? placemark.createdAt,
        updatedAt: extendedData["terra-globe:updatedAt"] ?? placemark.updatedAt,
      });
    } catch (err) {
      warnings.push(
        `Skipped placemark "${entryName}": ${err instanceof Error ? err.message : String(err)}`,
      );
    }
  });

  return placemarks;
}

function parseNetworkLinkRefs(
  nodes: Record<string, unknown>[],
  folderId: string | null,
): KmlNetworkLinkRef[] {
  return nodes.flatMap((node) => {
    const link = node.Link as { href?: string } | undefined;
    const url = node.Url as { href?: string } | undefined;
    const href = link?.href ?? url?.href;
    return href ? [{ folderId, href }] : [];
  });
}

function parseFolder(
  node: Record<string, unknown>,
  parentId: string | null,
  folders: Folder[],
  placemarks: Placemark[],
  screenOverlays: ScreenOverlay[],
  networkLinks: KmlNetworkLinkRef[],
  warnings: string[],
): void {
  const extendedData = parseExtendedData(node);
  const folder = createFolder({
    name: String(node.name ?? ""),
    parentId,
    order:
      extendedData["terra-globe:order"] !== undefined ? Number(extendedData["terra-globe:order"]) : 0,
    visibility: node.visibility === undefined ? true : String(node.visibility) === "1",
    description: node.description !== undefined ? String(node.description) : undefined,
    open: node.open !== undefined ? String(node.open) === "1" : undefined,
  });
  const resolved: Folder = {
    ...folder,
    id: String(node["@_id"] ?? folder.id),
    createdAt: extendedData["terra-globe:createdAt"] ?? folder.createdAt,
    updatedAt: extendedData["terra-globe:updatedAt"] ?? folder.updatedAt,
  };
  folders.push(resolved);

  for (const child of asArray(node.Folder as Record<string, unknown>[] | undefined)) {
    parseFolder(child, resolved.id, folders, placemarks, screenOverlays, networkLinks, warnings);
  }
  for (const child of asArray(node.Placemark as Record<string, unknown>[] | undefined)) {
    placemarks.push(...parsePlacemarks(child, resolved.id, warnings));
  }
  for (const child of asArray(node.GroundOverlay as Record<string, unknown>[] | undefined)) {
    placemarks.push(...parseGroundOverlay(child, resolved.id, warnings));
  }
  for (const child of asArray(node.PhotoOverlay as Record<string, unknown>[] | undefined)) {
    placemarks.push(...parsePhotoOverlay(child, resolved.id, warnings));
  }
  for (const child of asArray(node.ScreenOverlay as Record<string, unknown>[] | undefined)) {
    screenOverlays.push(...parseScreenOverlay(child, resolved.id, warnings));
  }
  networkLinks.push(
    ...parseNetworkLinkRefs(
      asArray(node.NetworkLink as Record<string, unknown>[] | undefined),
      resolved.id,
    ),
  );
}

function parseStyleNode(styleNode: Record<string, unknown>, id: string): Style {
  const lineStyle = styleNode.LineStyle as { color?: string; width?: number } | undefined;
  const polyStyle = styleNode.PolyStyle as
    { color?: string; fill?: number; outline?: number } | undefined;
  const iconStyle = styleNode.IconStyle as { scale?: number; Icon?: { href?: string } } | undefined;
  const labelStyle = styleNode.LabelStyle as { color?: string } | undefined;
  const balloonStyle = styleNode.BalloonStyle as
    { text?: string; bgColor?: string; textColor?: string; displayMode?: string } | undefined;
  const listStyle = styleNode.ListStyle as
    { listItemType?: string; ItemIcon?: { href?: string } } | undefined;
  // KML spec default when <fill>/<outline> are absent is 1 (true) for both.
  return kmlDataToStyle({
    id,
    lineColorKml: lineStyle?.color ?? "ff000000",
    lineWidth: lineStyle?.width !== undefined ? Number(lineStyle.width) : 1,
    polyColorKml: polyStyle?.color ?? "ff000000",
    outlineEnabled: polyStyle?.outline === undefined ? true : Number(polyStyle.outline) !== 0,
    filled: polyStyle?.fill === undefined ? true : Number(polyStyle.fill) !== 0,
    iconUrl: iconStyle?.Icon?.href,
    iconScale: iconStyle?.scale !== undefined ? Number(iconStyle.scale) : undefined,
    labelColorKml: labelStyle?.color,
    balloonText: balloonStyle?.text,
    balloonBgColorKml: balloonStyle?.bgColor,
    balloonTextColorKml: balloonStyle?.textColor,
    balloonDisplayMode: balloonStyle?.displayMode === "hide" ? "hide" : undefined,
    listItemType: listStyle?.listItemType,
    itemIconHref: listStyle?.ItemIcon?.href,
  });
}

interface KmlStyleMapPair {
  key?: string;
  styleUrl?: string;
}

function parseStyleMaps(
  styleMapNodes: Record<string, unknown>[],
  stylesById: Map<string, Style>,
): Style[] {
  return styleMapNodes.flatMap((node) => {
    const id = String(node["@_id"]);
    const pairs = asArray(node.Pair as KmlStyleMapPair[] | undefined);
    const normalRef = pairs.find((p) => p.key === "normal")?.styleUrl?.replace(/^#/, "");
    const highlightRef = pairs.find((p) => p.key === "highlight")?.styleUrl?.replace(/^#/, "");
    const normal = normalRef ? stylesById.get(normalRef) : undefined;
    const highlightStyle = highlightRef ? stylesById.get(highlightRef) : undefined;
    if (!normal) return [];
    return [
      {
        ...normal,
        id,
        highlight: highlightStyle
          ? {
              outlineColor: highlightStyle.outlineColor,
              outlineWidth: highlightStyle.outlineWidth,
              fillColor: highlightStyle.fillColor,
              fillOpacity: highlightStyle.fillOpacity,
              iconUrl: highlightStyle.iconUrl,
              iconScale: highlightStyle.iconScale,
              labelColor: highlightStyle.labelColor,
            }
          : undefined,
      },
    ];
  });
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
  const screenOverlays: ScreenOverlay[] = [];
  const networkLinks: KmlNetworkLinkRef[] = [];
  const warnings: string[] = [];

  const baseStyles: Style[] = asArray(container.Style as Record<string, unknown>[] | undefined).map(
    (styleNode) => parseStyleNode(styleNode, String(styleNode["@_id"])),
  );
  const stylesById = new Map(baseStyles.map((s) => [s.id, s]));
  const styleMapStyles = parseStyleMaps(
    asArray(container.StyleMap as Record<string, unknown>[] | undefined),
    stylesById,
  );
  const styles: Style[] = [...baseStyles, ...styleMapStyles];

  for (const folderNode of asArray(container.Folder as Record<string, unknown>[] | undefined)) {
    parseFolder(folderNode, null, folders, placemarks, screenOverlays, networkLinks, warnings);
  }
  for (const placemarkNode of asArray(
    container.Placemark as Record<string, unknown>[] | undefined,
  )) {
    placemarks.push(...parsePlacemarks(placemarkNode, null, warnings));
  }
  for (const overlayNode of asArray(
    container.GroundOverlay as Record<string, unknown>[] | undefined,
  )) {
    placemarks.push(...parseGroundOverlay(overlayNode, null, warnings));
  }
  for (const overlayNode of asArray(
    container.PhotoOverlay as Record<string, unknown>[] | undefined,
  )) {
    placemarks.push(...parsePhotoOverlay(overlayNode, null, warnings));
  }
  for (const overlayNode of asArray(
    container.ScreenOverlay as Record<string, unknown>[] | undefined,
  )) {
    screenOverlays.push(...parseScreenOverlay(overlayNode, null, warnings));
  }
  networkLinks.push(
    ...parseNetworkLinkRefs(
      asArray(container.NetworkLink as Record<string, unknown>[] | undefined),
      null,
    ),
  );

  return { folders, placemarks, styles, screenOverlays, networkLinks, warnings };
}
