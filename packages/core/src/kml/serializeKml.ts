import type { Folder } from "../domain/folder.js";
import type { AltitudeMode, GeoPoint } from "../domain/geometry.js";
import type { Placemark, PlacemarkRegion, PlacemarkView } from "../domain/placemark.js";
import type { ScreenOverlay, ScreenOverlayAnchor } from "../domain/screenOverlay.js";
import type { Style } from "../domain/style.js";
import { domainGeometryToKml } from "./mapping/geometryKmlMapping.js";
import { styleToKmlData } from "./mapping/styleKmlMapping.js";
import { escapeXml } from "./xmlEscape.js";

function renderAnchor(tag: string, anchor: ScreenOverlayAnchor): string {
  return `<${tag} x="${anchor.x}" y="${anchor.y}" xunits="${anchor.xUnits}" yunits="${anchor.yUnits}"/>`;
}

function renderScreenOverlay(overlay: ScreenOverlay): string {
  const size =
    overlay.sizeX !== undefined || overlay.sizeY !== undefined
      ? `<size x="${overlay.sizeX ?? -1}" y="${overlay.sizeY ?? -1}" xunits="pixels" yunits="pixels"/>`
      : "";
  const rotation = overlay.rotation !== undefined ? `<rotation>${overlay.rotation}</rotation>` : "";
  return `<ScreenOverlay id="${escapeXml(overlay.id)}"><name>${escapeXml(overlay.name)}</name><visibility>${overlay.visibility ? 1 : 0}</visibility><Icon><href>${escapeXml(overlay.imageUrl)}</href></Icon>${renderAnchor("overlayXY", overlay.overlayXY)}${renderAnchor("screenXY", overlay.screenXY)}${size}${rotation}</ScreenOverlay>`;
}

function altitudeModeTag(mode: AltitudeMode | undefined): string {
  return mode ? `<altitudeMode>${mode}</altitudeMode>` : "";
}

function renderView(view: PlacemarkView | undefined): string {
  if (!view) return "";
  const tags = Object.entries(view.params)
    .map(([key, value]) => `<${key}>${typeof value === "number" ? value : escapeXml(value)}</${key}>`)
    .join("");
  return `<${view.kind}>${tags}</${view.kind}>`;
}

function renderRegion(region: PlacemarkRegion | undefined): string {
  if (!region) return "";
  const box = region.raw;
  const north = box["LatLonAltBox.north"];
  const south = box["LatLonAltBox.south"];
  const east = box["LatLonAltBox.east"];
  const west = box["LatLonAltBox.west"];
  const minLodPixels = box["Lod.minLodPixels"];
  const maxLodPixels = box["Lod.maxLodPixels"];
  const hasBox = north !== undefined || south !== undefined || east !== undefined || west !== undefined;
  const hasLod = minLodPixels !== undefined || maxLodPixels !== undefined;
  const boxXml = hasBox
    ? `<LatLonAltBox>${north !== undefined ? `<north>${north}</north>` : ""}${south !== undefined ? `<south>${south}</south>` : ""}${east !== undefined ? `<east>${east}</east>` : ""}${west !== undefined ? `<west>${west}</west>` : ""}</LatLonAltBox>`
    : "";
  const lodXml = hasLod
    ? `<Lod>${minLodPixels !== undefined ? `<minLodPixels>${minLodPixels}</minLodPixels>` : ""}${maxLodPixels !== undefined ? `<maxLodPixels>${maxLodPixels}</maxLodPixels>` : ""}</Lod>`
    : "";
  return `<Region>${boxXml}${lodXml}</Region>`;
}

