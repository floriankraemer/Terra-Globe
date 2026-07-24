import {
  createCircleGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
  type GeoPoint,
  type PlacemarkGeometry,
} from "@webglobe/core";
import type { IEntityFactory } from "../entities/IEntityFactory.js";
import { circleRadiusMeters, rectangleFromCorners } from "./geometryMath.js";

type Mode = "idle" | "point" | "rectangle" | "circle" | "polygon";

export class DrawingController {
  private mode: Mode = "idle";
  private vertices: GeoPoint[] = [];

  constructor(private readonly entityFactory: IEntityFactory) {}

  get isActive(): boolean {
    return this.mode !== "idle";
  }

  startPoint(): void {
    this.beginMode("point");
  }

  startRectangle(): void {
    this.beginMode("rectangle");
  }

  startCircle(): void {
    this.beginMode("circle");
  }

  startPolygon(): void {
    this.beginMode("polygon");
  }

  private beginMode(mode: Mode): void {
    this.mode = mode;
    this.vertices = [];
  }

  /**
   * Feeds a click point into the active tool.
   * Returns the committed geometry once the shape is complete (point/rectangle/circle
   * commit on their fixed number of clicks); polygons only commit via finishPolygon().
   */
  addPoint(point: GeoPoint): PlacemarkGeometry | undefined {
    switch (this.mode) {
      case "idle":
        throw new Error("No drawing tool active; call start*() first");
      case "point":
        return this.commit(createPointGeometry(point));
      case "rectangle": {
        this.vertices.push(point);
        if (this.vertices.length < 2) return undefined;
        const [a, b] = this.vertices as [GeoPoint, GeoPoint];
        return this.commit(createRectangleGeometry(rectangleFromCorners(a, b)));
      }
      case "circle": {
        this.vertices.push(point);
        if (this.vertices.length < 2) return undefined;
        const [center, edge] = this.vertices as [GeoPoint, GeoPoint];
        return this.commit(createCircleGeometry(center, circleRadiusMeters(center, edge)));
      }
      case "polygon":
        this.vertices.push(point);
        return undefined;
    }
  }

  /**
   * Computes what the shape would look like if committed right now, without
   * mutating any state - lets the UI render a live rubber-band preview as
   * the cursor moves, without affecting the actual click sequence.
   */
  previewGeometry(cursor: GeoPoint): PlacemarkGeometry | undefined {
    try {
      switch (this.mode) {
        case "idle":
        case "point":
          return undefined;
        case "rectangle": {
          const anchor = this.vertices[0];
          if (!anchor) return undefined;
          return createRectangleGeometry(rectangleFromCorners(anchor, cursor));
        }
        case "circle": {
          const center = this.vertices[0];
          if (!center) return undefined;
          return createCircleGeometry(center, circleRadiusMeters(center, cursor));
        }
        case "polygon": {
          if (this.vertices.length < 2) return undefined;
          return createPolygonGeometry([...this.vertices, cursor]);
        }
      }
    } catch {
      // Degenerate cursor position (e.g. same point as the anchor) - no preview yet.
      return undefined;
    }
  }

  undoLastVertex(): void {
    if (this.mode !== "polygon") {
      throw new Error("undoLastVertex is only valid while drawing a polygon");
    }
    this.vertices.pop();
  }

  finishPolygon(): PlacemarkGeometry {
    if (this.mode !== "polygon") {
      throw new Error("finishPolygon called while not drawing a polygon");
    }
    return this.commit(createPolygonGeometry([...this.vertices]));
  }

  cancel(): void {
    this.mode = "idle";
    this.vertices = [];
  }

  private commit(geometry: PlacemarkGeometry): PlacemarkGeometry {
    this.entityFactory.createEntity(geometry);
    this.mode = "idle";
    this.vertices = [];
    return geometry;
  }
}
