import type { GeoPoint } from "../domain/geometry.js";
import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";

function pointKey(point: GeoPoint): string {
  return `${point.lat},${point.lon}`;
}

/** In-memory test double: returns canned results keyed by exact query string / point. */
export class FakeGeocodingProvider implements GeocodingProvider {
  constructor(
    private readonly resultsByQuery: Record<string, GeocodeResult[]> = {},
    private readonly labelsByPoint: Record<string, string> = {},
  ) {}

  async search(query: string): Promise<GeocodeResult[]> {
    return this.resultsByQuery[query] ?? [];
  }

  async reverse(point: GeoPoint): Promise<string | null> {
    return this.labelsByPoint[pointKey(point)] ?? null;
  }
}
