import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";

const MAPBOX_GEOCODING_BASE_URL = "https://api.mapbox.com/geocoding/v5/mapbox.places";

interface MapboxFeature {
  place_name: string;
  center: [number, number];
}

function isMapboxFeature(value: unknown): value is MapboxFeature {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<MapboxFeature>;
  return (
    typeof candidate.place_name === "string" &&
    Array.isArray(candidate.center) &&
    candidate.center.length === 2 &&
    typeof candidate.center[0] === "number" &&
    typeof candidate.center[1] === "number"
  );
}

export function mapMapboxResults(body: unknown): GeocodeResult[] {
  if (typeof body !== "object" || body === null) return [];
  const features = (body as { features?: unknown }).features;
  if (!Array.isArray(features)) return [];

  return features.filter(isMapboxFeature).map((feature) => ({
    label: feature.place_name,
    point: { lon: feature.center[0], lat: feature.center[1] },
  }));
}

/** Mapbox Geocoding v5 adapter, requires an API key (access token). */
export class MapboxGeocodingProvider implements GeocodingProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async search(query: string): Promise<GeocodeResult[]> {
    const trimmed = query.trim();
    if (trimmed.length === 0) return [];

    const url = new URL(`${MAPBOX_GEOCODING_BASE_URL}/${encodeURIComponent(trimmed)}.json`);
    url.searchParams.set("access_token", this.apiKey);

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`Mapbox geocoding search failed: ${response.status} ${response.statusText}`);
    }

    return mapMapboxResults(await response.json());
  }
}
