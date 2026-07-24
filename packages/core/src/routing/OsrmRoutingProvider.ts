import type { GeoPoint } from "../domain/geometry.js";
import type { RouteLeg, RoutingProfile, RoutingProvider } from "./RoutingProvider.js";
import { mapOsrmCompatibleResults } from "./osrmCompatible.js";

const OSRM_BASE_URL = "https://router.project-osrm.org";

const PROFILE_MAP: Record<RoutingProfile, string> = {
  car: "driving",
  foot: "walking",
  bike: "cycling",
};

/** OSRM adapter. Uses the free public demo server by default, no API key required. */
export class OsrmRoutingProvider implements RoutingProvider {
  constructor(
    private readonly baseUrl: string = OSRM_BASE_URL,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async route(waypoints: GeoPoint[], profile: RoutingProfile): Promise<RouteLeg[]> {
    if (waypoints.length < 2) return [];

    const coordinates = waypoints.map((p) => `${p.lon},${p.lat}`).join(";");
    const url = new URL(`${this.baseUrl}/route/v1/${PROFILE_MAP[profile]}/${coordinates}`);
    url.searchParams.set("alternatives", "true");
    url.searchParams.set("geometries", "geojson");
    url.searchParams.set("overview", "full");

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`OSRM routing failed: ${response.status} ${response.statusText}`);
    }

    return mapOsrmCompatibleResults(await response.json());
  }
}