function renderTimePrimitive(placemark: Placemark): string {
  if (placemark.timeStamp) return `<TimeStamp><when>${escapeXml(placemark.timeStamp)}</when></TimeStamp>`;
  if (placemark.timeSpanBegin || placemark.timeSpanEnd) {
    return `<TimeSpan>${placemark.timeSpanBegin ? `<begin>${escapeXml(placemark.timeSpanBegin)}</begin>` : ""}${placemark.timeSpanEnd ? `<end>${escapeXml(placemark.timeSpanEnd)}</end>` : ""}</TimeSpan>`;
  }
  return "";
}

function renderFeatureExtras(placemark: Placemark): string {
  const snippet = placemark.snippet ? `<Snippet>${escapeXml(placemark.snippet)}</Snippet>` : "";
  const address = placemark.address ? `<address>${escapeXml(placemark.address)}</address>` : "";
  const phoneNumber = placemark.phoneNumber
    ? `<phoneNumber>${escapeXml(placemark.phoneNumber)}</phoneNumber>`
    : "";
  return `${snippet}${address}${phoneNumber}${renderTimePrimitive(placemark)}${renderView(placemark.view)}${renderRegion(placemark.region)}`;
}

function renderModel(geometry: { position: GeoPoint; modelUri: string; scale?: number; heading?: number; tilt?: number; roll?: number }): string {
  const { position, modelUri, scale, heading, tilt, roll } = geometry;
  const location = `<Location><longitude>${position.lon}</longitude><latitude>${position.lat}</latitude>${position.altitude !== undefined ? `<altitude>${position.altitude}</altitude>` : ""}</Location>`;
  const orientation =
    heading !== undefined || tilt !== undefined || roll !== undefined
      ? `<Orientation><heading>${heading ?? 0}</heading><tilt>${tilt ?? 0}</tilt><roll>${roll ?? 0}</roll></Orientation>`
      : "";
  const scaleXml = scale !== undefined ? `<Scale><x>${scale}</x><y>${scale}</y><z>${scale}</z></Scale>` : "";
  return `<Model>${location}${orientation}${scaleXml}<Link><href>${escapeXml(modelUri)}</href></Link></Model>`;
}

function renderGroundOverlay(placemark: Placemark): string {
  if (placemark.geometry.type !== "GroundOverlay") {
    throw new Error("renderGroundOverlay called with non-GroundOverlay geometry");
  }
  const { bounds, imageUrl, rotation } = placemark.geometry;
  const description = placemark.description
    ? `<description>${escapeXml(placemark.description)}</description>`
    : "";
  const box = `<LatLonBox><north>${bounds.north}</north><south>${bounds.south}</south><east>${bounds.east}</east><west>${bounds.west}</west>${rotation !== undefined ? `<rotation>${rotation}</rotation>` : ""}</LatLonBox>`;
  const extendedData = [
    ...Object.entries(placemark.extendedData ?? {}).map(([name, value]) => ({ name, value })),
    { name: "webglobe:createdAt", value: placemark.createdAt },
    { name: "webglobe:updatedAt", value: placemark.updatedAt },
  ];
  return `<GroundOverlay id="${escapeXml(placemark.id)}"><name>${escapeXml(placemark.name)}</name>${description}${renderFeatureExtras(placemark)}<visibility>${placemark.visibility ? 1 : 0}</visibility><Icon><href>${escapeXml(imageUrl)}</href></Icon>${box}${renderExtendedData(extendedData)}</GroundOverlay>`;
}

export interface SerializeKmlInput {
  folders: Folder[];
  placemarks: Placemark[];
  styles: Style[];
  screenOverlays?: ScreenOverlay[];
}

function renderCoordinates(ring: GeoPoint[]): string {
  return ring
    .map((p) => `${p.lon},${p.lat}${p.altitude !== undefined ? `,${p.altitude}` : ""}`)
    .join(" ");
}

function renderExtendedData(entries: { name: string; value: string }[]): string {
  if (entries.length === 0) return "";
  const data = entries.map(
    (e) => `<Data name="${escapeXml(e.name)}"><value>${escapeXml(e.value)}</value></Data>`,
  );
  return `<ExtendedData>${data.join("")}</ExtendedData>`;
}

