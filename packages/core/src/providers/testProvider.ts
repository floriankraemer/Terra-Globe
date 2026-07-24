import { LocationIqGeocodingProvider } from "../geocoding/LocationIqGeocodingProvider.js";
import { MapboxGeocodingProvider } from "../geocoding/MapboxGeocodingProvider.js";
import { OpenCageGeocodingProvider } from "../geocoding/OpenCageGeocodingProvider.js";
import type { GeocodingProvider } from "../geocoding/GeocodingProvider.js";
import { OsrmRoutingProvider } from "../routing/OsrmRoutingProvider.js";
import { GraphHopperRoutingProvider } from "../routing/GraphHopperRoutingProvider.js";
import { OpenRouteServiceRoutingProvider } from "../routing/OpenRouteServiceRoutingProvider.js";
import { MapboxRoutingProvider } from "../routing/MapboxRoutingProvider.js";
import type { RoutingProvider } from "../routing/RoutingProvider.js";
import { TILE_PRESETS } from "./catalog/tilePresets.js";
import type { GeocodingProviderConfig, RoutingProviderConfig, TileProviderConfig } from "./ProviderConfig.js";

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

export function createRoutingAdapter(
  config: RoutingProviderConfig,
  apiKey: string,
  fetchImpl?: typeof fetch,
): RoutingProvider {
  switch (config.preset) {
    case "osrm":
      return new OsrmRoutingProvider(undefined, fetchImpl);
    case "graphhopper":
      return new GraphHopperRoutingProvider(apiKey, fetchImpl);
    case "openrouteservice":
      return new OpenRouteServiceRoutingProvider(apiKey, fetchImpl);
    case "mapbox-directions":
      return new MapboxRoutingProvider(apiKey, fetchImpl);
  }
}

const TEST_ROUTE_WAYPOINTS = [
  { lon: 13.404954, lat: 52.5200066 },
  { lon: 13.42, lat: 52.51 },
];

export async function testRoutingProviderConfig(
  config: RoutingProviderConfig,
  apiKey: string,
  fetchImpl: typeof fetch = fetch.bind(globalThis),
): Promise<ProviderTestResult> {
  try {
    await createRoutingAdapter(config, apiKey, fetchImpl).route(TEST_ROUTE_WAYPOINTS, "car");
    return { ok: true };
  } catch (err) {
    return { ok: false, error: err instanceof Error ? err.message : String(err) };
  }
}
