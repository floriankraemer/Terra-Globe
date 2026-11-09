import { describe, expect, it } from "vitest";
import {
  createCircleGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
} from "../../src/domain/geometry.js";
import {
  circleAreaSquareMeters,
  circleCircumferenceMeters,
  polygonAreaSquareMeters,
  rectangleAreaSquareMeters,
  rectangleHeightMeters,
  rectangleWidthMeters,
} from "../../src/domain/geometryMeasurements.js";

describe("circleAreaSquareMeters", () => {
  it("computes pi * r^2", () => {
    const geometry = createCircleGeometry({ lon: 0, lat: 0 }, 1000);
    expect(circleAreaSquareMeters(geometry)).toBeCloseTo(Math.PI * 1000 * 1000, 0);
  });
});

describe("circleCircumferenceMeters", () => {
  it("computes 2 * pi * r", () => {
    const geometry = createCircleGeometry({ lon: 0, lat: 0 }, 1000);
    expect(circleCircumferenceMeters(geometry)).toBeCloseTo(2 * Math.PI * 1000, 0);
  });
});

describe("rectangleAreaSquareMeters", () => {
  it("returns a positive area for a small rectangle at the equator", () => {
    const geometry = createRectangleGeometry({ north: 1, south: 0, east: 1, west: 0 });
    const area = rectangleAreaSquareMeters(geometry);
    // ~1 degree square at the equator is roughly 111km x 111km.
    expect(area).toBeGreaterThan(1e10);
    expect(area).toBeLessThan(1.3e10);
  });

  it("shrinks with latitude (a degree of longitude is shorter near the poles)", () => {
    const equatorBox = rectangleAreaSquareMeters(
      createRectangleGeometry({ north: 1, south: 0, east: 1, west: 0 }),
    );
    const highLatBox = rectangleAreaSquareMeters(
      createRectangleGeometry({ north: 81, south: 80, east: 1, west: 0 }),
    );
    expect(highLatBox).toBeLessThan(equatorBox);
  });

  it("agrees with the general polygon formula for the same four corners", () => {
    const bounds = { north: 10, south: 0, east: 10, west: 0 };
    const rectangle = createRectangleGeometry(bounds);
    const asPolygon = createPolygonGeometry([
      { lon: bounds.west, lat: bounds.south },
      { lon: bounds.east, lat: bounds.south },
      { lon: bounds.east, lat: bounds.north },
      { lon: bounds.west, lat: bounds.north },
    ]);
    expect(rectangleAreaSquareMeters(rectangle)).toBeCloseTo(
      polygonAreaSquareMeters(asPolygon),
      -2,
    );
  });
});

describe("rectangleHeightMeters", () => {
  it("matches the exact meridian-arc distance (a degree of latitude is ~111.2km)", () => {
    const geometry = createRectangleGeometry({ north: 1, south: 0, east: 1, west: 0 });
    const expected = (Math.PI / 180) * 6_371_000;
    expect(rectangleHeightMeters(geometry)).toBeCloseTo(expected, -1);
  });

  it("is unaffected by longitude span", () => {
    const narrow = rectangleHeightMeters(
      createRectangleGeometry({ north: 10, south: 0, east: 1, west: 0 }),
    );
    const wide = rectangleHeightMeters(
      createRectangleGeometry({ north: 10, south: 0, east: 90, west: 0 }),
    );
    expect(narrow).toBeCloseTo(wide, -1);
  });
});

describe("rectangleWidthMeters", () => {
  it("returns a positive width for a rectangle at the equator", () => {
    const geometry = createRectangleGeometry({ north: 1, south: 0, east: 1, west: 0 });
    // 1 degree of longitude at the equator is ~111.3km.
    expect(rectangleWidthMeters(geometry)).toBeCloseTo(111_320, -3);
  });

  it("shrinks with latitude (a degree of longitude is shorter near the poles)", () => {
    const equatorWidth = rectangleWidthMeters(
      createRectangleGeometry({ north: 1, south: 0, east: 1, west: 0 }),
    );
    const highLatWidth = rectangleWidthMeters(
      createRectangleGeometry({ north: 81, south: 80, east: 1, west: 0 }),
    );
    expect(highLatWidth).toBeLessThan(equatorWidth);
  });
});

describe("polygonAreaSquareMeters", () => {
  it("returns 0 for a degenerate (zero-area) polygon", () => {
    const geometry = createPolygonGeometry([
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
      { lon: 0.5, lat: 0 },
    ]);
    expect(polygonAreaSquareMeters(geometry)).toBeCloseTo(0, 0);
  });

  it("subtracts inner ring (hole) area from the outer ring area", () => {
    const outer = [
      { lon: 0, lat: 0 },
      { lon: 4, lat: 0 },
      { lon: 4, lat: 4 },
      { lon: 0, lat: 4 },
    ];
    const hole = [
      { lon: 1, lat: 1 },
      { lon: 2, lat: 1 },
      { lon: 2, lat: 2 },
      { lon: 1, lat: 2 },
    ];
    const withHole = createPolygonGeometry(outer, [hole]);
    const withoutHole = createPolygonGeometry(outer);
    expect(polygonAreaSquareMeters(withHole)).toBeLessThan(polygonAreaSquareMeters(withoutHole));
  });

  it("is unaffected by winding direction", () => {
    const ring = [
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
      { lon: 1, lat: 1 },
      { lon: 0, lat: 1 },
    ];
    const reversed = [...ring].reverse();
    expect(polygonAreaSquareMeters(createPolygonGeometry(ring))).toBeCloseTo(
      polygonAreaSquareMeters(createPolygonGeometry(reversed)),
      0,
    );
  });
});
