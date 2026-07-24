import type { GeoPoint, LineStringGeometry } from "./geometry.js";

const EARTH_RADIUS_METERS = 6_371_000;

function toRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

/** Great-circle distance between two points (haversine formula). */
export function haversineMeters(a: GeoPoint, b: GeoPoint): number {
  const dLat = toRadians(b.lat - a.lat);
  const dLon = toRadians(b.lon - a.lon);
  const sinLat = Math.sin(dLat / 2);
  const sinLon = Math.sin(dLon / 2);
  const h = sinLat ** 2 + Math.cos(toRadians(a.lat)) * Math.cos(toRadians(b.lat)) * sinLon ** 2;
  return 2 * EARTH_RADIUS_METERS * Math.asin(Math.min(1, Math.sqrt(h)));
}

export interface TrackProfilePoint {
  distanceMeters: number;
  altitudeMeters: number;
}

/** Cumulative distance and altitude at each vertex of a track's path. */
export function computeTrackProfile(geometry: LineStringGeometry): TrackProfilePoint[] {
  let distanceMeters = 0;
  return geometry.path.map((point, i) => {
    if (i > 0) distanceMeters += haversineMeters(geometry.path[i - 1]!, point);
    return { distanceMeters, altitudeMeters: point.altitude ?? 0 };
  });
}

/** Adaptive gridline spacing: 0.5km under 5km total, 1km beyond. */
export function pickGridStepMeters(totalDistanceMeters: number): number {
  return totalDistanceMeters < 5000 ? 500 : 1000;
}
