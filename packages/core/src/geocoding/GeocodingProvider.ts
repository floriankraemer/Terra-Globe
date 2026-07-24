import type { GeoPoint } from "../domain/geometry.js";

export interface GeocodeResult {
  label: string;
  point: GeoPoint;
}

/**
 * Geocoding port (dependency-inversion boundary). No adapter-specific types
 * (Nominatim response shape, API keys, ...) may leak through this interface.
 */
export interface GeocodingProvider {
  search(query: string): Promise<GeocodeResult[]>;
  /** Reverse geocode a point to a human-readable label, or null if none was found. */
  reverse(point: GeoPoint): Promise<string | null>;
}
