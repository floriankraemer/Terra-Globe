import type { GeoPoint, PlacemarkGeometry } from "./geometry.js";

function translatePoint(point: GeoPoint, deltaLon: number, deltaLat: number): GeoPoint {
  return { ...point, lon: point.lon + deltaLon, lat: point.lat + deltaLat };
}

export function translateGeometry(
  geometry: PlacemarkGeometry,
  deltaLon: number,
  deltaLat: number,
): PlacemarkGeometry {
  switch (geometry.type) {
    case "Point":
      return { ...geometry, coordinates: translatePoint(geometry.coordinates, deltaLon, deltaLat) };
    case "Model":
      return { ...geometry, position: translatePoint(geometry.position, deltaLon, deltaLat) };
    case "Circle":
      return { ...geometry, center: translatePoint(geometry.center, deltaLon, deltaLat) };
    case "Rectangle":
      return {
        ...geometry,
        north: geometry.north + deltaLat,
        south: geometry.south + deltaLat,
        east: geometry.east + deltaLon,
        west: geometry.west + deltaLon,
      };
    case "GroundOverlay":
      return {
        ...geometry,
        bounds: {
          north: geometry.bounds.north + deltaLat,
          south: geometry.bounds.south + deltaLat,
          east: geometry.bounds.east + deltaLon,
          west: geometry.bounds.west + deltaLon,
        },
      };
    case "Polygon":
      return {
        ...geometry,
        outerRing: geometry.outerRing.map((p) => translatePoint(p, deltaLon, deltaLat)),
        innerRings: geometry.innerRings?.map((ring) =>
          ring.map((p) => translatePoint(p, deltaLon, deltaLat)),
        ),
      };
    case "LineString":
      return {
        ...geometry,
        path: geometry.path.map((p) => translatePoint(p, deltaLon, deltaLat)),
      };
    default:
      return geometry;
  }
}
