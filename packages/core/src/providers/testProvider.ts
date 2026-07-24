import { LocationIqGeocodingProvider } from "../geocoding/LocationIqGeocodingProvider.js";
import { MapboxGeocodingProvider } from "../geocoding/MapboxGeocodingProvider.js";
import { OpenCageGeocodingProvider } from "../geocoding/OpenCageGeocodingProvider.js";
import type { GeocodingProvider } from "../geocoding/GeocodingProvider.js";
import { TILE_PRESETS } from "./catalog/tilePresets.js";
import type { GeocodingProviderConfig, TileProviderConfig } from "./ProviderConfig.js";

export interface ProviderTestResult {
  ok: boolean;
  error?: string;
}

export function createGeocodingAdapter(
  config: GeocodingProviderConfig,
  apiKey: string,
  fetchImpl?: typeof fetch,
): GeocodingProvider {
  switch (config.preset) {
    case "locationiq":
      return new LocationIqGeocodingProvider(apiKey, fetchImpl);
    case "mapbox-geocoding":
      return new MapboxGeocodingProvider(apiKey, fetchImpl);
    case "opencage":
      return new OpenCageGeocodingProvider(apiKey, fetchImpl);
  }
}

export async function testTileProviderConfig(
  config: TileProviderConfig,
  apiKey: string,
  fetchImpl: typeof fetch = fetch.bind(globalThis),
): Promise<ProviderTestResult> {
  const preset = TILE_PRESETS[config.preset];
  const sampleUrl = preset.urlTemplate
    .replace("{z}", "0")
    .replace("{x}", "0")
    .replace("{y}", "0")
    .replace("{s}", preset.subdomains?.[0] ?? "a")
    .replace("{api_key}", apiKey);

  try {
    const response = await fetchImpl(sampleUrl);
    if (!response.ok) return { ok: false, error: `HTTP ${response.status}` };
    const contentType = response.headers.get("content-type") ?? "";
    if (!contentType.startsWith("image/")) {
      return { ok: false, error: `Unexpected content-type: ${contentType || "none"}` };
    }
    return { ok: true };
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}

export async function testGeocodingProviderConfig(
  config: GeocodingProviderConfig,
  apiKey: string,
  fetchImpl: typeof fetch = fetch.bind(globalThis),
): Promise<ProviderTestResult> {
  try {
    await createGeocodingAdapter(config, apiKey, fetchImpl).search("Berlin");
    return { ok: true };
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}
