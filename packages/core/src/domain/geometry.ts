export interface GeoPoint {
  lon: number;
  lat: number;
  altitude?: number;
}

export interface PointGeometry {
  type: "Point";
  coordinates: GeoPoint;
}

export interface CircleGeometry {
  type: "Circle";
  center: GeoPoint;
  radiusMeters: number;
}

export interface RectangleGeometry {
  type: "Rectangle";
  north: number;
  south: number;
  east: number;
  west: number;
}

export interface PolygonGeometry {
  type: "Polygon";
  outerRing: GeoPoint[];
  innerRings?: GeoPoint[][];
}

export interface LineStringGeometry {
  type: "LineString";
  path: GeoPoint[];
}

export type PlacemarkGeometry =
  PointGeometry | CircleGeometry | RectangleGeometry | PolygonGeometry | LineStringGeometry;

function assertInRange(value: number, min: number, max: number, label: string): void {
  if (!Number.isFinite(value) || value < min || value > max) {
    throw new Error(`${label} must be in range [${min}, ${max}], got ${value}`);
  }
}

function assertValidGeoPoint(point: GeoPoint): void {
  assertInRange(point.lon, -180, 180, "longitude");
  assertInRange(point.lat, -90, 90, "latitude");
}

export function createPointGeometry(coordinates: GeoPoint): PointGeometry {
  assertValidGeoPoint(coordinates);
  return { type: "Point", coordinates };
}

export function createCircleGeometry(center: GeoPoint, radiusMeters: number): CircleGeometry {
  assertValidGeoPoint(center);
  if (!Number.isFinite(radiusMeters) || radiusMeters <= 0) {
    throw new Error(`radius must be a positive number, got ${radiusMeters}`);
  }
  return { type: "Circle", center, radiusMeters };
}

export interface RectangleBounds {
  north: number;
  south: number;
  east: number;
  west: number;
}

export function createRectangleGeometry(bounds: RectangleBounds): RectangleGeometry {
  const { north, south, east, west } = bounds;
  assertInRange(north, -90, 90, "north");
  assertInRange(south, -90, 90, "south");
  assertInRange(east, -180, 180, "east");
  assertInRange(west, -180, 180, "west");
  if (south >= north) {
    throw new Error(`south (${south}) must be less than north (${north})`);
  }
  if (west >= east) {
    throw new Error(`west (${west}) must be less than east (${east})`);
  }
  return { type: "Rectangle", north, south, east, west };
}

function assertValidRing(ring: GeoPoint[], label: string): void {
  if (ring.length < 3) {
    throw new Error(`${label} must have at least 3 points, got ${ring.length}`);
  }
  ring.forEach(assertValidGeoPoint);
}

export function createPolygonGeometry(
  outerRing: GeoPoint[],
  innerRings?: GeoPoint[][],
): PolygonGeometry {
  assertValidRing(outerRing, "outer ring");
  innerRings?.forEach((ring) => assertValidRing(ring, "inner ring"));
  return { type: "Polygon", outerRing, innerRings };
}

export function createLineStringGeometry(path: GeoPoint[]): LineStringGeometry {
  if (path.length < 2) {
    throw new Error(`path must have at least 2 points, got ${path.length}`);
  }
  path.forEach(assertValidGeoPoint);
  return { type: "LineString", path };
}
