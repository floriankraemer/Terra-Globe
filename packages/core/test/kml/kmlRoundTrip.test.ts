import { describe, expect, it } from "vitest";
import { createFolder } from "../../src/domain/folder.js";
import {
  createCircleGeometry,
  createPointGeometry,
  createPolygonGeometry,
  createRectangleGeometry,
} from "../../src/domain/geometry.js";
import { createPlacemark } from "../../src/domain/placemark.js";
import { createStyle } from "../../src/domain/style.js";
import { parseKml } from "../../src/kml/parseKml.js";
import { serializeKml } from "../../src/kml/serializeKml.js";

describe("serializeKml / parseKml round trip", () => {
  it("round-trips folders, nested folders, placemarks of every shape, and styles losslessly", () => {
    const style = createStyle({
      outlineColor: "#3366cc",
      outlineWidth: 2,
      fillColor: "#cc6633",
      fillOpacity: 0.6,
    });
    const root = createFolder({ name: "My Places", parentId: null, order: 0 });
    const nested = createFolder({ name: "Trips", parentId: root.id, order: 0 });

    const pointPlacemark = createPlacemark({
      name: "Berlin",
      description: "Capital of Germany",
      folderId: nested.id,
      geometry: createPointGeometry({ lon: 13.4, lat: 52.5 }),
      styleId: style.id,
    });
    const polygonPlacemark = createPlacemark({
      name: "Triangle",
      folderId: nested.id,
      geometry: createPolygonGeometry([
        { lon: 0, lat: 0 },
        { lon: 1, lat: 0 },
        { lon: 0, lat: 1 },
      ]),
    });
    const rectanglePlacemark = createPlacemark({
      name: "Box",
      folderId: null,
      geometry: createRectangleGeometry({ north: 10, south: 0, east: 10, west: 0 }),
    });
    const circlePlacemark = createPlacemark({
      name: "Zone",
      folderId: null,
      geometry: createCircleGeometry({ lon: 1, lat: 2 }, 500),
    });

    const xml = serializeKml({
      folders: [root, nested],
      placemarks: [pointPlacemark, polygonPlacemark, rectanglePlacemark, circlePlacemark],
      styles: [style],
    });

    const parsed = parseKml(xml);

    expect(parsed.folders).toEqual(
      expect.arrayContaining([
        expect.objectContaining({ id: root.id, name: "My Places", parentId: null }),
        expect.objectContaining({ id: nested.id, name: "Trips", parentId: root.id }),
      ]),
    );
    expect(parsed.folders).toHaveLength(2);

    const byId = new Map(parsed.placemarks.map((p) => [p.id, p]));
    expect(byId.get(pointPlacemark.id)).toEqual(pointPlacemark);
    expect(byId.get(polygonPlacemark.id)).toEqual(polygonPlacemark);
    expect(byId.get(rectanglePlacemark.id)).toEqual(rectanglePlacemark);
    expect(byId.get(circlePlacemark.id)?.geometry).toEqual(circlePlacemark.geometry);

    expect(parsed.styles).toEqual([style]);
  });

  it("gracefully imports plain third-party KML with no webglobe ExtendedData", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark>
      <name>Somewhere</name>
      <Point><coordinates>10.5,20.25,0</coordinates></Point>
    </Placemark>
  </Document>
</kml>`;

    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.name).toBe("Somewhere");
    expect(parsed.placemarks[0]!.geometry).toEqual({
      type: "Point",
      coordinates: { lon: 10.5, lat: 20.25, altitude: 0 },
    });
  });
});
