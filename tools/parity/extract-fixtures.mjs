#!/usr/bin/env node
// Differential fixture extractor.
//
// Runs the *TypeScript* implementation and writes the results as JSON goldens
// for the Rust port. This is how the rewrite proves behavioural parity instead
// of re-deriving expected values by hand. It requires `pnpm -r build` first.
//
// Usage: node tools/parity/extract-fixtures.mjs

import { writeFileSync, mkdirSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const core = (path) => import(resolve(root, "packages/core/dist", path));

const units = await core("domain/units.js");
const geometry = await core("domain/geometry.js");

const distances = [
  0, 1, 12.4, 999.4, 999.6, 1000, 1234.5, 1609.344, 1609.35, 5000, 1_000_000,
];
const areas = [0, 1, 9999.4, 10_000, 12345.67, 1_000_000, 2_500_000_000];
const durations = [0, 30, 45, 90, 3600, 5400, 7260];
const coordinates = [
  { lat: 52.52, lon: 13.405 },
  { lat: -33.45, lon: -70.5 },
  { lat: 0, lon: 0 },
  { lat: -0.000_001, lon: 179.999_999 },
];

const systems = ["metric", "imperial"];
const coordinateFormats = ["decimal", "dms"];

const fixture = {
  generatedBy: "tools/parity/extract-fixtures.mjs",
  units: {
    distance: systems.flatMap((system) =>
      distances.map((meters) => ({
        meters,
        system,
        expected: units.formatDistance(meters, system),
      })),
    ),
    area: systems.flatMap((system) =>
      areas.map((squareMeters) => ({
        squareMeters,
        system,
        expected: units.formatArea(squareMeters, system),
      })),
    ),
    duration: durations.map((seconds) => ({
      seconds,
      expected: units.formatDuration(seconds),
    })),
    coordinate: coordinateFormats.flatMap((format) =>
      coordinates.map((point) => ({
        ...point,
        format,
        expected: units.formatCoordinate(point, format),
      })),
    ),
  },
  geometryValidation: [
    {
      kind: "point-lon-out-of-range",
      lon: 190,
      lat: 0,
      expectedError: capture(() => geometry.createPointGeometry({ lon: 190, lat: 0 })),
    },
    {
      kind: "point-lat-out-of-range",
      lon: 0,
      lat: -91,
      expectedError: capture(() => geometry.createPointGeometry({ lon: 0, lat: -91 })),
    },
    {
      kind: "circle-zero-radius",
      expectedError: capture(() =>
        geometry.createCircleGeometry({ lon: 0, lat: 0 }, 0),
      ),
    },
    {
      kind: "rectangle-inverted",
      expectedError: capture(() =>
        geometry.createRectangleGeometry({ north: 10, south: 20, east: 1, west: 0 }),
      ),
    },
    {
      kind: "polygon-too-few-points",
      expectedError: capture(() =>
        geometry.createPolygonGeometry([{ lon: 0, lat: 0 }, { lon: 1, lat: 1 }]),
      ),
    },
    {
      kind: "linestring-too-few-points",
      expectedError: capture(() =>
        geometry.createLineStringGeometry([{ lon: 0, lat: 0 }]),
      ),
    },
  ],
};

function capture(fn) {
  try {
    fn();
    return null;
  } catch (error) {
    return error.message;
  }
}

const out = resolve(root, "crates/terra-testkit/fixtures/core-parity.json");
mkdirSync(dirname(out), { recursive: true });
writeFileSync(out, `${JSON.stringify(fixture, null, 2)}\n`);
console.log(`wrote ${out}`);
