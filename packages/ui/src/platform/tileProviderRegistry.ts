import {
  TILE_PRESETS,
  type ProviderConfig,
  type SecretStore,
  type TileProviderConfig,
} from "@terra-globe/core";
import { BUILTIN_TILE_SOURCES, type TileSource } from "@terra-globe/map";

/**
 * Composition root: merges the built-in tile sources with the user's enabled
 * tile provider configs into the list shown in the Basemap dropdown.
 */
export async function buildTileSources(
  providers: ProviderConfig[],
  secretStore: SecretStore,
): Promise<TileSource[]> {
  const tileConfigs = providers.filter(
    (p): p is TileProviderConfig => p.kind === "tile" && p.enabled,
  );

  const custom = await Promise.all(
    tileConfigs.map(async (config): Promise<TileSource> => {
      const preset = TILE_PRESETS[config.preset];
      const apiKey = (await secretStore.get(config.id)) ?? "";
      return {
        id: config.id,
        name: config.name,
        url: preset.urlTemplate.replace("{api_key}", apiKey),
        subdomains: preset.subdomains,
        credit: preset.attribution,
        maximumLevel: preset.maxLevel,
      };
    }),
  );

  return [...BUILTIN_TILE_SOURCES, ...custom];
}
