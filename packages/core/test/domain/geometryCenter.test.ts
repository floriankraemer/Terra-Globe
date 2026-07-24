import { describe, expect, it } from "vitest";
import { geometryCenter } from "../../src/domain/geometryCenter.js";
import {
  createCircleGeometry,
  createLineStringGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
} from "../../src/domain/geometry.js";

describe("geometryCenter", () => {
  it("returns the point itself for a Point", () => {
    const geometry = createPointGeometry({ lon: 13.4, lat: 52.5 });
    expect(geometryCenter(geometry)).toEqual({ lon: 13.4, lat: 52.5 });
  });

  it("returns the circle's center", () => {
    const geometry = createCircleGeometry({ lon: 1, lat: 2 }, 500);
    expect(geometryCenter(geometry)).toEqual({ lon: 1, lat: 2 });
  });

  it("returns the midpoint of a Rectangle's bounds", () => {
    const geometry = createRectangleGeometry({ north: 10, south: 0, east: 10, west: 0 });
    expect(geometryCenter(geometry)).toEqual({ lon: 5, lat: 5 });
  });

  it("returns the average of a Polygon's outer ring vertices", () => {
    const geometry = createPolygonGeometry([
      { lon: 0, lat: 0 },
      { lon: 3, lat: 0 },
      { lon: 3, lat: 3 },
      { lon: 0, lat: 3 },
    ]);
    expect(geometryCenter(geometry)).toEqual({ lon: 1.5, lat: 1.5 });
  });

  it("returns the average of a LineString's path points", () => {
    const geometry = createLineStringGeometry([
      { lon: 0, lat: 0 },
      { lon: 2, lat: 0 },
    ]);
    expect(geometryCenter(geometry)).toEqual({ lon: 1, lat: 0 });
  });
});
