import type { GeocodeResult, GeocodingProvider } from "./GeocodingProvider.js";

const OPENCAGE_SEARCH_URL = "https://api.opencagedata.com/geocode/v1/json";

interface OpenCageResult {
  formatted: string;
  geometry: { lat: number; lng: number };
}

function isOpenCageResult(value: unknown): value is OpenCageResult {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<OpenCageResult>;
  if (typeof candidate.formatted !== "string") return false;
  if (typeof candidate.geometry !== "object" || candidate.geometry === null) return false;
  const geometry = candidate.geometry as Partial<OpenCageResult["geometry"]>;
  return typeof geometry.lat === "number" && typeof geometry.lng === "number";
}

export function mapOpenCageResults(body: unknown): GeocodeResult[] {
  if (typeof body !== "object" || body === null) return [];
  const results = (body as { results?: unknown }).results;
  if (!Array.isArray(results)) return [];

  return results.filter(isOpenCageResult).map((result) => ({
    label: result.formatted,
    point: { lon: result.geometry.lng, lat: result.geometry.lat },
  }));
}

/** OpenCage Geocoder adapter, requires an API key. */
export class OpenCageGeocodingProvider implements GeocodingProvider {
  constructor(
    private readonly apiKey: string,
    private readonly fetchImpl: typeof fetch = fetch.bind(globalThis),
  ) {}

  async search(query: string): Promise<GeocodeResult[]> {
    const trimmed = query.trim();
    if (trimmed.length === 0) return [];

    const url = new URL(OPENCAGE_SEARCH_URL);
    url.searchParams.set("q", trimmed);
    url.searchParams.set("key", this.apiKey);

    const response = await this.fetchImpl(url, { headers: { Accept: "application/json" } });
    if (!response.ok) {
      throw new Error(`OpenCage search failed: ${response.status} ${response.statusText}`);
    }

    return mapOpenCageResults(await response.json());
  }
}
