import { createLineStringGeometry, type GeoPoint } from "./geometry.js";
import { haversineMeters } from "./trackProfile.js";
import type { RouteLeg } from "../routing/RoutingProvider.js";

export type EstimatedTravelMode = "train" | "plane";

const AVERAGE_SPEED_METERS_PER_SECOND: Record<EstimatedTravelMode, number> = {
  train: 120_000 / 3600,
  plane: 800_000 / 3600,
};

/**
 * Straight-line distance/time estimate for travel modes with no real routing
 * API (timetabled train, flight). Not a real route - a rough estimate drawn
 * as a straight line through the waypoints in order.
 */
export function estimateStraightLineRoute(
  waypoints: GeoPoint[],
  mode: EstimatedTravelMode,
): RouteLeg {
  let distanceMeters = 0;
  for (let i = 1; i < waypoints.length; i++) {
    distanceMeters += haversineMeters(waypoints[i - 1]!, waypoints[i]!);
  }
  return {
    geometry: createLineStringGeometry(waypoints),
    distanceMeters,
    durationSeconds: distanceMeters / AVERAGE_SPEED_METERS_PER_SECOND[mode],
  };
}
