import { describe, expect, it } from "vitest";
import { createFolder } from "../../src/domain/folder.js";
import {
  createCircleGeometry,
  createGroundOverlayGeometry,
  createLineStringGeometry,
  createModelGeometry,
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

  it("round-trips a StyleMap with a highlight style, and BalloonStyle/ListStyle fields", () => {
    const style = createStyle({
      outlineColor: "#3366cc",
      outlineWidth: 2,
      fillColor: "#cc6633",
      fillOpacity: 0.6,
      balloonText: "<b>$[name]</b>",
      balloonBgColor: "#ffffff",
      balloonTextColor: "#000000",
      listItemType: "checkHideChildren",
      itemIconHref: "icon.png",
      highlight: { outlineColor: "#ff0000", outlineWidth: 4 },
    });
    const placemark = createPlacemark({
      name: "Highlighted",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
      styleId: style.id,
    });

    const xml = serializeKml({ folders: [], placemarks: [placemark], styles: [style] });
    expect(xml).toContain("<StyleMap");
    const parsed = parseKml(xml);

    const parsedStyle = parsed.styles.find((s) => s.id === style.id)!;
    expect(parsedStyle.balloonText).toBe(style.balloonText);
    expect(parsedStyle.balloonBgColor).toBe(style.balloonBgColor);
    expect(parsedStyle.listItemType).toBe(style.listItemType);
    expect(parsedStyle.itemIconHref).toBe(style.itemIconHref);
    expect(parsedStyle.highlight?.outlineColor).toBe("#ff0000");
    expect(parsedStyle.highlight?.outlineWidth).toBe(4);
  });

  it("round-trips a GroundOverlay placemark", () => {
    const overlay = createPlacemark({
      name: "Scan",
      folderId: null,
      geometry: createGroundOverlayGeometry(
        { north: 10, south: 0, east: 10, west: 0 },
        "https://example.com/scan.png",
        45,
      ),
    });

    const xml = serializeKml({ folders: [], placemarks: [overlay], styles: [] });
    expect(xml).toContain("<GroundOverlay");
    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.geometry).toEqual(overlay.geometry);
  });

  it("round-trips a Model placemark", () => {
    const model = createPlacemark({
      name: "Statue",
      folderId: null,
      geometry: createModelGeometry(
        { lon: 1, lat: 2, altitude: 5 },
        "https://example.com/model.glb",
        { scale: 2, heading: 90, tilt: 0, roll: 0 },
      ),
    });

    const xml = serializeKml({ folders: [], placemarks: [model], styles: [] });
    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.geometry).toEqual(model.geometry);
  });

  it("round-trips TimeSpan, TimeStamp, Snippet, address, phoneNumber, and arbitrary ExtendedData", () => {
    const withTimeStamp = createPlacemark({
      name: "Event",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
      timeStamp: "2020-01-01T00:00:00Z",
      snippet: "A short summary",
      address: "1 Infinite Loop",
      phoneNumber: "+1 555 0100",
      extendedData: { population: "12345", mayor: "Someone" },
    });
    const withTimeSpan = createPlacemark({
      name: "Era",
      folderId: null,
      geometry: createPointGeometry({ lon: 1, lat: 1 }),
      timeSpanBegin: "2020-01-01",
      timeSpanEnd: "2021-01-01",
    });

    const xml = serializeKml({ folders: [], placemarks: [withTimeStamp, withTimeSpan], styles: [] });
    const parsed = parseKml(xml);

    const event = parsed.placemarks.find((p) => p.name === "Event")!;
    expect(event.timeStamp).toBe("2020-01-01T00:00:00Z");
    expect(event.snippet).toBe("A short summary");
    expect(event.address).toBe("1 Infinite Loop");
    expect(event.phoneNumber).toBe("+1 555 0100");
    expect(event.extendedData).toEqual({ population: "12345", mayor: "Someone" });

    const era = parsed.placemarks.find((p) => p.name === "Era")!;
    expect(era.timeSpanBegin).toBe("2020-01-01");
    expect(era.timeSpanEnd).toBe("2021-01-01");
  });

  it("round-trips a Camera view and a Region", () => {
    const placemark = createPlacemark({
      name: "Viewpoint",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
      view: {
        kind: "Camera",
        params: { longitude: 1, latitude: 2, altitude: 1000, heading: 90, tilt: 45, roll: 0 },
      },
      region: { raw: { "LatLonAltBox.north": 1, "LatLonAltBox.south": 0, "Lod.minLodPixels": 128 } },
    });

    const xml = serializeKml({ folders: [], placemarks: [placemark], styles: [] });
    expect(xml).toContain("<Camera>");
    expect(xml).toContain("<Region>");
    const parsed = parseKml(xml);

    expect(parsed.placemarks[0]!.view).toEqual({
      kind: "Camera",
      params: { longitude: 1, latitude: 2, altitude: 1000, heading: 90, tilt: 45, roll: 0 },
    });
    expect(parsed.placemarks[0]!.region?.raw["LatLonAltBox.north"]).toBe(1);
    expect(parsed.placemarks[0]!.region?.raw["Lod.minLodPixels"]).toBe(128);
  });

  it("round-trips folder description and open state", () => {
    const folder = createFolder({
      name: "Trips",
      parentId: null,
      order: 0,
      description: "Vacation spots",
      open: true,
    });

    const xml = serializeKml({ folders: [folder], placemarks: [], styles: [] });
    const parsed = parseKml(xml);

    expect(parsed.folders[0]!.description).toBe("Vacation spots");
    expect(parsed.folders[0]!.open).toBe(true);
  });

  it("round-trips tessellate/extrude/altitudeMode on LineString and Polygon", () => {
    const line = createPlacemark({
      name: "Path",
      folderId: null,
      geometry: createLineStringGeometry(
        [
          { lon: 0, lat: 0, altitudeMode: "relativeToGround" },
          { lon: 1, lat: 1 },
        ],
        { tessellate: true },
      ),
    });
    const polygon = createPlacemark({
      name: "Building",
      folderId: null,
      geometry: {
        ...createPolygonGeometry([
          { lon: 0, lat: 0, altitude: 50, altitudeMode: "relativeToGround" },
          { lon: 1, lat: 0 },
          { lon: 0, lat: 1 },
        ]),
        extrudeHeight: 50,
      },
    });

    const xml = serializeKml({ folders: [], placemarks: [line, polygon], styles: [] });
    expect(xml).toContain("<tessellate>1</tessellate>");
    expect(xml).toContain("<extrude>1</extrude>");
    expect(xml).toContain("<altitudeMode>relativeToGround</altitudeMode>");

    const parsed = parseKml(xml);
    const parsedLine = parsed.placemarks.find((p) => p.name === "Path")!;
    expect(parsedLine.geometry).toMatchObject({ type: "LineString", tessellate: true });
    const parsedPolygon = parsed.placemarks.find((p) => p.name === "Building")!;
    expect(parsedPolygon.geometry).toMatchObject({ type: "Polygon", extrudeHeight: 50 });
  });

  it("regroups placemarks that share a multiGeometryGroup marker into one Placemark/MultiGeometry", () => {
    const a = createPlacemark({
      name: "Group",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
      multiGeometryGroup: "group-1",
    });
    const b = createPlacemark({
      name: "Group (2)",
      folderId: null,
      geometry: createLineStringGeometry([
        { lon: 1, lat: 1 },
        { lon: 2, lat: 2 },
      ]),
      multiGeometryGroup: "group-1",
    });

    const xml = serializeKml({ folders: [], placemarks: [a, b], styles: [] });
    expect(xml.match(/<Placemark /g)).toHaveLength(1);
    expect(xml).toContain("<MultiGeometry>");

    const parsed = parseKml(xml);
    expect(parsed.placemarks).toHaveLength(2);
    expect(parsed.placemarks[0]!.multiGeometryGroup).toBe(parsed.placemarks[1]!.multiGeometryGroup);
  });

  it("imports a gx:Track as a LineString with parallel timestamps", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2" xmlns:gx="http://www.google.com/kml/ext/2.2">
  <Document>
    <Placemark>
      <name>Trip</name>
      <gx:Track>
        <when>2020-01-01T00:00:00Z</when>
        <when>2020-01-01T00:01:00Z</when>
        <gx:coord>1.0 2.0 10</gx:coord>
        <gx:coord>1.1 2.1 20</gx:coord>
      </gx:Track>
    </Placemark>
  </Document>
</kml>`;
    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.geometry).toEqual({
      type: "LineString",
      path: [
        { lon: 1.0, lat: 2.0, altitude: 10 },
        { lon: 1.1, lat: 2.1, altitude: 20 },
      ],
      timestamps: ["2020-01-01T00:00:00Z", "2020-01-01T00:01:00Z"],
    });
  });

  it("round-trips a ScreenOverlay", () => {
    const xml = serializeKml({
      folders: [],
      placemarks: [],
      styles: [],
      screenOverlays: [
        {
          id: "so1",
          folderId: null,
          name: "Legend",
          imageUrl: "https://example.com/legend.png",
          overlayXY: { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" },
          screenXY: { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" },
          rotation: 0,
          visibility: true,
          order: 0,
          createdAt: "now",
          updatedAt: "now",
        },
      ],
    });
    expect(xml).toContain("<ScreenOverlay");
    const parsed = parseKml(xml);

    expect(parsed.screenOverlays).toHaveLength(1);
    expect(parsed.screenOverlays[0]!.imageUrl).toBe("https://example.com/legend.png");
    expect(parsed.screenOverlays[0]!.name).toBe("Legend");
  });

  it("imports a PhotoOverlay with a LatLonBox as a GroundOverlay, and without one as a Point placemark", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <PhotoOverlay>
      <name>Panorama</name>
      <Icon><href>pano.jpg</href></Icon>
      <LatLonBox><north>1</north><south>0</south><east>1</east><west>0</west></LatLonBox>
    </PhotoOverlay>
    <PhotoOverlay>
      <name>Snapshot</name>
      <Icon><href>snap.jpg</href></Icon>
      <Point><coordinates>1,2,0</coordinates></Point>
    </PhotoOverlay>
  </Document>
</kml>`;
    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(2);
    const panorama = parsed.placemarks.find((p) => p.name === "Panorama")!;
    expect(panorama.geometry.type).toBe("GroundOverlay");
    const snapshot = parsed.placemarks.find((p) => p.name === "Snapshot")!;
    expect(snapshot.geometry.type).toBe("Point");
    expect(snapshot.extendedData?.["webglobe:photoUrl"]).toBe("snap.jpg");
  });
});
