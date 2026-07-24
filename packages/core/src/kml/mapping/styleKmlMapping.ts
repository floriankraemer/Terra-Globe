import type { Style, StyleHighlight } from "../../domain/style.js";
import { cssColorToKmlColor, kmlColorToCssColor } from "./colorMapping.js";

export interface KmlHighlightStyleData {
  lineColorKml?: string;
  lineWidth?: number;
  polyColorKml?: string;
  iconUrl?: string;
  iconScale?: number;
  labelColorKml?: string;
}

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
  highlight?: KmlHighlightStyleData;
  balloonText?: string;
  balloonBgColorKml?: string;
  balloonTextColorKml?: string;
  balloonDisplayMode?: "default" | "hide";
  listItemType?: string;
  itemIconHref?: string;
}

function highlightToKmlData(highlight: StyleHighlight): KmlHighlightStyleData {
  return {
    lineColorKml: highlight.outlineColor ? cssColorToKmlColor(highlight.outlineColor, 1) : undefined,
    lineWidth: highlight.outlineWidth,
    polyColorKml:
      highlight.fillColor !== undefined
        ? cssColorToKmlColor(highlight.fillColor, highlight.fillOpacity ?? 1)
        : undefined,
    iconUrl: highlight.iconUrl,
    iconScale: highlight.iconScale,
    labelColorKml: highlight.labelColor ? cssColorToKmlColor(highlight.labelColor, 1) : undefined,
  };
}

function kmlDataToHighlight(data: KmlHighlightStyleData): StyleHighlight {
  const line = data.lineColorKml ? kmlColorToCssColor(data.lineColorKml) : undefined;
  const poly = data.polyColorKml ? kmlColorToCssColor(data.polyColorKml) : undefined;
  return {
    outlineColor: line?.hex,
    outlineWidth: data.lineWidth,
    fillColor: poly?.hex,
    fillOpacity: poly?.opacity,
    iconUrl: data.iconUrl,
    iconScale: data.iconScale,
    labelColor: data.labelColorKml ? kmlColorToCssColor(data.labelColorKml).hex : undefined,
  };
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
    highlight: style.highlight ? highlightToKmlData(style.highlight) : undefined,
    balloonText: style.balloonText,
    balloonBgColorKml: style.balloonBgColor ? cssColorToKmlColor(style.balloonBgColor, 1) : undefined,
    balloonTextColorKml: style.balloonTextColor
      ? cssColorToKmlColor(style.balloonTextColor, 1)
      : undefined,
    balloonDisplayMode: style.balloonDisplayMode,
    listItemType: style.listItemType,
    itemIconHref: style.itemIconHref,
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
    highlight: data.highlight ? kmlDataToHighlight(data.highlight) : undefined,
    balloonText: data.balloonText,
    balloonBgColor: data.balloonBgColorKml ? kmlColorToCssColor(data.balloonBgColorKml).hex : undefined,
    balloonTextColor: data.balloonTextColorKml
      ? kmlColorToCssColor(data.balloonTextColorKml).hex
      : undefined,
    balloonDisplayMode: data.balloonDisplayMode,
    listItemType: data.listItemType,
    itemIconHref: data.itemIconHref,
  };
}
