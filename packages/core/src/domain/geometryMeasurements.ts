import type { CircleGeometry, GeoPoint, PolygonGeometry, RectangleGeometry } from "./geometry.js";

const EARTH_RADIUS_METERS = 6_371_000;

function toRadians(degrees: number): number {
  return (degrees * Math.PI) / 180;
}

export function circleAreaSquareMeters(geometry: CircleGeometry): number {
  return Math.PI * geometry.radiusMeters ** 2;
}

export function circleCircumferenceMeters(geometry: CircleGeometry): number {
  return 2 * Math.PI * geometry.radiusMeters;
}

/** Exact area of a graticule rectangle (bounded by meridians and parallels). */
export function rectangleAreaSquareMeters(geometry: RectangleGeometry): number {
  const lonSpan = toRadians(geometry.east - geometry.west);
  const latFactor = Math.sin(toRadians(geometry.north)) - Math.sin(toRadians(geometry.south));
  return Math.abs(EARTH_RADIUS_METERS ** 2 * lonSpan * latFactor);
}

/** Spherical polygon area (sum-over-edges formula), with holes subtracted. */
function ringAreaSquareMeters(ring: GeoPoint[]): number {
  if (ring.length < 3) return 0;
  let sum = 0;
  for (let i = 0; i < ring.length; i++) {
    const a = ring[i]!;
    const b = ring[(i + 1) % ring.length]!;
    sum += toRadians(b.lon - a.lon) * (2 + Math.sin(toRadians(a.lat)) + Math.sin(toRadians(b.lat)));
  }
  return Math.abs((sum * EARTH_RADIUS_METERS ** 2) / 2);
}

export function polygonAreaSquareMeters(geometry: PolygonGeometry): number {
  const outer = ringAreaSquareMeters(geometry.outerRing);
  const holes = (geometry.innerRings ?? []).reduce(
    (sum, ring) => sum + ringAreaSquareMeters(ring),
    0,
  );
  return Math.max(0, outer - holes);
}
