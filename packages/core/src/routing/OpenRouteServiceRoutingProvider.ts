import { createLineStringGeometry, type GeoPoint } from "../domain/geometry.js";
import type { RouteLeg, RoutingProfile, RoutingProvider } from "./RoutingProvider.js";

const ORS_DIRECTIONS_BASE_URL = "https://api.openrouteservice.org/v2/directions";

const PROFILE_MAP: Record<RoutingProfile, string> = {
  car: "driving-car",
  foot: "foot-walking",
  bike: "cycling-regular",
};

interface OrsFeature {
  geometry: { coordinates: [number, number][] };
  properties: { summary: { distance: number; duration: number } };
}

function isOrsFeature(value: unknown): value is OrsFeature {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<OrsFeature>;
  const summary = candidate.properties?.summary;
  return (
    typeof summary?.distance === "number" &&
    typeof summary?.duration === "number" &&
    Array.isArray(candidate.geometry?.coordinates)
  );
}

export function mapOpenRouteServiceResults(body: unknown): RouteLeg[] {
  if (typeof body !== "object" || body === null) {
    throw new Error("Malformed OpenRouteService response: expected a JSON object");
  }
  const features = (body as { features?: unknown }).features;
  if (!Array.isArray(features)) {
    throw new Error("Malformed OpenRouteService response: expected a `features` array");
  }

  const malformed = features.filter((feature) => !isOrsFeature(feature));
  if (malformed.length > 0) {
    throw new Error(
      `Malformed OpenRouteService response: ${malformed.length} feature(s) missing required fields`,
    );
  }

  return (features as OrsFeature[]).map((feature) => ({
    geometry: createLineStringGeometry(
      feature.geometry.coordinates.map(([lon, lat]) => ({ lon, lat })),
    ),
    distanceMeters: feature.properties.summary.distance,
    durationSeconds: feature.properties.summary.duration,
  }));
}

/** OpenRouteService adapter, requires an API key. */
export class OpenRouteServiceRoutingProvider implements RoutingProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async route(waypoints: GeoPoint[], profile: RoutingProfile): Promise<RouteLeg[]> {
    if (waypoints.length < 2) return [];

    const url = new URL(`${ORS_DIRECTIONS_BASE_URL}/${PROFILE_MAP[profile]}/geojson`);
    const response = await this.fetchImpl(url, {
      method: "POST",
      headers: {
        Accept: "application/json",
        "Content-Type": "application/json",
        Authorization: this.apiKey,
      },
      body: JSON.stringify({
        coordinates: waypoints.map((p) => [p.lon, p.lat]),
        alternative_routes: { target_count: 3 },
      }),
    });
    if (!response.ok) {
      throw new Error(`OpenRouteService routing failed: ${response.status} ${response.statusText}`);
    }

    return mapOpenRouteServiceResults(await response.json());
  }
}
