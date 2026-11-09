import {
  createRectangleGeometry,
  type GeoPoint,
  type RectangleBounds,
  type RectangleGeometry,
} from "@terra-globe/core";
import { rectangleFromCorners } from "./geometryMath.js";

/**
 * Two-click rectangle selection for the area-export tool - mirrors
 * DrawingController's rectangle mode, but is a standalone controller since
 * area export isn't a placemark drawing tool.
 */
export class AreaSelectController {
  private anchor: GeoPoint | undefined;
  private active = false;

  get isActive(): boolean {
    return this.active;
  }

  start(): void {
    this.active = true;
    this.anchor = undefined;
  }

  addPoint(point: GeoPoint): RectangleBounds | undefined {
    if (!this.active) throw new Error("Area select is not active; call start() first");
    if (!this.anchor) {
      this.anchor = point;
      return undefined;
    }
    const bounds = rectangleFromCorners(this.anchor, point);
    this.active = false;
    this.anchor = undefined;
    return bounds;
  }

  /** Accumulated anchor plus a live cursor corner, for a rubber-band preview rectangle. */
  previewGeometry(cursor: GeoPoint): RectangleGeometry | undefined {
    if (!this.active || !this.anchor) return undefined;
    return createRectangleGeometry(rectangleFromCorners(this.anchor, cursor));
  }

  cancel(): void {
    this.active = false;
    this.anchor = undefined;
  }
}
