import { describe, expect, it } from "vitest";
import { parseKml } from "../../src/kml/parseKml.js";

describe("parseKml - resilience against real-world KML", () => {
  it("expands a MultiGeometry placemark into one placemark per supported sub-geometry", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark>
      <name>District</name>
      <MultiGeometry>
        <Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 1,0 1,1 0,1 0,0</coordinates></LinearRing></outerBoundaryIs></Polygon>
        <Polygon><outerBoundaryIs><LinearRing><coordinates>2,2 3,2 3,3 2,3 2,2</coordinates></LinearRing></outerBoundaryIs></Polygon>
      </MultiGeometry>
    </Placemark>
  </Document>
</kml>`;

    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(2);
    expect(parsed.placemarks[0]!.name).toBe("District");
    expect(parsed.placemarks[1]!.name).toBe("District (2)");
    expect(parsed.placemarks[0]!.geometry.type).toBe("Polygon");
    expect(parsed.placemarks[1]!.geometry.type).toBe("Polygon");
    expect(parsed.warnings).toEqual([]);
  });

  it("imports a LineString placemark as a path, with no warning", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark>
      <name>Trail</name>
      <LineString><coordinates>0,0 1,1 2,2</coordinates></LineString>
    </Placemark>
    <Placemark>
      <name>Camp</name>
      <Point><coordinates>5,5</coordinates></Point>
    </Placemark>
  </Document>
</kml>`;

    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(2);
    const trail = parsed.placemarks.find((p) => p.name === "Trail");
    expect(trail?.geometry).toEqual({
      type: "LineString",
      path: [
        { lon: 0, lat: 0 },
        { lon: 1, lat: 1 },
        { lon: 2, lat: 2 },
      ],
    });
    expect(parsed.warnings).toEqual([]);
  });

  it("treats a bare LinearRing geometry as a Polygon", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark>
      <name>Ring</name>
      <LinearRing><coordinates>0,0 1,0 1,1 0,1 0,0</coordinates></LinearRing>
    </Placemark>
  </Document>
</kml>`;

    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.geometry.type).toBe("Polygon");
    expect(parsed.warnings).toEqual([]);
  });

  it("skips a placemark whose geometry fails domain validation, but keeps parsing the rest", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark>
      <name>BadPolygon</name>
      <Polygon><outerBoundaryIs><LinearRing><coordinates>0,0 1,0</coordinates></LinearRing></outerBoundaryIs></Polygon>
    </Placemark>
    <Placemark>
      <name>GoodSpot</name>
      <Point><coordinates>5,5</coordinates></Point>
    </Placemark>
  </Document>
</kml>`;

    const parsed = parseKml(xml);

    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.name).toBe("GoodSpot");
    expect(parsed.warnings).toHaveLength(1);
    expect(parsed.warnings[0]).toMatch(/BadPolygon/);
  });

  it("skips a folder/placemark with no supported geometry at all (e.g. GroundOverlay) without throwing", () => {
    const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Folder>
      <name>Overlays</name>
      <GroundOverlay>
        <name>Map image</name>
      </GroundOverlay>
      <Placemark>
        <name>Marker</name>
        <Point><coordinates>1,1</coordinates></Point>
      </Placemark>
    </Folder>
  </Document>
</kml>`;

    expect(() => parseKml(xml)).not.toThrow();
    const parsed = parseKml(xml);
    expect(parsed.placemarks).toHaveLength(1);
    expect(parsed.placemarks[0]!.name).toBe("Marker");
  });

  it("throws a clear error for fundamentally invalid input", () => {
    expect(() => parseKml("this is not xml at all { }")).toThrow(/kml/i);
  });
});
