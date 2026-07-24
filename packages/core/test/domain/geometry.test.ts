import { describe, expect, it } from "vitest";
import {
  createCircleGeometry,
  createLineStringGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
} from "../../src/domain/geometry.js";

describe("createPointGeometry", () => {
  it("creates a Point from a valid GeoPoint", () => {
    const geometry = createPointGeometry({ lon: 13.4, lat: 52.5 });
    expect(geometry).toEqual({ type: "Point", coordinates: { lon: 13.4, lat: 52.5 } });
  });

  it.each([
    [181, 0],
    [-181, 0],
    [0, 91],
    [0, -91],
  ])("rejects out-of-range coordinates lon=%s lat=%s", (lon, lat) => {
    expect(() => createPointGeometry({ lon, lat })).toThrow(/range/i);
  });
});

describe("createCircleGeometry", () => {
  it("creates a Circle with a positive radius", () => {
    const geometry = createCircleGeometry({ lon: 0, lat: 0 }, 500);
    expect(geometry).toEqual({
      type: "Circle",
      center: { lon: 0, lat: 0 },
      radiusMeters: 500,
    });
  });

  it.each([0, -1, Number.NaN])("rejects a non-positive radius (%s)", (radius) => {
    expect(() => createCircleGeometry({ lon: 0, lat: 0 }, radius)).toThrow(/radius/i);
  });
});

describe("createRectangleGeometry", () => {
  it("creates a Rectangle from valid bounds", () => {
    const geometry = createRectangleGeometry({ north: 10, south: 0, east: 10, west: 0 });
    expect(geometry).toEqual({ type: "Rectangle", north: 10, south: 0, east: 10, west: 0 });
  });

  it("rejects when south is not less than north", () => {
    expect(() => createRectangleGeometry({ north: 0, south: 10, east: 10, west: 0 })).toThrow(
      /north/i,
    );
  });

  it("rejects when west is not less than east", () => {
    expect(() => createRectangleGeometry({ north: 10, south: 0, east: 0, west: 10 })).toThrow(
      /east/i,
    );
  });
});

describe("createPolygonGeometry", () => {
  const triangle = [
    { lon: 0, lat: 0 },
    { lon: 1, lat: 0 },
    { lon: 0, lat: 1 },
  ];

  it("creates a Polygon from an outer ring with at least 3 points", () => {
    const geometry = createPolygonGeometry(triangle);
    expect(geometry).toEqual({ type: "Polygon", outerRing: triangle, innerRings: undefined });
  });

  it("rejects an outer ring with fewer than 3 points", () => {
    expect(() => createPolygonGeometry(triangle.slice(0, 2))).toThrow(/at least 3/i);
  });

  it("accepts inner rings (holes) with at least 3 points each", () => {
    const hole = [
      { lon: 0.1, lat: 0.1 },
      { lon: 0.2, lat: 0.1 },
      { lon: 0.1, lat: 0.2 },
    ];
    const geometry = createPolygonGeometry(triangle, [hole]);
    expect(geometry.innerRings).toEqual([hole]);
  });

  it("rejects an inner ring with fewer than 3 points", () => {
    expect(() => createPolygonGeometry(triangle, [[{ lon: 0.1, lat: 0.1 }]])).toThrow(
      /at least 3/i,
    );
  });
});

describe("createLineStringGeometry", () => {
  const path = [
    { lon: 0, lat: 0 },
    { lon: 1, lat: 0 },
    { lon: 1, lat: 1 },
  ];

  it("creates a LineString from a path with at least 2 points", () => {
    const geometry = createLineStringGeometry(path);
    expect(geometry).toEqual({ type: "LineString", path });
  });

  it("rejects a path with fewer than 2 points", () => {
    expect(() => createLineStringGeometry([{ lon: 0, lat: 0 }])).toThrow(/at least 2/i);
  });

  it("rejects a path with an out-of-range coordinate", () => {
    expect(() =>
      createLineStringGeometry([
        { lon: 200, lat: 0 },
        { lon: 1, lat: 1 },
      ]),
    ).toThrow(/range/i);
  });
});
