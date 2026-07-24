import type { GeoPoint } from "../domain/geometry.js";
import type { LineStringGeometry } from "../domain/geometry.js";

export type RoutingProfile = "car" | "foot" | "bike";

export interface RouteLeg {
  geometry: LineStringGeometry;
  distanceMeters: number;
  durationSeconds: number;
}

/**
 * Routing port (dependency-inversion boundary). No adapter-specific types
 * (OSRM/GraphHopper/ORS/Mapbox response shapes, API keys, ...) may leak
 * through this interface.
 */
export interface RoutingProvider {
  /** Routes through waypoints in the given order. [0] is the primary route, rest are alternatives. */
  route(waypoints: GeoPoint[], profile: RoutingProfile): Promise<RouteLeg[]>;
}
