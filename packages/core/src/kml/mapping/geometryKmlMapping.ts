import {
  createCircleGeometry,
  createLineStringGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
  type CircleGeometry,
  type GeoPoint,
  type LineStringGeometry,
  type PointGeometry,
  type PolygonGeometry,
  type RectangleGeometry,
} from "../../domain/geometry.js";
import { circleToPolygonRing } from "./circleApproximation.js";

/**
 * GroundOverlay and Model are not `<Placemark><Point|LineString|Polygon>`
 * shapes - they're structurally different KML elements (GroundOverlay is a
 * sibling Feature to Placemark; Model has Location/Orientation/Scale/Link
 * children). They're parsed/serialized directly in parseKml.ts/serializeKml.ts
 * and never flow through this shape-family mapping.
 */
export type ShapePlacemarkGeometry =
  PointGeometry | CircleGeometry | RectangleGeometry | PolygonGeometry | LineStringGeometry;

export type KmlGeometryElement = "Point" | "Polygon" | "LineString";

export interface KmlExtendedDataEntry {
  name: string;
  value: string;
}

export interface KmlGeometryResult {
  element: KmlGeometryElement;
  /** [outerRing, ...innerRings] for Polygon; [[singlePoint]] for Point. */
  rings: GeoPoint[][];
  extendedData: KmlExtendedDataEntry[];
}

function rectangleRing(bounds: {
  north: number;
  south: number;
  east: number;
  west: number;
}): GeoPoint[] {
  const { north, south, east, west } = bounds;
  return [
    { lon: west, lat: south },
    { lon: east, lat: south },
    { lon: east, lat: north },
    { lon: west, lat: north },
    { lon: west, lat: south },
  ];
}

/**
 * KML has no native Circle/Rectangle geometry - Google Earth itself stores
 * them as Polygons. We do the same on export for interop, but also stash the
 * original shape parameters in ExtendedData so re-importing into this app
 * reconstructs the exact Circle/Rectangle rather than a polygon approximation.
 */
export function domainGeometryToKml(geometry: ShapePlacemarkGeometry): KmlGeometryResult {
  switch (geometry.type) {
    case "Point":
      return { element: "Point", rings: [[geometry.coordinates]], extendedData: [] };
    case "Polygon":
      return {
        element: "Polygon",
        rings: [geometry.outerRing, ...(geometry.innerRings ?? [])],
        extendedData: [],
      };
    case "Rectangle":
      return {
        element: "Polygon",
        rings: [rectangleRing(geometry)],
        extendedData: [
          { name: "terra-globe:shape", value: "rectangle" },
          { name: "terra-globe:north", value: String(geometry.north) },
          { name: "terra-globe:south", value: String(geometry.south) },
          { name: "terra-globe:east", value: String(geometry.east) },
          { name: "terra-globe:west", value: String(geometry.west) },
        ],
      };
    case "Circle":
      return {
        element: "Polygon",
        rings: [circleToPolygonRing(geometry.center, geometry.radiusMeters)],
        extendedData: [
          { name: "terra-globe:shape", value: "circle" },
          { name: "terra-globe:centerLon", value: String(geometry.center.lon) },
          { name: "terra-globe:centerLat", value: String(geometry.center.lat) },
          { name: "terra-globe:radiusMeters", value: String(geometry.radiusMeters) },
        ],
      };
    case "LineString":
      return { element: "LineString", rings: [geometry.path], extendedData: [] };
  }
}

export function kmlToDomainGeometry(
  element: KmlGeometryElement,
  rings: GeoPoint[][],
  extendedData: Record<string, string>,
): ShapePlacemarkGeometry {
  const shape = extendedData["terra-globe:shape"];

  if (shape === "rectangle") {
    return createRectangleGeometry({
      north: Number(extendedData["terra-globe:north"]),
      south: Number(extendedData["terra-globe:south"]),
      east: Number(extendedData["terra-globe:east"]),
      west: Number(extendedData["terra-globe:west"]),
    });
  }

  if (shape === "circle") {
    return createCircleGeometry(
      {
        lon: Number(extendedData["terra-globe:centerLon"]),
        lat: Number(extendedData["terra-globe:centerLat"]),
      },
      Number(extendedData["terra-globe:radiusMeters"]),
    );
  }

  if (element === "Point") {
    const point = rings[0]![0];
    return createPointGeometry(point!);
  }

  if (element === "LineString") {
    return createLineStringGeometry(rings[0]!);
  }

  const [outerRing, ...innerRings] = rings;
  return createPolygonGeometry(outerRing!, innerRings.length > 0 ? innerRings : undefined);
}
