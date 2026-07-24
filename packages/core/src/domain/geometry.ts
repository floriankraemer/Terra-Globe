export type AltitudeMode = "clampToGround" | "relativeToGround" | "absolute";

export interface GeoPoint {
  lon: number;
  lat: number;
  altitude?: number;
  altitudeMode?: AltitudeMode;
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
  tessellate?: boolean;
  extrudeHeight?: number;
}

export interface LineStringGeometry {
  type: "LineString";
  path: GeoPoint[];
  tessellate?: boolean;
  /** Parallel array to `path` (gx:Track "when" timestamps), ISO 8601. */
  timestamps?: string[];
}

export interface GroundOverlayGeometry {
  type: "GroundOverlay";
  bounds: RectangleBounds;
  imageUrl: string;
  rotation?: number;
}

export interface ModelGeometry {
  type: "Model";
  position: GeoPoint;
  modelUri: string;
  scale?: number;
  heading?: number;
  tilt?: number;
  roll?: number;
}

export type PlacemarkGeometry =
  | PointGeometry
  | CircleGeometry
  | RectangleGeometry
  | PolygonGeometry
  | LineStringGeometry
  | GroundOverlayGeometry
  | ModelGeometry;

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
  options?: { tessellate?: boolean; extrudeHeight?: number },
): PolygonGeometry {
  assertValidRing(outerRing, "outer ring");
  innerRings?.forEach((ring) => assertValidRing(ring, "inner ring"));
  return {
    type: "Polygon",
    outerRing,
    innerRings,
    tessellate: options?.tessellate,
    extrudeHeight: options?.extrudeHeight,
  };
}

export function createLineStringGeometry(
  path: GeoPoint[],
  options?: { tessellate?: boolean; timestamps?: string[] },
): LineStringGeometry {
  if (path.length < 2) {
    throw new Error(`path must have at least 2 points, got ${path.length}`);
  }
  path.forEach(assertValidGeoPoint);
  if (options?.timestamps && options.timestamps.length !== path.length) {
    throw new Error(
      `timestamps length (${options.timestamps.length}) must match path length (${path.length})`,
    );
  }
  return {
    type: "LineString",
    path,
    tessellate: options?.tessellate,
    timestamps: options?.timestamps,
  };
}

export function createGroundOverlayGeometry(
  bounds: RectangleBounds,
  imageUrl: string,
  rotation?: number,
): GroundOverlayGeometry {
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
  if (imageUrl.trim().length === 0) {
    throw new Error("imageUrl must not be empty");
  }
  return { type: "GroundOverlay", bounds, imageUrl, rotation };
}

export function createModelGeometry(
  position: GeoPoint,
  modelUri: string,
  options?: { scale?: number; heading?: number; tilt?: number; roll?: number },
): ModelGeometry {
  assertValidGeoPoint(position);
  if (modelUri.trim().length === 0) {
    throw new Error("modelUri must not be empty");
  }
  return {
    type: "Model",
    position,
    modelUri,
    scale: options?.scale,
    heading: options?.heading,
    tilt: options?.tilt,
    roll: options?.roll,
  };
}
