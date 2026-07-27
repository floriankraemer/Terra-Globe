import type { GeoPoint } from "../domain/geometry.js";
import type { RouteLeg, RoutingProfile, RoutingProvider } from "./RoutingProvider.js";
import { mapOsrmCompatibleResults } from "./osrmCompatible.js";

const PROFILE_MAP: Record<RoutingProfile, string> = {
  car: "driving",
  foot: "walking",
  bike: "cycling",
};

/**
 * FOSSGIS/OpenStreetMap Germany runs a separate OSRM instance per profile
 * (routed-car/routed-foot/routed-bike), unlike router.project-osrm.org which
 * ignores the profile segment in the URL and always serves the driving graph
 * (see https://github.com/Project-OSRM/osrm-backend/issues/4868). Using
 * per-profile hosts is what actually makes foot/bike routing correct.
 */
const OSM_DE_HOSTS: Record<RoutingProfile, string> = {
  car: "https://routing.openstreetmap.de/routed-car",
  foot: "https://routing.openstreetmap.de/routed-foot",
  bike: "https://routing.openstreetmap.de/routed-bike",
};

/** OSRM adapter. Uses the free public per-profile OpenStreetMap Germany instances by default, no API key required. */
export class OsrmRoutingProvider implements RoutingProvider {
  constructor(
    private readonly hosts: Record<RoutingProfile, string> = OSM_DE_HOSTS,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async route(waypoints: GeoPoint[], profile: RoutingProfile): Promise<RouteLeg[]> {
    if (waypoints.length < 2) return [];

    const coordinates = waypoints.map((p) => `${p.lon},${p.lat}`).join(";");
    const url = new URL(`${this.hosts[profile]}/route/v1/${PROFILE_MAP[profile]}/${coordinates}`);
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
