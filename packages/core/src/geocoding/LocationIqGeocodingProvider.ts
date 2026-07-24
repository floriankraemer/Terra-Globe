import type { GeoPoint } from "../domain/geometry.js";
import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";
import { mapNominatimResults } from "./nominatimCompatible.js";

const LOCATIONIQ_SEARCH_URL = "https://us1.locationiq.com/v1/search";
const LOCATIONIQ_REVERSE_URL = "https://us1.locationiq.com/v1/reverse";

/** LocationIQ adapter: Nominatim-compatible response shape, requires an API key. */
export class LocationIqGeocodingProvider implements GeocodingProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async search(query: string): Promise<GeocodeResult[]> {
    const trimmed = query.trim();
    if (trimmed.length === 0) return [];

    const url = new URL(LOCATIONIQ_SEARCH_URL);
    url.searchParams.set("q", trimmed);
    url.searchParams.set("format", "json");
    url.searchParams.set("key", this.apiKey);

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`LocationIQ search failed: ${response.status} ${response.statusText}`);
    }

    return mapNominatimResults(await response.json());
  }

  async reverse(point: GeoPoint): Promise<string | null> {
    const url = new URL(LOCATIONIQ_REVERSE_URL);
    url.searchParams.set("lat", String(point.lat));
    url.searchParams.set("lon", String(point.lon));
    url.searchParams.set("format", "json");
    url.searchParams.set("key", this.apiKey);

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(
        `LocationIQ reverse geocode failed: ${response.status} ${response.statusText}`,
      );
    }

    const body: unknown = await response.json();
    return mapNominatimResults([body])[0]?.label ?? null;
  }
}
