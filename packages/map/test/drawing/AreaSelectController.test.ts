import { describe, expect, it } from "vitest";
import { AreaSelectController } from "../../src/drawing/AreaSelectController.js";

describe("AreaSelectController", () => {
  it("starts idle", () => {
    const controller = new AreaSelectController();
    expect(controller.isActive).toBe(false);
  });

  it("throws adding a point before start()", () => {
    const controller = new AreaSelectController();
    expect(() => controller.addPoint({ lon: 0, lat: 0 })).toThrow(/not active/i);
  });

  it("activates on start()", () => {
    const controller = new AreaSelectController();
    controller.start();
    expect(controller.isActive).toBe(true);
  });

  it("clears a prior anchor when start() is called again", () => {
    const controller = new AreaSelectController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.start();

    expect(controller.previewGeometry({ lon: 1, lat: 1 })).toBeUndefined();
  });

  describe("previewGeometry", () => {
    it("returns undefined when idle", () => {
      const controller = new AreaSelectController();
      expect(controller.previewGeometry({ lon: 0, lat: 0 })).toBeUndefined();
    });

    it("returns undefined before the anchor is set", () => {
      const controller = new AreaSelectController();
      controller.start();
      expect(controller.previewGeometry({ lon: 0, lat: 0 })).toBeUndefined();
    });

    it("previews the rectangle spanned by the anchor and the cursor", () => {
      const controller = new AreaSelectController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });

      expect(controller.previewGeometry({ lon: 1, lat: 2 })).toEqual({
        type: "Rectangle",
        north: 2,
        south: 0,
        east: 1,
        west: 0,
      });
    });
  });

  describe("addPoint", () => {
    it("records the anchor on the first call and returns undefined", () => {
      const controller = new AreaSelectController();
      controller.start();

      expect(controller.addPoint({ lon: 0, lat: 0 })).toBeUndefined();
      expect(controller.isActive).toBe(true);
    });

    it("commits bounds and deactivates on the second call", () => {
      const controller = new AreaSelectController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });

      const bounds = controller.addPoint({ lon: 1, lat: 2 });

      expect(bounds).toEqual({ north: 2, south: 0, east: 1, west: 0 });
      expect(controller.isActive).toBe(false);
    });

    it("throws when called again after committing", () => {
      const controller = new AreaSelectController();
      controller.start();
      controller.addPoint({ lon: 0, lat: 0 });
      controller.addPoint({ lon: 1, lat: 2 });

      expect(() => controller.addPoint({ lon: 3, lat: 3 })).toThrow(/not active/i);
    });
  });

  it("cancel clears the anchor and deactivates", () => {
    const controller = new AreaSelectController();
    controller.start();
    controller.addPoint({ lon: 0, lat: 0 });
    controller.cancel();

    expect(controller.isActive).toBe(false);
    expect(controller.previewGeometry({ lon: 1, lat: 1 })).toBeUndefined();
  });
});
