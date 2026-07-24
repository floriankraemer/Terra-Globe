import type { Folder } from "../domain/folder.js";
import type { GeoPoint } from "../domain/geometry.js";
import type { Placemark } from "../domain/placemark.js";
import type { Style } from "../domain/style.js";
import { domainGeometryToKml } from "./mapping/geometryKmlMapping.js";
import { styleToKmlData } from "./mapping/styleKmlMapping.js";
import { escapeXml } from "./xmlEscape.js";

export interface SerializeKmlInput {
  folders: Folder[];
  placemarks: Placemark[];
  styles: Style[];
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
  const { element, rings, extendedData } = domainGeometryToKml(placemark.geometry);
  if (element === "Point") {
    return {
      xml: `<Point><coordinates>${renderCoordinates(rings[0]!)}</coordinates></Point>`,
      extendedData,
    };
  }
  if (element === "LineString") {
    return {
      xml: `<LineString><coordinates>${renderCoordinates(rings[0]!)}</coordinates></LineString>`,
      extendedData,
    };
  }
  const [outer, ...inner] = rings;
  const outerXml = `<outerBoundaryIs><LinearRing><coordinates>${renderCoordinates(outer!)}</coordinates></LinearRing></outerBoundaryIs>`;
  const innerXml = inner
    .map(
      (ring) =>
        `<innerBoundaryIs><LinearRing><coordinates>${renderCoordinates(ring)}</coordinates></LinearRing></innerBoundaryIs>`,
    )
    .join("");
  return { xml: `<Polygon>${outerXml}${innerXml}</Polygon>`, extendedData };
}

function renderStyle(style: Style): string {
  const data = styleToKmlData(style);
  const iconStyle = data.iconUrl
    ? `<IconStyle>${data.iconScale !== undefined ? `<scale>${data.iconScale}</scale>` : ""}<Icon><href>${escapeXml(data.iconUrl)}</href></Icon></IconStyle>`
    : "";
  const labelStyle = data.labelColorKml
    ? `<LabelStyle><color>${data.labelColorKml}</color></LabelStyle>`
    : "";
  return `<Style id="${escapeXml(data.id)}"><LineStyle><color>${data.lineColorKml}</color><width>${data.lineWidth}</width></LineStyle><PolyStyle><color>${data.polyColorKml}</color><fill>${data.filled ? 1 : 0}</fill><outline>${data.outlineEnabled ? 1 : 0}</outline></PolyStyle>${iconStyle}${labelStyle}</Style>`;
}

function renderPlacemark(placemark: Placemark): string {
  const { xml: geometryXml, extendedData } = renderGeometry(placemark);
  const allExtendedData = [
    ...extendedData,
    { name: "webglobe:createdAt", value: placemark.createdAt },
    { name: "webglobe:updatedAt", value: placemark.updatedAt },
  ];
  const description = placemark.description
    ? `<description>${escapeXml(placemark.description)}</description>`
    : "";
  const styleUrl = placemark.styleId ? `<styleUrl>#${escapeXml(placemark.styleId)}</styleUrl>` : "";
  return `<Placemark id="${escapeXml(placemark.id)}"><name>${escapeXml(placemark.name)}</name>${description}<visibility>${placemark.visibility ? 1 : 0}</visibility>${styleUrl}${geometryXml}${renderExtendedData(allExtendedData)}</Placemark>`;
}

function renderFolder(folder: Folder, allFolders: Folder[], allPlacemarks: Placemark[]): string {
  const childFolders = allFolders.filter((f) => f.parentId === folder.id);
  const childPlacemarks = allPlacemarks.filter((p) => p.folderId === folder.id);
  const extendedData = renderExtendedData([
    { name: "webglobe:order", value: String(folder.order) },
    { name: "webglobe:createdAt", value: folder.createdAt },
    { name: "webglobe:updatedAt", value: folder.updatedAt },
  ]);
  const childrenXml = [
    ...childFolders.map((f) => renderFolder(f, allFolders, allPlacemarks)),
    ...childPlacemarks.map(renderPlacemark),
  ].join("");
  return `<Folder id="${escapeXml(folder.id)}"><name>${escapeXml(folder.name)}</name><visibility>${folder.visibility ? 1 : 0}</visibility>${extendedData}${childrenXml}</Folder>`;
}

export function serializeKml({ folders, placemarks, styles }: SerializeKmlInput): string {
  const rootFolders = folders.filter((f) => f.parentId === null);
  const rootPlacemarks = placemarks.filter((p) => p.folderId === null);

  const body = [
    ...styles.map(renderStyle),
    ...rootFolders.map((f) => renderFolder(f, folders, placemarks)),
    ...rootPlacemarks.map(renderPlacemark),
  ].join("");

  return `<?xml version="1.0" encoding="UTF-8"?><kml xmlns="http://www.opengis.net/kml/2.2"><Document>${body}</Document></kml>`;
}
