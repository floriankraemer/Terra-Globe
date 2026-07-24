import { generateId } from "./ids.js";

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