function renderGeometry(placemark: Placemark): {
  xml: string;
  extendedData: { name: string; value: string }[];
} {
  if (placemark.geometry.type === "Model") {
    return { xml: renderModel(placemark.geometry), extendedData: [] };
  }
  if (placemark.geometry.type === "GroundOverlay") {
    throw new Error("GroundOverlay placemarks must be rendered via renderGroundOverlay, not renderGeometry");
  }
  const { element, rings, extendedData } = domainGeometryToKml(placemark.geometry);
  if (element === "Point") {
    const altitudeMode =
      placemark.geometry.type === "Point" ? altitudeModeTag(placemark.geometry.coordinates.altitudeMode) : "";
    return {
      xml: `<Point>${altitudeMode}<coordinates>${renderCoordinates(rings[0]!)}</coordinates></Point>`,
      extendedData,
    };
  }
  if (element === "LineString") {
    const tessellate =
      placemark.geometry.type === "LineString" && placemark.geometry.tessellate
        ? "<tessellate>1</tessellate>"
        : "";
    const altitudeMode =
      placemark.geometry.type === "LineString"
        ? altitudeModeTag(placemark.geometry.path[0]?.altitudeMode)
        : "";
    return {
      xml: `<LineString>${tessellate}${altitudeMode}<coordinates>${renderCoordinates(rings[0]!)}</coordinates></LineString>`,
      extendedData,
    };
  }
  const extrude =
    placemark.geometry.type === "Polygon" && placemark.geometry.extrudeHeight !== undefined
      ? "<extrude>1</extrude>"
      : "";
  const tessellate =
    placemark.geometry.type === "Polygon" && placemark.geometry.tessellate ? "<tessellate>1</tessellate>" : "";
  const altitudeMode =
    placemark.geometry.type === "Polygon"
      ? altitudeModeTag(placemark.geometry.outerRing[0]?.altitudeMode)
      : "";
  const [outer, ...inner] = rings;
  const outerXml = `<outerBoundaryIs><LinearRing><coordinates>${renderCoordinates(outer!)}</coordinates></LinearRing></outerBoundaryIs>`;
  const innerXml = inner
    .map(
      (ring) =>
        `<innerBoundaryIs><LinearRing><coordinates>${renderCoordinates(ring)}</coordinates></LinearRing></innerBoundaryIs>`,
    )
    .join("");
  return {
    xml: `<Polygon>${extrude}${tessellate}${altitudeMode}${outerXml}${innerXml}</Polygon>`,
    extendedData,
  };
}

function renderStyleBody(data: ReturnType<typeof styleToKmlData>): string {
  const iconStyle = data.iconUrl
    ? `<IconStyle>${data.iconScale !== undefined ? `<scale>${data.iconScale}</scale>` : ""}<Icon><href>${escapeXml(data.iconUrl)}</href></Icon></IconStyle>`
    : "";
  const labelStyle = data.labelColorKml
    ? `<LabelStyle><color>${data.labelColorKml}</color></LabelStyle>`
    : "";
  const balloonStyle =
    data.balloonText || data.balloonBgColorKml || data.balloonTextColorKml
      ? `<BalloonStyle>${data.balloonText ? `<text>${escapeXml(data.balloonText)}</text>` : ""}${data.balloonBgColorKml ? `<bgColor>${data.balloonBgColorKml}</bgColor>` : ""}${data.balloonTextColorKml ? `<textColor>${data.balloonTextColorKml}</textColor>` : ""}${data.balloonDisplayMode === "hide" ? `<displayMode>hide</displayMode>` : ""}</BalloonStyle>`
      : "";
  const listStyle =
    data.listItemType || data.itemIconHref
      ? `<ListStyle>${data.listItemType ? `<listItemType>${escapeXml(data.listItemType)}</listItemType>` : ""}${data.itemIconHref ? `<ItemIcon><href>${escapeXml(data.itemIconHref)}</href></ItemIcon>` : ""}</ListStyle>`
      : "";
  return `<LineStyle><color>${data.lineColorKml}</color><width>${data.lineWidth}</width></LineStyle><PolyStyle><color>${data.polyColorKml}</color><fill>${data.filled ? 1 : 0}</fill><outline>${data.outlineEnabled ? 1 : 0}</outline></PolyStyle>${iconStyle}${labelStyle}${balloonStyle}${listStyle}`;
}

