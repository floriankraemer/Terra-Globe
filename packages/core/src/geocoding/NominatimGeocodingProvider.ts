import type { GeoPoint } from "../domain/geometry.js";
import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";
import { mapNominatimResults } from "./nominatimCompatible.js";

const NOMINATIM_SEARCH_URL = "https://nominatim.openstreetmap.org/search";
const NOMINATIM_REVERSE_URL = "https://nominatim.openstreetmap.org/reverse";

/** Default adapter: the public OSM Nominatim search API, no API key required. */
export class NominatimGeocodingProvider implements GeocodingProvider {
  constructor(private readonly fetchImpl: typeof fetch = fetch.bind(globalThis)) {}

  async search(query: string): Promise<GeocodeResult[]> {
    const trimmed = query.trim();
    if (trimmed.length === 0) return [];

    const url = new URL(NOMINATIM_SEARCH_URL);
    url.searchParams.set("q", trimmed);
    url.searchParams.set("format", "jsonv2");

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`Nominatim search failed: ${response.status} ${response.statusText}`);
    }

    return mapNominatimResults(await response.json());
  }

  async reverse(point: GeoPoint): Promise<string | null> {
    const url = new URL(NOMINATIM_REVERSE_URL);
    url.searchParams.set("lat", String(point.lat));
    url.searchParams.set("lon", String(point.lon));
    url.searchParams.set("format", "jsonv2");

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`Nominatim reverse geocode failed: ${response.status} ${response.statusText}`);
    }

    const body: unknown = await response.json();
    return mapNominatimResults([body])[0]?.label ?? null;
  }
}
