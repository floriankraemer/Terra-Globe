import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";

/** In-memory test double: returns canned results keyed by exact query string. */
export class FakeGeocodingProvider implements GeocodingProvider {
  constructor(private readonly resultsByQuery: Record<string, GeocodeResult[]> = {}) {}

  async search(query: string): Promise<GeocodeResult[]> {
    return this.resultsByQuery[query] ?? [];
  }
}
