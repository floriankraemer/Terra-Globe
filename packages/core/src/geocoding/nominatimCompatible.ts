import type { GeocodeResult } from "./GeocodingProvider.js";

export interface NominatimResult {
  display_name: string;
  lat: string;
  lon: string;
}

export function isNominatimResult(value: unknown): value is NominatimResult {
  if (typeof value !== "object" || value === null) return false;
  const candidate = value as Partial<NominatimResult>;
  return (
    typeof candidate.display_name === "string" &&
    typeof candidate.lat === "string" &&
    typeof candidate.lon === "string"
  );
}

/** Shared response mapping for Nominatim and Nominatim-compatible search APIs (e.g. LocationIQ). */
export function mapNominatimResults(body: unknown): GeocodeResult[] {
  if (!Array.isArray(body)) return [];

  return body.filter(isNominatimResult).map((result) => ({
    label: result.display_name,
    point: { lon: Number(result.lon), lat: Number(result.lat) },
  }));
}
