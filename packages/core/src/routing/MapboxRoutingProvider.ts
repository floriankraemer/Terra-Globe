import type { GeoPoint } from "../domain/geometry.js";
import type { RouteLeg, RoutingProfile, RoutingProvider } from "./RoutingProvider.js";
import { mapOsrmCompatibleResults } from "./osrmCompatible.js";

const MAPBOX_DIRECTIONS_BASE_URL = "https://api.mapbox.com/directions/v5/mapbox";

const PROFILE_MAP: Record<RoutingProfile, string> = {
  car: "driving",
  foot: "walking",
  bike: "cycling",
};

/** Mapbox Directions v5 adapter, requires an API key (access token). */
export class MapboxRoutingProvider implements RoutingProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async route(waypoints: GeoPoint[], profile: RoutingProfile): Promise<RouteLeg[]> {
    if (waypoints.length < 2) return [];

    const coordinates = waypoints.map((p) => `${p.lon},${p.lat}`).join(";");
    const url = new URL(`${MAPBOX_DIRECTIONS_BASE_URL}/${PROFILE_MAP[profile]}/${coordinates}`);
    url.searchParams.set("alternatives", "true");
    url.searchParams.set("geometries", "geojson");
    url.searchParams.set("overview", "full");
    url.searchParams.set("access_token", this.apiKey);

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`Mapbox routing failed: ${response.status} ${response.statusText}`);
    }

    return mapOsrmCompatibleResults(await response.json());
  }
}
