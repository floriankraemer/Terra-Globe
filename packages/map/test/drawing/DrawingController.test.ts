import { describe, expect, it } from "vitest";
import { FakeEntityFactory } from "../../src/entities/FakeEntityFactory.js";
import { DrawingController } from "../../src/drawing/DrawingController.js";

describe("DrawingController - point", () => {
  it("commits a point immediately on a single click", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startPoint();
    const geometry = controller.addPoint({ lon: 1, lat: 2 });

    expect(geometry).toEqual({ type: "Point", coordinates: { lon: 1, lat: 2 } });
    expect(factory.entities.size).toBe(1);
    expect(controller.isActive).toBe(false);
  });
});

describe("DrawingController - rectangle", () => {
  it("commits a rectangle after two corner clicks", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startRectangle();
    expect(controller.addPoint({ lon: 0, lat: 0 })).toBeUndefined();
    const geometry = controller.addPoint({ lon: 10, lat: 10 });

    expect(geometry).toEqual({ type: "Rectangle", north: 10, south: 0, east: 10, west: 0 });
    expect(controller.isActive).toBe(false);
  });
});

describe("DrawingController - circle", () => {
  it("commits a circle after center + edge clicks", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startCircle();
    expect(controller.addPoint({ lon: 0, lat: 0 })).toBeUndefined();
    const geometry = controller.addPoint({ lon: 0, lat: 1 });

    expect(geometry?.type).toBe("Circle");
    expect(controller.isActive).toBe(false);
  });
});

describe("DrawingController - polygon", () => {
  it("accumulates vertices and commits only on finishPolygon", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startPolygon();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    controller.addPoint({ lon: 0, lat: 1 });
    expect(controller.isActive).toBe(true);
    const geometry = controller.finishPolygon();

    expect(geometry).toEqual({
      type: "Polygon",
      outerRing: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
        { lon: 0, lat: 1 },
      ],
      innerRings: undefined,
    });
  });

  it("supports undoing the last vertex", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startPolygon();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 5, lat: 5 });
    controller.undoLastVertex();
    controller.addPoint({ lon: 1, lat: 0 });
    controller.addPoint({ lon: 0, lat: 1 });
    const geometry = controller.finishPolygon();

    expect(geometry?.outerRing).toEqual([
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
      { lon: 0, lat: 1 },
    ]);
  });

  it("throws finishing a polygon with fewer than 3 vertices", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startPolygon();
    controller.addPoint({ lon: 0, lat: 0 });
    expect(() => controller.finishPolygon()).toThrow(/at least 3/i);
  });

  it("cancel discards the in-progress shape without creating an entity", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startPolygon();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    controller.cancel();

    expect(controller.isActive).toBe(false);
    expect(factory.entities.size).toBe(0);
  });
});

describe("DrawingController - line", () => {
  it("accumulates vertices and commits only on finishLine", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startLine();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    expect(controller.isActive).toBe(true);
    const geometry = controller.finishLine();

    expect(geometry).toEqual({
      type: "LineString",
      path: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
      ],
    });
    expect(controller.isActive).toBe(false);
  });

  it("supports undoing the last vertex", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startLine();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 5, lat: 5 });
    controller.undoLastVertex();
    controller.addPoint({ lon: 1, lat: 0 });
    const geometry = controller.finishLine();

    expect(geometry?.type).toBe("LineString");
    expect((geometry as { path: unknown }).path).toEqual([
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
    ]);
  });

  it("throws finishing a line with fewer than 2 vertices", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startLine();
    controller.addPoint({ lon: 0, lat: 0 });
    expect(() => controller.finishLine()).toThrow(/at least 2/i);
  });

  it("cancel discards the in-progress line without creating an entity", () => {
    const factory = new FakeEntityFactory();
    const controller = new DrawingController(factory);

    controller.startLine();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    controller.cancel();

    expect(controller.isActive).toBe(false);
    expect(factory.entities.size).toBe(0);
  });
});

describe("DrawingController - previewGeometry", () => {
  it("returns undefined when idle", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    expect(controller.previewGeometry({ lon: 0, lat: 0 })).toBeUndefined();
  });

  it("returns undefined for point mode (commits immediately, no preview needed)", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startPoint();
    expect(controller.previewGeometry({ lon: 0, lat: 0 })).toBeUndefined();
  });

  it("returns undefined for rectangle before the first corner is clicked", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startRectangle();
    expect(controller.previewGeometry({ lon: 5, lat: 5 })).toBeUndefined();
  });

  it("previews a rectangle from the first corner to the cursor", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startRectangle();
    controller.addPoint({ lon: 0, lat: 0 });

    const preview = controller.previewGeometry({ lon: 10, lat: 10 });

    expect(preview).toEqual({ type: "Rectangle", north: 10, south: 0, east: 10, west: 0 });
  });

  it("does not mutate state when previewing a rectangle", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startRectangle();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.previewGeometry({ lon: 10, lat: 10 });

    // Second real click should still treat this as the second corner, not a third.
    const geometry = controller.addPoint({ lon: 4, lat: 4 });
    expect(geometry).toEqual({ type: "Rectangle", north: 4, south: 0, east: 4, west: 0 });
  });

  it("returns undefined for a degenerate (zero-area) rectangle preview", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startRectangle();
    controller.addPoint({ lon: 5, lat: 5 });

    expect(controller.previewGeometry({ lon: 5, lat: 5 })).toBeUndefined();
  });

  it("previews a circle from the center to the cursor", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startCircle();
    controller.addPoint({ lon: 0, lat: 0 });

    const preview = controller.previewGeometry({ lon: 0, lat: 1 });

    expect(preview?.type).toBe("Circle");
  });

  it("returns undefined for a degenerate (zero-radius) circle preview", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startCircle();
    controller.addPoint({ lon: 5, lat: 5 });

    expect(controller.previewGeometry({ lon: 5, lat: 5 })).toBeUndefined();
  });

  it("returns undefined for polygon preview with fewer than 2 accumulated vertices", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startPolygon();
    controller.addPoint({ lon: 0, lat: 0 });

    expect(controller.previewGeometry({ lon: 1, lat: 1 })).toBeUndefined();
  });

  it("previews a polygon as the accumulated vertices plus the cursor", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startPolygon();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });

    const preview = controller.previewGeometry({ lon: 0, lat: 1 });

    expect(preview).toEqual({
      type: "Polygon",
      outerRing: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
        { lon: 0, lat: 1 },
      ],
      innerRings: undefined,
    });
  });

  it("returns undefined for line preview before any vertex is clicked", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startLine();

    expect(controller.previewGeometry({ lon: 1, lat: 1 })).toBeUndefined();
  });

  it("previews a line as the accumulated vertices plus the cursor", () => {
    const controller = new DrawingController(new FakeEntityFactory());
    controller.startLine();
    controller.addPoint({ lon: 0, lat: 0 });

    const preview = controller.previewGeometry({ lon: 1, lat: 0 });

    expect(preview).toEqual({
      type: "LineString",
      path: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
      ],
    });
  });
});
