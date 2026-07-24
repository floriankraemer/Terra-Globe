import type { GeoPoint } from "@webglobe/core";

export interface ScreenPosition {
  x: number;
  y: number;
}

/** Seam converting a screen click into a ground GeoPoint. */
export interface IScreenPicker {
  pickGround(position: ScreenPosition): GeoPoint | undefined;
}
