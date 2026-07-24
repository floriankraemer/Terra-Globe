import { describe, expect, it } from "vitest";
import {
  createCircleGeometry,
  createLineStringGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
} from "../../../src/domain/geometry.js";
import {
  domainGeometryToKml,
  kmlToDomainGeometry,
} from "../../../src/kml/mapping/geometryKmlMapping.js";

describe("domainGeometryToKml", () => {
  it("maps a Point to a KML Point with no extended data", () => {
    const geometry = createPointGeometry({ lon: 13.4, lat: 52.5 });
    const result = domainGeometryToKml(geometry);
    expect(result.element).toBe("Point");
    expect(result.rings).toEqual([[{ lon: 13.4, lat: 52.5 }]]);
    expect(result.extendedData).toEqual([]);
  });

  it("maps a Polygon to a KML Polygon with outer and inner rings, no extended data", () => {
    const outer = [
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
      { lon: 0, lat: 1 },
    ];
    const geometry = createPolygonGeometry(outer);
    const result = domainGeometryToKml(geometry);
    expect(result.element).toBe("Polygon");
    expect(result.rings).toEqual([outer]);
    expect(result.extendedData).toEqual([]);
  });

  it("maps a Rectangle to a KML Polygon annotated with shape=rectangle extended data", () => {
    const geometry = createRectangleGeometry({ north: 10, south: 0, east: 10, west: 0 });
    const result = domainGeometryToKml(geometry);
    expect(result.element).toBe("Polygon");
    expect(result.rings[0]).toEqual([
      { lon: 0, lat: 0 },
      { lon: 10, lat: 0 },
      { lon: 10, lat: 10 },
      { lon: 0, lat: 10 },
      { lon: 0, lat: 0 },
    ]);
    expect(result.extendedData).toEqual([
      { name: "webglobe:shape", value: "rectangle" },
      { name: "webglobe:north", value: "10" },
      { name: "webglobe:south", value: "0" },
      { name: "webglobe:east", value: "10" },
      { name: "webglobe:west", value: "0" },
    ]);
  });

  it("maps a Circle to a KML Polygon (approximated) annotated with shape=circle extended data", () => {
    const geometry = createCircleGeometry({ lon: 1, lat: 2 }, 500);
    const result = domainGeometryToKml(geometry);
    expect(result.element).toBe("Polygon");
    expect(result.rings[0]!.length).toBeGreaterThan(3);
    expect(result.extendedData).toEqual([
      { name: "webglobe:shape", value: "circle" },
      { name: "webglobe:centerLon", value: "1" },
      { name: "webglobe:centerLat", value: "2" },
      { name: "webglobe:radiusMeters", value: "500" },
    ]);
  });
});

describe("kmlToDomainGeometry", () => {
  it("reconstructs a Point from a KML Point", () => {
    const geometry = kmlToDomainGeometry("Point", [[{ lon: 13.4, lat: 52.5 }]], {});
    expect(geometry).toEqual(createPointGeometry({ lon: 13.4, lat: 52.5 }));
  });

  it("reconstructs a plain Polygon when there is no webglobe:shape extended data", () => {
    const outer = [
      { lon: 0, lat: 0 },
      { lon: 1, lat: 0 },
      { lon: 0, lat: 1 },
    ];
    const geometry = kmlToDomainGeometry("Polygon", [outer], {});
    expect(geometry).toEqual(createPolygonGeometry(outer));
  });

  it("reconstructs a Polygon with holes from outer + inner rings", () => {
    const outer = [
      { lon: 0, lat: 0 },
      { lon: 4, lat: 0 },
      { lon: 0, lat: 4 },
    ];
    const hole = [
      { lon: 0.5, lat: 0.5 },
      { lon: 1, lat: 0.5 },
      { lon: 0.5, lat: 1 },
    ];
    const geometry = kmlToDomainGeometry("Polygon", [outer, hole], {});
    expect(geometry).toEqual(createPolygonGeometry(outer, [hole]));
  });

  it("losslessly reconstructs a Rectangle from webglobe:shape extended data, ignoring the polygon approximation", () => {
    const geometry = kmlToDomainGeometry(
      "Polygon",
      [
        [
          { lon: 0, lat: 0 },
          { lon: 10, lat: 0 },
          { lon: 10, lat: 10 },
          { lon: 0, lat: 10 },
        ],
      ],
      {
        "webglobe:shape": "rectangle",
        "webglobe:north": "10",
        "webglobe:south": "0",
        "webglobe:east": "10",
        "webglobe:west": "0",
      },
    );
    expect(geometry).toEqual(createRectangleGeometry({ north: 10, south: 0, east: 10, west: 0 }));
  });

  it("losslessly reconstructs a Circle from webglobe:shape extended data, ignoring the polygon approximation", () => {
    const geometry = kmlToDomainGeometry(
      "Polygon",
      [
        [
          { lon: 1, lat: 2 },
          { lon: 2, lat: 2 },
          { lon: 2, lat: 3 },
        ],
      ],
      {
        "webglobe:shape": "circle",
        "webglobe:centerLon": "1",
        "webglobe:centerLat": "2",
        "webglobe:radiusMeters": "500",
      },
    );
    expect(geometry).toEqual(createCircleGeometry({ lon: 1, lat: 2 }, 500));
  });
});

describe("LineString", () => {
  const path = [
    { lon: 0, lat: 0 },
    { lon: 1, lat: 1 },
    { lon: 2, lat: 0 },
  ];

  it("maps a LineString to a KML LineString with no extended data", () => {
    const geometry = createLineStringGeometry(path);
    const result = domainGeometryToKml(geometry);
    expect(result.element).toBe("LineString");
    expect(result.rings).toEqual([path]);
    expect(result.extendedData).toEqual([]);
  });

  it("reconstructs a LineString from a KML LineString", () => {
    const geometry = kmlToDomainGeometry("LineString", [path], {});
    expect(geometry).toEqual(createLineStringGeometry(path));
  });
});
