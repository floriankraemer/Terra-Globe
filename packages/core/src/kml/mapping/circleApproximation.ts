import type { GeoPoint } from "../../domain/geometry.js";

const EARTH_RADIUS_METERS = 6_371_000;

function toRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

function toDegrees(radians: number): number {
  return (radians * 180) / Math.PI;
}

/**
 * Approximates a circle as a closed polygon ring, for KML export (KML has no
 * native circle geometry). Uses the spherical destination-point formula.
 */
export function circleToPolygonRing(
  center: GeoPoint,
  radiusMeters: number,
  segments = 64,
): GeoPoint[] {
  const lat1 = toRadians(center.lat);
  const lon1 = toRadians(center.lon);
  const angularDistance = radiusMeters / EARTH_RADIUS_METERS;

  const ring: GeoPoint[] = [];
  for (let i = 0; i <= segments; i++) {
    const bearing = (2 * Math.PI * i) / segments;
    const lat2 = Math.asin(
      Math.sin(lat1) * Math.cos(angularDistance) +
        Math.cos(lat1) * Math.sin(angularDistance) * Math.cos(bearing),
    );
    const lon2 =
      lon1 +
      Math.atan2(
        Math.sin(bearing) * Math.sin(angularDistance) * Math.cos(lat1),
        Math.cos(angularDistance) - Math.sin(lat1) * Math.sin(lat2),
      );
    ring.push({ lon: toDegrees(lon2), lat: toDegrees(lat2) });
  }
  return ring;
}
