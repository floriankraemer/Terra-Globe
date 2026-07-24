export interface TileSource {
  id: string;
  name: string;
  url: string;
  subdomains?: string[];
  credit: string;
  maximumLevel: number;
}

export function openStreetMapSource(): TileSource {
  return {
    id: "osm",
    name: "OpenStreetMap",
    url: "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
    credit: "© OpenStreetMap contributors",
    maximumLevel: 19,
  };
}

export function openTopoMapSource(): TileSource {
  return {
    id: "opentopomap",
    name: "OpenTopoMap",
    url: "https://{s}.tile.opentopomap.org/{z}/{x}/{y}.png",
    subdomains: ["a", "b", "c"],
    credit: "© OpenStreetMap contributors, SRTM | © OpenTopoMap (CC-BY-SA)",
    maximumLevel: 17,
  };
}

export const BUILTIN_TILE_SOURCES: TileSource[] = [openStreetMapSource(), openTopoMapSource()];
