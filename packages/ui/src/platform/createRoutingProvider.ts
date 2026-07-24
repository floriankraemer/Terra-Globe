import {
  OsrmRoutingProvider,
  createRoutingAdapter,
  type RoutingProvider,
  type RoutingProviderConfig,
  type SecretStore,
} from "@webglobe/core";

/**
 * Composition root: the only place that picks the active routing backend.
 * Everything downstream depends solely on the RoutingProvider port.
 */
export async function createRoutingProvider(
  config: RoutingProviderConfig | undefined,
  secretStore: SecretStore,
): Promise<RoutingProvider> {
  if (!config) return new OsrmRoutingProvider();
  const apiKey = (await secretStore.get(config.id)) ?? "";
  return createRoutingAdapter(config, apiKey);
}
