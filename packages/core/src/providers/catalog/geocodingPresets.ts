import type { GeocodingPresetId } from "../ProviderConfig.js";

export interface GeocodingPreset {
  id: GeocodingPresetId;
  name: string;
}

export const GEOCODING_PRESETS: Record<GeocodingPresetId, GeocodingPreset> = {
  locationiq: { id: "locationiq", name: "LocationIQ" },
  "mapbox-geocoding": { id: "mapbox-geocoding", name: "Mapbox Geocoding" },
  opencage: { id: "opencage", name: "OpenCage" },
};
