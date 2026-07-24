import { createLineStringGeometry, type GeoPoint, type LineStringGeometry } from "@webglobe/core";

/**
 * Open-ended vertex accumulation for the ruler tool - similar to
 * DrawingController's polygon mode, but has no entity/commit step of its own:
 * the ruler is a transient overlay, not a placemark, so entity rendering is
 * owned by the UI hook instead of this controller.
 */
export class RulerController {
  private points: GeoPoint[] = [];
  private active = false;

  get isActive(): boolean {
    return this.active;
  }

  get vertexCount(): number {
    return this.points.length;
  }

  getPoints(): GeoPoint[] {
    return [...this.points];
  }

  start(): void {
    this.active = true;
    this.points = [];
  }

  addPoint(point: GeoPoint): void {
    if (!this.active) throw new Error("Ruler is not active; call start() first");
    this.points.push(point);
  }

  /** Accumulated path plus a live cursor vertex, for a rubber-band preview segment. */
  previewGeometry(cursor: GeoPoint): LineStringGeometry | undefined {
    if (!this.active || this.points.length === 0) return undefined;
    return createLineStringGeometry([...this.points, cursor]);
  }

  /** Committed path so far (no cursor point). */
  currentGeometry(): LineStringGeometry | undefined {
    if (this.points.length < 2) return undefined;
    return createLineStringGeometry([...this.points]);
  }

  undoLastVertex(): void {
    this.points.pop();
  }

  finish(): void {
    this.active = false;
  }

  cancel(): void {
    this.active = false;
    this.points = [];
  }
}
