import type { TilePresetId } from "../ProviderConfig.js";

export interface TilePreset {
  id: TilePresetId;
  name: string;
  urlTemplate: string;
  subdomains?: string[];
  maxLevel: number;
  attribution: string;
}

export const TILE_PRESETS: Record<TilePresetId, TilePreset> = {
  "mapbox-streets": {
    id: "mapbox-streets",
    name: "Mapbox Streets",
    urlTemplate:
      "https://api.mapbox.com/styles/v1/mapbox/streets-v12/tiles/{z}/{x}/{y}?access_token={api_key}",
    maxLevel: 20,
    attribution: "© Mapbox © OpenStreetMap",
  },
  "maptiler-streets": {
    id: "maptiler-streets",
    name: "MapTiler Streets",
    urlTemplate: "https://api.maptiler.com/maps/streets-v2/{z}/{x}/{y}.png?key={api_key}",
    maxLevel: 20,
    attribution: "© MapTiler © OpenStreetMap",
  },
  "thunderforest-cycle": {
    id: "thunderforest-cycle",
    name: "Thunderforest Cycle",
    urlTemplate: "https://tile.thunderforest.com/cycle/{z}/{x}/{y}.png?apikey={api_key}",
    maxLevel: 22,
    attribution: "© Thunderforest, © OpenStreetMap",
  },
};
