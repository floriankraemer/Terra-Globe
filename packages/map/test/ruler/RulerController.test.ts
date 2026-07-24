import { describe, expect, it } from "vitest";
import { RulerController } from "../../src/ruler/RulerController.js";

describe("RulerController", () => {
  it("starts idle with no vertices", () => {
    const controller = new RulerController();
    expect(controller.isActive).toBe(false);
    expect(controller.vertexCount).toBe(0);
  });

  it("throws adding a point before start()", () => {
    const controller = new RulerController();
    expect(() => controller.addPoint({ lon: 0, lat: 0 })).toThrow(/not active/i);
  });

  it("accumulates vertices while active", () => {
    const controller = new RulerController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });

    expect(controller.isActive).toBe(true);
    expect(controller.vertexCount).toBe(2);
    expect(controller.getPoints()).toEqual([
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
    ]);
  });

  it("clears prior vertices when start() is called again", () => {
    const controller = new RulerController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.start();

    expect(controller.vertexCount).toBe(0);
  });

  describe("currentGeometry", () => {
    it("returns undefined with fewer than 2 vertices", () => {
      const controller = new RulerController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });

      expect(controller.currentGeometry()).toBeUndefined();
    });

    it("returns the committed path once 2+ vertices exist", () => {
      const controller = new RulerController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });
      controller.addPoint({ lon: 1, lat: 0 });

      expect(controller.currentGeometry()).toEqual({
        type: "LineString",
        path: [
          { lon: 0, lat: 0 },
          { lon: 1, lat: 0 },
        ],
      });
    });
  });

  describe("previewGeometry", () => {
    it("returns undefined when idle", () => {
      const controller = new RulerController();
      expect(controller.previewGeometry({ lon: 0, lat: 0 })).toBeUndefined();
    });

    it("returns undefined before the first vertex is clicked", () => {
      const controller = new RulerController();
      controller.start();
      expect(controller.previewGeometry({ lon: 0, lat: 0 })).toBeUndefined();
    });

    it("previews the accumulated path plus the cursor", () => {
      const controller = new RulerController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });

      expect(controller.previewGeometry({ lon: 1, lat: 0 })).toEqual({
        type: "LineString",
        path: [
          { lon: 0, lat: 0 },
          { lon: 1, lat: 0 },
        ],
      });
    });

    it("does not mutate accumulated vertices", () => {
      const controller = new RulerController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });
      controller.previewGeometry({ lon: 1, lat: 0 });

      expect(controller.vertexCount).toBe(1);
    });
  });

  it("undoLastVertex removes the most recent vertex", () => {
    const controller = new RulerController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    controller.undoLastVertex();

    expect(controller.getPoints()).toEqual([{ lon: 0, lat: 0 }]);
  });

  it("finish stops accepting vertices but keeps the accumulated path", () => {
    const controller = new RulerController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    controller.finish();

    expect(controller.isActive).toBe(false);
    expect(controller.currentGeometry()).toEqual({
      type: "LineString",
      path: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
      ],
    });
    expect(() => controller.addPoint({ lon: 2, lat: 0 })).toThrow(/not active/i);
  });

  it("cancel clears the accumulated path and deactivates", () => {
    const controller = new RulerController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.addPoint({ lon: 1, lat: 0 });
    controller.cancel();

    expect(controller.isActive).toBe(false);
    expect(controller.vertexCount).toBe(0);
  });
});
