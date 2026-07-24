import type { RoutingPresetId } from "../ProviderConfig.js";

export interface RoutingPreset {
  id: RoutingPresetId;
  name: string;
}

export const ROUTING_PRESETS: Record<RoutingPresetId, RoutingPreset> = {
  osrm: { id: "osrm", name: "OSRM" },
  graphhopper: { id: "graphhopper", name: "GraphHopper" },
  openrouteservice: { id: "openrouteservice", name: "OpenRouteService" },
  "mapbox-directions": { id: "mapbox-directions", name: "Mapbox Directions" },
};
