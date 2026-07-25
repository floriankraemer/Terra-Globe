import { generateId } from "./ids.js";

/** Built-in marker icon choices for Point placemarks (sourced from Lucide, see markerIcons.ts). */
export const MARKER_ICON_IDS = [
  "map-pin",
  "pin",
  "flag",
  "star",
  "landmark",
  "navigation",
] as const;
export type MarkerIconId = (typeof MARKER_ICON_IDS)[number];
export const DEFAULT_MARKER_ICON: MarkerIconId = "map-pin";

export interface StyleHighlight {
  outlineColor?: string;
  outlineWidth?: number;
  fillColor?: string;
  fillOpacity?: number;
  iconUrl?: string;
  iconScale?: number;
  labelColor?: string;
}

export interface Style {
  id: string;
  outlineEnabled: boolean;
  outlineColor: string;
  outlineWidth: number;
  filled: boolean;
  fillColor: string;
  fillOpacity: number;
  iconUrl?: string;
  iconScale?: number;
  /** Built-in marker icon for Point placemarks. Ignored when iconUrl (a KML custom icon) is set. */
  markerIcon?: MarkerIconId;
  labelColor?: string;
  /** From a KML StyleMap's "highlight" pair. Rendering always uses the normal fields above. */
  highlight?: StyleHighlight;
  balloonText?: string;
  balloonBgColor?: string;
  balloonTextColor?: string;
  balloonDisplayMode?: "default" | "hide";
  listItemType?: string;
  itemIconHref?: string;
}

export interface NewStyle {
  outlineColor: string;
  outlineWidth: number;
  fillColor: string;
  fillOpacity: number;
  outlineEnabled?: boolean;
  filled?: boolean;
  iconUrl?: string;
  iconScale?: number;
  markerIcon?: MarkerIconId;
  labelColor?: string;
  highlight?: StyleHighlight;
  balloonText?: string;
  balloonBgColor?: string;
  balloonTextColor?: string;
  balloonDisplayMode?: "default" | "hide";
  listItemType?: string;
  itemIconHref?: string;
}

export function createStyle(input: NewStyle): Style {
  if (!Number.isFinite(input.outlineWidth) || input.outlineWidth < 0) {
    throw new Error(`outlineWidth must be a non-negative number, got ${input.outlineWidth}`);
  }
  if (!Number.isFinite(input.fillOpacity) || input.fillOpacity < 0 || input.fillOpacity > 1) {
    throw new Error(`fillOpacity must be in range [0, 1], got ${input.fillOpacity}`);
  }
  return {
    id: generateId(),
    ...input,
    outlineEnabled: input.outlineEnabled ?? true,
    filled: input.filled ?? false,
  };
}
