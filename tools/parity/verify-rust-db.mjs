#!/usr/bin/env node
// Reads a database written by the Rust app the way the TypeScript repository
// does: `JSON.parse(row.data) as Folder | Placemark`. The TS package cannot be
// imported by Node directly (its package.json points at TypeScript sources that
// Vite resolves), so this script reproduces the adapter's only transformation
// and asserts the payload shapes its interfaces declare.
//
// Usage: node tools/parity/verify-rust-db.mjs <library.db>

import { createRequire } from "node:module";
import { resolve, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const require = createRequire(resolve(root, "packages/storage-sqlite/package.json"));
const Database = require("better-sqlite3");

const databasePath = process.argv[2];
if (!databasePath) {
  console.error("usage: node tools/parity/verify-rust-db.mjs <library.db>");
  process.exit(2);
}

const database = new Database(databasePath, { readonly: true });
const failures = [];

const rows = (table) => database.prepare(`SELECT data FROM ${table}`).all().map((row) => JSON.parse(row.data));

const folders = rows("folders");
const placemarks = rows("placemarks");
const styles = rows("styles");
const overlays = rows("screen_overlays");

for (const folder of folders) {
  if (typeof folder.id !== "string" || typeof folder.name !== "string") {
    failures.push(`folder row is malformed: ${JSON.stringify(folder)}`);
  }
  if (!("parentId" in folder)) failures.push(`folder ${folder.id} has no parentId key`);
  if (typeof folder.visibility !== "boolean") failures.push(`folder ${folder.id} visibility is not boolean`);
  if (typeof folder.order !== "number") failures.push(`folder ${folder.id} order is not a number`);
}

const GEOMETRY_TYPES = new Set(["Point", "Circle", "Rectangle", "Polygon", "LineString", "GroundOverlay", "Model"]);
for (const placemark of placemarks) {
  if (typeof placemark.id !== "string" || typeof placemark.name !== "string") {
    failures.push(`placemark row is malformed: ${JSON.stringify(placemark)}`);
  }
  if (!GEOMETRY_TYPES.has(placemark.geometry?.type)) {
    failures.push(`placemark ${placemark.id} has an unknown geometry type: ${placemark.geometry?.type}`);
  }
  if (!("folderId" in placemark)) failures.push(`placemark ${placemark.id} has no folderId key`);
  if (typeof placemark.createdAt !== "string" || !placemark.createdAt.endsWith("Z")) {
    failures.push(`placemark ${placemark.id} has a bad createdAt: ${placemark.createdAt}`);
  }
  if (typeof placemark.updatedAt !== "string" || !placemark.updatedAt.endsWith("Z")) {
    failures.push(`placemark ${placemark.id} has a bad updatedAt: ${placemark.updatedAt}`);
  }
}

for (const style of styles) {
  if (typeof style.outlineColor !== "string" || typeof style.fillColor !== "string") {
    failures.push(`style row is malformed: ${JSON.stringify(style)}`);
  }
}

const schemaVersion = database.prepare("SELECT version FROM schema_migrations").get()?.version;

console.log(
  `old app can read the Rust-written DB: ${folders.length} folders, ${placemarks.length} placemarks, ` +
    `${styles.length} styles, ${overlays.length} screen overlays, schema v${schemaVersion}`
);
for (const placemark of placemarks) {
  console.log(`  - ${placemark.name} (${placemark.geometry.type})`);
}

database.close();
if (failures.length > 0) {
  for (const failure of failures) console.error(`FAIL: ${failure}`);
  process.exit(1);
}
