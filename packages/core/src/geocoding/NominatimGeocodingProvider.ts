import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";
import { mapNominatimResults } from "./nominatimCompatible.js";

const NOMINATIM_SEARCH_URL = "https://nominatim.openstreetmap.org/search";

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
}
