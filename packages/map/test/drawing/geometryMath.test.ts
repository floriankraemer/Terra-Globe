import { describe, expect, it } from "vitest";
import { circleRadiusMeters, rectangleFromCorners } from "../../src/drawing/geometryMath.js";

describe("rectangleFromCorners", () => {
  it("normalizes two opposite corners into north/south/east/west bounds", () => {
    const bounds = rectangleFromCorners({ lon: 10, lat: 0 }, { lon: 0, lat: 10 });
    expect(bounds).toEqual({ north: 10, south: 0, east: 10, west: 0 });
  });

  it("works regardless of which corner is passed first", () => {
    const a = rectangleFromCorners({ lon: 0, lat: 0 }, { lon: 10, lat: 10 });
    const b = rectangleFromCorners({ lon: 10, lat: 10 }, { lon: 0, lat: 0 });
    expect(a).toEqual(b);
  });
});

describe("circleRadiusMeters", () => {
  it("computes ~0 for identical points", () => {
    const radius = circleRadiusMeters({ lon: 0, lat: 0 }, { lon: 0, lat: 0 });
    expect(radius).toBeCloseTo(0, 3);
  });

  it("computes the great-circle distance between center and edge", () => {
    // 1 degree of latitude is ~111.2 km
    const radius = circleRadiusMeters({ lon: 0, lat: 0 }, { lon: 0, lat: 1 });
    expect(radius).toBeCloseTo(111_195, -2);
  });
});
