import {
  createCircleGeometry,
  createLineStringGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
  type GeoPoint,
  type PlacemarkGeometry,
} from "../../domain/geometry.js";
import { circleToPolygonRing } from "./circleApproximation.js";

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
export function domainGeometryToKml(geometry: PlacemarkGeometry): KmlGeometryResult {
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
          { name: "webglobe:shape", value: "rectangle" },
          { name: "webglobe:north", value: String(geometry.north) },
          { name: "webglobe:south", value: String(geometry.south) },
          { name: "webglobe:east", value: String(geometry.east) },
          { name: "webglobe:west", value: String(geometry.west) },
        ],
      };
    case "Circle":
      return {
        element: "Polygon",
        rings: [circleToPolygonRing(geometry.center, geometry.radiusMeters)],
        extendedData: [
          { name: "webglobe:shape", value: "circle" },
          { name: "webglobe:centerLon", value: String(geometry.center.lon) },
          { name: "webglobe:centerLat", value: String(geometry.center.lat) },
          { name: "webglobe:radiusMeters", value: String(geometry.radiusMeters) },
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
): PlacemarkGeometry {
  const shape = extendedData["webglobe:shape"];

  if (shape === "rectangle") {
    return createRectangleGeometry({
      north: Number(extendedData["webglobe:north"]),
      south: Number(extendedData["webglobe:south"]),
      east: Number(extendedData["webglobe:east"]),
      west: Number(extendedData["webglobe:west"]),
    });
  }

  if (shape === "circle") {
    return createCircleGeometry(
      {
        lon: Number(extendedData["webglobe:centerLon"]),
        lat: Number(extendedData["webglobe:centerLat"]),
      },
      Number(extendedData["webglobe:radiusMeters"]),
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
