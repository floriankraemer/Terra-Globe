import type { GeoPoint } from "@terra-globe/core";
import type { RectangleBounds } from "@terra-globe/core";

const EARTH_RADIUS_METERS = 6_371_000;

export function rectangleFromCorners(a: GeoPoint, b: GeoPoint): RectangleBounds {
  return {
    north: Math.max(a.lat, b.lat),
    south: Math.min(a.lat, b.lat),
    east: Math.max(a.lon, b.lon),
    west: Math.min(a.lon, b.lon),
  };
}

function toRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

/** Great-circle (haversine) distance between two points, in meters. */
export function circleRadiusMeters(center: GeoPoint, edge: GeoPoint): number {
  const lat1 = toRadians(center.lat);
  const lat2 = toRadians(edge.lat);
  const deltaLat = toRadians(edge.lat - center.lat);
  const deltaLon = toRadians(edge.lon - center.lon);

  const a =
    Math.sin(deltaLat / 2) ** 2 + Math.cos(lat1) * Math.cos(lat2) * Math.sin(deltaLon / 2) ** 2;
  const c = 2 * Math.atan2(Math.sqrt(a), Math.sqrt(1 - a));

  return EARTH_RADIUS_METERS * c;
}
