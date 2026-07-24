import {
  NominatimGeocodingProvider,
  createGeocodingAdapter,
  type GeocodingProvider,
  type GeocodingProviderConfig,
  type SecretStore,
} from "@webglobe/core";

/**
 * Composition root: the only place that picks the active geocoding backend.
 * Everything downstream depends solely on the GeocodingProvider port.
 */
export async function createGeocodingProvider(
  config: GeocodingProviderConfig | undefined,
  secretStore: SecretStore,
): Promise<GeocodingProvider> {
  if (!config) return new NominatimGeocodingProvider();
  const apiKey = (await secretStore.get(config.id)) ?? "";
  return createGeocodingAdapter(config, apiKey);
}
