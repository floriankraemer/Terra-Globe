import { describe, expect, it } from "vitest";
import { translateGeometry } from "../../src/domain/geometryTranslate.js";
import type {
  CircleGeometry,
  GroundOverlayGeometry,
  LineStringGeometry,
  ModelGeometry,
  PointGeometry,
  PolygonGeometry,
  RectangleGeometry,
} from "../../src/domain/geometry.js";

describe("translateGeometry", () => {
  it("shifts a Point and preserves altitude fields", () => {
    const geometry: PointGeometry = {
      type: "Point",
      coordinates: { lon: 1, lat: 2, altitude: 5, altitudeMode: "absolute" },
    };
    const result = translateGeometry(geometry, 0.5, -0.5) as PointGeometry;
    expect(result.coordinates).toEqual({
      lon: 1.5,
      lat: 1.5,
      altitude: 5,
      altitudeMode: "absolute",
    });
  });

  it("shifts a Circle center and preserves radius", () => {
    const geometry: CircleGeometry = {
      type: "Circle",
      center: { lon: 1, lat: 2 },
      radiusMeters: 50,
    };
    const result = translateGeometry(geometry, 1, 1) as CircleGeometry;
    expect(result.center).toEqual({ lon: 2, lat: 3 });
    expect(result.radiusMeters).toBe(50);
  });

  it("shifts a Rectangle's bounds uniformly", () => {
    const geometry: RectangleGeometry = {
      type: "Rectangle",
      north: 10,
      south: 5,
      east: 20,
      west: 15,
    };
    const result = translateGeometry(geometry, 1, -1) as RectangleGeometry;
    expect(result).toEqual({ type: "Rectangle", north: 9, south: 4, east: 21, west: 16 });
  });

  it("shifts a GroundOverlay's bounds and preserves other fields", () => {
    const geometry: GroundOverlayGeometry = {
      type: "GroundOverlay",
      bounds: { north: 10, south: 5, east: 20, west: 15 },
      imageUrl: "img.png",
      rotation: 30,
    };
    const result = translateGeometry(geometry, 1, 1) as GroundOverlayGeometry;
    expect(result.bounds).toEqual({ north: 11, south: 6, east: 21, west: 16 });
    expect(result.imageUrl).toBe("img.png");
    expect(result.rotation).toBe(30);
  });

  it("shifts every point of a Polygon's outer and inner rings", () => {
    const geometry: PolygonGeometry = {
      type: "Polygon",
      outerRing: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
        { lon: 1, lat: 1 },
      ],
      innerRings: [
        [
          { lon: 0.2, lat: 0.2 },
          { lon: 0.4, lat: 0.2 },
          { lon: 0.4, lat: 0.4 },
        ],
      ],
    };
    const result = translateGeometry(geometry, 1, 1) as PolygonGeometry;
    expect(result.outerRing).toEqual([
      { lon: 1, lat: 1 },
      { lon: 2, lat: 1 },
      { lon: 2, lat: 2 },
    ]);
    expect(result.innerRings?.[0]).toEqual([
      { lon: 1.2, lat: 1.2 },
      { lon: 1.4, lat: 1.2 },
      { lon: 1.4, lat: 1.4 },
    ]);
  });

  it("shifts every point of a LineString and preserves timestamps", () => {
    const geometry: LineStringGeometry = {
      type: "LineString",
      path: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 1 },
      ],
      timestamps: ["2024-01-01T00:00:00Z", "2024-01-01T00:01:00Z"],
    };
    const result = translateGeometry(geometry, 1, 1) as LineStringGeometry;
    expect(result.path).toEqual([
      { lon: 1, lat: 1 },
      { lon: 2, lat: 2 },
    ]);
    expect(result.timestamps).toEqual(geometry.timestamps);
  });

  it("shifts a Model's position and preserves orientation fields", () => {
    const geometry: ModelGeometry = {
      type: "Model",
      position: { lon: 1, lat: 2 },
      modelUri: "model.glb",
      heading: 90,
    };
    const result = translateGeometry(geometry, 1, 1) as ModelGeometry;
    expect(result.position).toEqual({ lon: 2, lat: 3 });
    expect(result.modelUri).toBe("model.glb");
    expect(result.heading).toBe(90);
  });
});
