function clampByte(value: number): number {
  return Math.max(0, Math.min(255, Math.round(value)));
}

function toHexByte(value: number): string {
  return clampByte(value).toString(16).padStart(2, "0");
}

/** CSS "#rrggbb" + opacity [0,1] -> KML color "aabbggrr". */
export function cssColorToKmlColor(cssHex: string, opacity: number): string {
  const r = cssHex.slice(1, 3);
  const g = cssHex.slice(3, 5);
  const b = cssHex.slice(5, 7);
  const a = toHexByte(opacity * 255);
  return `${a}${b}${g}${r}`.toLowerCase();
}

/** KML color "aabbggrr" -> CSS "#rrggbb" + opacity [0,1]. */
export function kmlColorToCssColor(kmlColor: string): { hex: string; opacity: number } {
  const a = kmlColor.slice(0, 2);
  const b = kmlColor.slice(2, 4);
  const g = kmlColor.slice(4, 6);
  const r = kmlColor.slice(6, 8);
  return {
    hex: `#${r}${g}${b}`.toLowerCase(),
    opacity: parseInt(a, 16) / 255,
  };
}