function renderStyle(style: Style): string {
  const data = styleToKmlData(style);

  if (!data.highlight) {
    return `<Style id="${escapeXml(data.id)}">${renderStyleBody(data)}</Style>`;
  }

  const normalId = `${data.id}-normal`;
  const highlightId = `${data.id}-highlight`;
  const highlightData = {
    ...data,
    id: highlightId,
    lineColorKml: data.highlight.lineColorKml ?? data.lineColorKml,
    lineWidth: data.highlight.lineWidth ?? data.lineWidth,
    polyColorKml: data.highlight.polyColorKml ?? data.polyColorKml,
    iconUrl: data.highlight.iconUrl ?? data.iconUrl,
    iconScale: data.highlight.iconScale ?? data.iconScale,
    labelColorKml: data.highlight.labelColorKml ?? data.labelColorKml,
  };
  const styleMap = `<StyleMap id="${escapeXml(data.id)}"><Pair><key>normal</key><styleUrl>#${escapeXml(normalId)}</styleUrl></Pair><Pair><key>highlight</key><styleUrl>#${escapeXml(highlightId)}</styleUrl></Pair></StyleMap>`;
  const normalStyle = `<Style id="${escapeXml(normalId)}">${renderStyleBody(data)}</Style>`;
  const highlightStyleXml = `<Style id="${escapeXml(highlightId)}">${renderStyleBody(highlightData)}</Style>`;
  return `${styleMap}${normalStyle}${highlightStyleXml}`;
}

function renderPlacemark(placemark: Placemark): string {
  const { xml: geometryXml, extendedData } = renderGeometry(placemark);
  const allExtendedData = [
    ...extendedData,
    ...Object.entries(placemark.extendedData ?? {}).map(([name, value]) => ({ name, value })),
    { name: "webglobe:createdAt", value: placemark.createdAt },
    { name: "webglobe:updatedAt", value: placemark.updatedAt },
    ...(placemark.multiGeometryGroup
      ? [{ name: "webglobe:multiGeometryGroup", value: placemark.multiGeometryGroup }]
      : []),
  ];
  const description = placemark.description
    ? `<description>${escapeXml(placemark.description)}</description>`
    : "";
  const styleUrl = placemark.styleId ? `<styleUrl>#${escapeXml(placemark.styleId)}</styleUrl>` : "";
  return `<Placemark id="${escapeXml(placemark.id)}"><name>${escapeXml(placemark.name)}</name>${description}${renderFeatureExtras(placemark)}<visibility>${placemark.visibility ? 1 : 0}</visibility>${styleUrl}${geometryXml}${renderExtendedData(allExtendedData)}</Placemark>`;
}

function renderFeature(placemark: Placemark): string {
  return placemark.geometry.type === "GroundOverlay"
    ? renderGroundOverlay(placemark)
    : renderPlacemark(placemark);
}

