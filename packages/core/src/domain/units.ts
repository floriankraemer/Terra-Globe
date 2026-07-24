import type { GeoPoint } from "./geometry.js";

export type UnitSystem = "metric" | "imperial";
export type CoordinateFormat = "decimal" | "dms";

const METERS_PER_FOOT = 0.3048;
const METERS_PER_MILE = 1609.344;
const SQUARE_METERS_PER_SQUARE_FOOT = 0.09290304;
const SQUARE_METERS_PER_ACRE = 4046.8564224;
const SQUARE_METERS_PER_HECTARE = 10_000;
const SQUARE_METERS_PER_SQUARE_KM = 1_000_000;
const SQUARE_FEET_PER_ACRE = 43_560;
const ACRES_PER_SQUARE_MILE = 640;

export function formatDistance(meters: number, system: UnitSystem): string {
  if (system === "imperial") {
    if (meters < METERS_PER_MILE) return `${Math.round(meters / METERS_PER_FOOT)} ft`;
    return `${(meters / METERS_PER_MILE).toFixed(2)} mi`;
  }
  if (meters < 1000) return `${Math.round(meters)} m`;
  return `${(meters / 1000).toFixed(2)} km`;
}

export function formatArea(squareMeters: number, system: UnitSystem): string {
  if (system === "imperial") {
    const squareFeet = squareMeters / SQUARE_METERS_PER_SQUARE_FOOT;
    if (squareFeet < SQUARE_FEET_PER_ACRE) return `${Math.round(squareFeet)} sq ft`;
    const acres = squareMeters / SQUARE_METERS_PER_ACRE;
    if (acres < ACRES_PER_SQUARE_MILE) return `${acres.toFixed(2)} acres`;
    return `${(acres / ACRES_PER_SQUARE_MILE).toFixed(2)} sq mi`;
  }
  if (squareMeters < SQUARE_METERS_PER_HECTARE) return `${Math.round(squareMeters)} m²`;
  if (squareMeters < SQUARE_METERS_PER_SQUARE_KM) {
    return `${(squareMeters / SQUARE_METERS_PER_HECTARE).toFixed(2)} ha`;
  }
  return `${(squareMeters / SQUARE_METERS_PER_SQUARE_KM).toFixed(2)} km²`;
}

function toDms(value: number, positiveLetter: string, negativeLetter: string): string {
  const hemisphere = value >= 0 ? positiveLetter : negativeLetter;
  const abs = Math.abs(value);
  const degrees = Math.floor(abs);
  const minutesFull = (abs - degrees) * 60;
  const minutes = Math.floor(minutesFull);
  const seconds = (minutesFull - minutes) * 60;
  return `${degrees}°${minutes}'${seconds.toFixed(2)}"${hemisphere}`;
}

export function formatCoordinate(point: GeoPoint, format: CoordinateFormat): string {
  if (format === "dms") {
    return `${toDms(point.lat, "N", "S")}, ${toDms(point.lon, "E", "W")}`;
  }
  return `${point.lat.toFixed(5)}, ${point.lon.toFixed(5)}`;
}
