export type TilePresetId = "mapbox-streets" | "maptiler-streets" | "thunderforest-cycle";
export type GeocodingPresetId = "locationiq" | "mapbox-geocoding" | "opencage";
export type RoutingPresetId = "osrm" | "graphhopper" | "openrouteservice" | "mapbox-directions";

export interface TileProviderConfig {
  id: string;
  kind: "tile";
  preset: TilePresetId;
  name: string;
  enabled: boolean;
}

export interface GeocodingProviderConfig {
  id: string;
  kind: "geocoding";
  preset: GeocodingPresetId;
  name: string;
  enabled: boolean;
}

export interface RoutingProviderConfig {
  id: string;
  kind: "routing";
  preset: RoutingPresetId;
  name: string;
  enabled: boolean;
}

export type ProviderConfig = TileProviderConfig | GeocodingProviderConfig | RoutingProviderConfig;
