import type { GeoPoint, PlacemarkGeometry } from "./geometry.js";

export function geometryCenter(geometry: PlacemarkGeometry): GeoPoint {
  switch (geometry.type) {
    case "Point":
      return geometry.coordinates;
    case "Circle":
      return geometry.center;
    case "Rectangle":
      return {
        lon: (geometry.east + geometry.west) / 2,
        lat: (geometry.north + geometry.south) / 2,
      };
    case "Polygon": {
      const ring = geometry.outerRing;
      const sum = ring.reduce((acc, p) => ({ lon: acc.lon + p.lon, lat: acc.lat + p.lat }), {
        lon: 0,
        lat: 0,
      });
      return { lon: sum.lon / ring.length, lat: sum.lat / ring.length };
    }
    case "LineString": {
      const path = geometry.path;
      const sum = path.reduce((acc, p) => ({ lon: acc.lon + p.lon, lat: acc.lat + p.lat }), {
        lon: 0,
        lat: 0,
      });
      return { lon: sum.lon / path.length, lat: sum.lat / path.length };
    }
    case "GroundOverlay":
      return {
        lon: (geometry.bounds.east + geometry.bounds.west) / 2,
        lat: (geometry.bounds.north + geometry.bounds.south) / 2,
      };
    case "Model":
      return geometry.position;
  }
}
