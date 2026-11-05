#!/usr/bin/env node
// Differential KML fixtures.
//
// Runs the TypeScript KML implementation over a corpus (the shared E2E fixtures
// plus documents built from the domain model) and records what it parsed and
// serialized. The Rust port is tested against these outputs, which is how the
// rewrite proves round-trip parity instead of asserting hand-written values.
//
// Requires `pnpm -r build` first. Usage: node tools/parity/extract-kml-fixtures.mjs

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const core = (path) => import(resolve(root, "packages/core/dist", path));

const { parseKml } = await core("kml/parseKml.js");
const { serializeKml } = await core("kml/serializeKml.js");
const geometry = await core("domain/geometry.js");
const { createFolder } = await core("domain/folder.js");
const { createPlacemark } = await core("domain/placemark.js");
const { createStyle } = await core("domain/style.js");
const { createScreenOverlay } = await core("domain/screenOverlay.js");

const fixturesDir = resolve(root, "e2e/playwright/fixtures");

function readFixture(name) {
  return readFileSync(resolve(fixturesDir, name), "utf8");
}

// Synthetic documents cover everything the E2E fixtures do not: circles,
// rectangles, styles with highlights, view/region/time round-tripping, gx:Track,
// models, ground overlays and screen overlays.
function syntheticCorpus() {
  const style = createStyle({
    outlineColor: "#3366cc",
    outlineWidth: 2,
    fillColor: "#cc6633",
    fillOpacity: 0.6,
  });
  style.highlight = { outlineColor: "#ff0000", outlineWidth: 3, fillColor: "#00ff00", fillOpacity: 0.4 };

  const root = createFolder({ name: "My Places", parentId: null, order: 0 });
  const trips = createFolder({ name: "Trips", parentId: root.id, order: 2, description: "Nested" });

  const point = createPlacemark({
    name: "Berlin",
    description: "Capital",
    folderId: root.id,
    styleId: style.id,
    geometry: geometry.createPointGeometry({ lon: 13.405, lat: 52.52, altitude: 34 }),
    snippet: "snippet",
    address: "Berlin, Germany",
    phoneNumber: "(030) 1234",
    timeStamp: "2024-01-01T00:00:00Z",
    view: { kind: "Camera", params: { longitude: 13.405, latitude: 52.52, altitude: 1000, heading: 0 } },
    region: { raw: { "LatLonAltBox.north": 53, "LatLonAltBox.south": 52, "Lod.minLodPixels": 128 } },
    extendedData: { source: "test" },
  });

  const circle = createPlacemark({
    name: "Circle",
    folderId: root.id,
    geometry: geometry.createCircleGeometry({ lon: 13.4, lat: 52.5 }, 1200),
  });
  const rectangle = createPlacemark({
    name: "Rectangle",
    folderId: trips.id,
    geometry: geometry.createRectangleGeometry({ north: 52.6, south: 52.4, east: 13.5, west: 13.3 }),
  });
  const polygon = createPlacemark({
    name: "Polygon",
    folderId: trips.id,
    geometry: geometry.createPolygonGeometry(
      [
        { lon: 9, lat: 51 },
        { lon: 9.1, lat: 51 },
        { lon: 9.1, lat: 51.1 },
        { lon: 9, lat: 51 },
      ],
      [
        [
          { lon: 9.02, lat: 51.02 },
          { lon: 9.04, lat: 51.02 },
          { lon: 9.04, lat: 51.04 },
          { lon: 9.02, lat: 51.02 },
        ],
      ],
      { tessellate: true, extrudeHeight: 12 },
    ),
  });
  const line = createPlacemark({
    name: "Track",
    folderId: trips.id,
    geometry: geometry.createLineStringGeometry(
      [
        { lon: 9, lat: 51, altitude: 100 },
        { lon: 9.05, lat: 51.02, altitude: 140 },
      ],
      { tessellate: true, timestamps: ["2024-01-01T00:00:00Z", "2024-01-01T00:05:00Z"] },
    ),
  });
  const model = createPlacemark({
    name: "Model",
    folderId: null,
    geometry: geometry.createModelGeometry(
      { lon: 9, lat: 51, altitude: 10 },
      "models/test.glb",
      { scale: 2, heading: 45, tilt: 10, roll: 5 },
    ),
  });
  const groundOverlay = createPlacemark({
    name: "Overlay",
    folderId: null,
    geometry: geometry.createGroundOverlayGeometry(
      { north: 52.6, south: 52.4, east: 13.5, west: 13.3 },
      "images/overlay.png",
      15,
    ),
  });
  const screenOverlay = createScreenOverlay({
    name: "Legend",
    folderId: root.id,
    imageUrl: "images/legend.png",
    overlayXY: { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" },
    screenXY: { x: 10, y: 10, xUnits: "pixels", yUnits: "pixels" },
    sizeX: 120,
    sizeY: 80,
    rotation: 0,
  });

  const input = {
    folders: [root, trips],
    placemarks: [point, circle, rectangle, polygon, line, model, groundOverlay],
    styles: [style],
    screenOverlays: [screenOverlay],
  };
  return { name: "synthetic-all-features", kml: serializeKml(input) };
}

function normalize(value) {
  if (Array.isArray(value)) return value.map(normalize);
  if (value && typeof value === "object") {
    const out = {};
    for (const [key, child] of Object.entries(value)) {
      // `JSON.stringify` drops undefined, but `Object.entries` sees own keys
      // that were explicitly set to undefined. Keep the two behaviours aligned.
      if (child === undefined) continue;
      if (key === "id") out[key] = "<id>";
      else if (key === "createdAt" || key === "updatedAt") out[key] = "<ts>";
      else if (key === "multiGeometryGroup") out[key] = "<group>";
      else out[key] = normalize(child);
    }
    return out;
  }
  return value;
}

// The E2E fixtures carry no ids, so both implementations would generate random
// UUIDs (and folder references would differ). Stamping ids makes the comparison
// deterministic; the Rust test applies the identical transformation.
function stampIds(xml) {
  const counters = new Map();
  return xml.replace(/<(Folder|Placemark|ScreenOverlay|GroundOverlay)(\s|>)/g, (_, tag, next) => {
    const index = (counters.get(tag) ?? 0) + 1;
    counters.set(tag, index);
    return `<${tag} id="auto-${tag}-${index}"${next}`;
  });
}

const corpus = [
  { name: "e2e-sample", kml: stampIds(readFixture("sample.kml")), stamped: true },
  { name: "e2e-mixed-geometry", kml: stampIds(readFixture("mixed-geometry.kml")), stamped: true },
  syntheticCorpus(),
];

// Generated ids and timestamps differ between the two implementations (and
// between runs), so they are masked identically on both sides before comparing
// serialized XML.
const normalizeSerialized = (xml) =>
  xml
    .replace(/id="[^"]+"/g, 'id="<id>"')
    .replace(/\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d{3}Z/g, "<ts>")
    // V8 and the platform libm can differ in the last ULP of sin/cos/asin, which
    // shows up in circle approximations. Nine decimals is far below the
    // precision the domain model or any consumer uses.
    .replace(/-?\d+\.\d{10,}/g, (value) => Number(value).toFixed(9));

const entries = corpus.map(({ name, kml, stamped }) => {
  const parsed = parseKml(kml);
  return {
    name,
    input: kml,
    stamped: Boolean(stamped),
    parsed: normalize(parsed),
    serialized: normalizeSerialized(serializeKml(parsed)),
    warnings: parsed.warnings,
  };
});

const out = resolve(root, "crates/terra-testkit/fixtures/kml-parity.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify({ generatedBy: "tools/parity/extract-kml-fixtures.mjs", entries }, null, 2)}\n`);
console.log(`wrote ${out} (${entries.length} entries)`);
