import type { Style } from "../../domain/style.js";
import { cssColorToKmlColor, kmlColorToCssColor } from "./colorMapping.js";

export interface KmlStyleData {
  id: string;
  lineColorKml: string;
  lineWidth: number;
  polyColorKml: string;
  outlineEnabled: boolean;
  filled: boolean;
  iconUrl?: string;
  iconScale?: number;
  labelColorKml?: string;
}

export function styleToKmlData(style: Style): KmlStyleData {
  return {
    id: style.id,
    lineColorKml: cssColorToKmlColor(style.outlineColor, 1),
    lineWidth: style.outlineWidth,
    polyColorKml: cssColorToKmlColor(style.fillColor, style.fillOpacity),
    outlineEnabled: style.outlineEnabled,
    filled: style.filled,
    iconUrl: style.iconUrl,
    iconScale: style.iconScale,
    labelColorKml: style.labelColor ? cssColorToKmlColor(style.labelColor, 1) : undefined,
  };
}

export function kmlDataToStyle(data: KmlStyleData): Style {
  const line = kmlColorToCssColor(data.lineColorKml);
  const poly = kmlColorToCssColor(data.polyColorKml);
  return {
    id: data.id,
    outlineColor: line.hex,
    outlineWidth: data.lineWidth,
    fillColor: poly.hex,
    fillOpacity: poly.opacity,
    outlineEnabled: data.outlineEnabled,
    filled: data.filled,
    iconUrl: data.iconUrl,
    iconScale: data.iconScale,
    labelColor: data.labelColorKml ? kmlColorToCssColor(data.labelColorKml).hex : undefined,
  };
}
