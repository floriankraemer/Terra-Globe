import { createLineStringGeometry, type GeoPoint } from "../domain/geometry.js";
import type { RouteLeg, RoutingProfile, RoutingProvider } from "./RoutingProvider.js";

const GRAPHHOPPER_ROUTE_URL = "https://graphhopper.com/api/1/route";

const PROFILE_MAP: Record<RoutingProfile, string> = {
  car: "car",
  foot: "foot",
  bike: "bike",
};

interface GraphHopperPath {
  points: { coordinates: [number, number][] };
  distance: number;
  time: number;
}

function isGraphHopperPath(value: unknown): value is GraphHopperPath {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<GraphHopperPath>;
  return (
    typeof candidate.distance === "number" &&
    typeof candidate.time === "number" &&
    typeof candidate.points === "object" &&
    candidate.points !== null &&
    Array.isArray((candidate.points as { coordinates?: unknown }).coordinates)
  );
}

export function mapGraphHopperResults(body: unknown): RouteLeg[] {
  if (typeof body !== "object" || body === null) {
    throw new Error("Malformed GraphHopper response: expected a JSON object");
  }
  const paths = (body as { paths?: unknown }).paths;
  if (!Array.isArray(paths)) {
    throw new Error("Malformed GraphHopper response: expected a `paths` array");
  }

  const malformed = paths.filter((path) => !isGraphHopperPath(path));
  if (malformed.length > 0) {
    throw new Error(
      `Malformed GraphHopper response: ${malformed.length} path(s) missing required fields`,
    );
  }

  return (paths as GraphHopperPath[]).map((path) => ({
    geometry: createLineStringGeometry(path.points.coordinates.map(([lon, lat]) => ({ lon, lat }))),
    distanceMeters: path.distance,
    durationSeconds: path.time / 1000,
  }));
}

/** GraphHopper Directions API adapter, requires an API key. */
export class GraphHopperRoutingProvider implements RoutingProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async route(waypoints: GeoPoint[], profile: RoutingProfile): Promise<RouteLeg[]> {
    if (waypoints.length < 2) return [];

    const url = new URL(GRAPHHOPPER_ROUTE_URL);
    for (const p of waypoints) url.searchParams.append("point", `${p.lat},${p.lon}`);
    url.searchParams.set("profile", PROFILE_MAP[profile]);
    url.searchParams.set("points_encoded", "false");
    url.searchParams.set("algorithm", "alternative_route");
    url.searchParams.set("alternative_route.max_paths", "3");
    url.searchParams.set("key", this.apiKey);

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`GraphHopper routing failed: ${response.status} ${response.statusText}`);
    }

    return mapGraphHopperResults(await response.json());
  }
}