/** Placemarks that came from one KML MultiGeometry (see multiGeometryGroup) render back into one. */
function renderMultiGeometryPlacemark(members: Placemark[]): string {
  const first = members[0]!;
  const parts = members.map((m) => renderGeometry(m));
  const geometryXml = `<MultiGeometry>${parts.map((p) => p.xml).join("")}</MultiGeometry>`;
  const allExtendedData = [
    ...parts.flatMap((p) => p.extendedData),
    ...Object.entries(first.extendedData ?? {}).map(([name, value]) => ({ name, value })),
    { name: "webglobe:createdAt", value: first.createdAt },
    { name: "webglobe:updatedAt", value: first.updatedAt },
  ];
  const description = first.description
    ? `<description>${escapeXml(first.description)}</description>`
    : "";
  const styleUrl = first.styleId ? `<styleUrl>#${escapeXml(first.styleId)}</styleUrl>` : "";
  return `<Placemark id="${escapeXml(first.multiGeometryGroup!)}"><name>${escapeXml(first.name)}</name>${description}${renderFeatureExtras(first)}<visibility>${first.visibility ? 1 : 0}</visibility>${styleUrl}${geometryXml}${renderExtendedData(allExtendedData)}</Placemark>`;
}

/** Groups placemarks that share a `multiGeometryGroup` marker so they render as one Placemark/MultiGeometry. */
function groupForRender(placemarks: Placemark[]): (Placemark | Placemark[])[] {
  const seenGroups = new Set<string>();
  const result: (Placemark | Placemark[])[] = [];
  for (const placemark of placemarks) {
    if (!placemark.multiGeometryGroup) {
      result.push(placemark);
      continue;
    }
    if (seenGroups.has(placemark.multiGeometryGroup)) continue;
    seenGroups.add(placemark.multiGeometryGroup);
    result.push(placemarks.filter((p) => p.multiGeometryGroup === placemark.multiGeometryGroup));
  }
  return result;
}

function renderFeatureOrGroup(item: Placemark | Placemark[]): string {
  return Array.isArray(item) ? renderMultiGeometryPlacemark(item) : renderFeature(item);
}

function renderFolder(
  folder: Folder,
  allFolders: Folder[],
  allPlacemarks: Placemark[],
  allScreenOverlays: ScreenOverlay[],
): string {
  const childFolders = allFolders.filter((f) => f.parentId === folder.id);
  const childPlacemarks = allPlacemarks.filter((p) => p.folderId === folder.id);
  const childScreenOverlays = allScreenOverlays.filter((o) => o.folderId === folder.id);
  const extendedData = renderExtendedData([
    { name: "webglobe:order", value: String(folder.order) },
    { name: "webglobe:createdAt", value: folder.createdAt },
    { name: "webglobe:updatedAt", value: folder.updatedAt },
  ]);
  const childrenXml = [
    ...childFolders.map((f) => renderFolder(f, allFolders, allPlacemarks, allScreenOverlays)),
    ...groupForRender(childPlacemarks).map(renderFeatureOrGroup),
    ...childScreenOverlays.map(renderScreenOverlay),
  ].join("");
  const description = folder.description
    ? `<description>${escapeXml(folder.description)}</description>`
    : "";
  const open = folder.open !== undefined ? `<open>${folder.open ? 1 : 0}</open>` : "";
  return `<Folder id="${escapeXml(folder.id)}"><name>${escapeXml(folder.name)}</name>${description}${open}<visibility>${folder.visibility ? 1 : 0}</visibility>${extendedData}${childrenXml}</Folder>`;
}

export function serializeKml({
  folders,
  placemarks,
  styles,
  screenOverlays = [],
}: SerializeKmlInput): string {
  const rootFolders = folders.filter((f) => f.parentId === null);
  const rootPlacemarks = placemarks.filter((p) => p.folderId === null);
  const rootScreenOverlays = screenOverlays.filter((o) => o.folderId === null);

  const body = [
    ...styles.map(renderStyle),
    ...rootFolders.map((f) => renderFolder(f, folders, placemarks, screenOverlays)),
    ...groupForRender(rootPlacemarks).map(renderFeatureOrGroup),
    ...rootScreenOverlays.map(renderScreenOverlay),
  ].join("");

  return `<?xml version="1.0" encoding="UTF-8"?><kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2"><Document>${body}</Document></kml>`;
}
