import { createLineStringGeometry } from "../domain/geometry.js";
import type { RouteLeg } from "./RoutingProvider.js";

interface OsrmRoute {
  geometry: { coordinates: [number, number][] };
  distance: number;
  duration: number;
}

function isOsrmRoute(value: unknown): value is OsrmRoute {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<OsrmRoute>;
  return (
    typeof candidate.distance === "number" &&
    typeof candidate.duration === "number" &&
    typeof candidate.geometry === "object" &&
    candidate.geometry !== null &&
    Array.isArray((candidate.geometry as { coordinates?: unknown }).coordinates)
  );
}

/** Shared response shape between OSRM and Mapbox Directions (both OSRM-derived APIs). */
export function mapOsrmCompatibleResults(body: unknown): RouteLeg[] {
  if (typeof body !== "object" || body === null) {
    throw new Error("Malformed routing response: expected a JSON object");
  }
  const routes = (body as { routes?: unknown }).routes;
  if (!Array.isArray(routes)) {
    throw new Error("Malformed routing response: expected a `routes` array");
  }

  const malformed = routes.filter((route) => !isOsrmRoute(route));
  if (malformed.length > 0) {
    throw new Error(
      `Malformed routing response: ${malformed.length} route(s) missing required fields`,
    );
  }

  return (routes as OsrmRoute[]).map((route) => ({
    geometry: createLineStringGeometry(
      route.geometry.coordinates.map(([lon, lat]) => ({ lon, lat })),
    ),
    distanceMeters: route.distance,
    durationSeconds: route.duration,
  }));
}
