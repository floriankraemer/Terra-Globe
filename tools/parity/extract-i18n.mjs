#!/usr/bin/env node
// Writes the web app's translation catalogues as flat JSON for the Rust UI.
//
// The TypeScript locales are nested objects; the Rust lookup uses dotted keys
// ("about.title"). Flattening here means the key-parity test can compare exact
// key sets and the UI only needs one lookup function.
//
// Usage: node tools/parity/extract-i18n.mjs

import { readFileSync, writeFileSync, mkdirSync, readdirSync } from "node:fs";
import { dirname, resolve, basename } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const localesDir = resolve(root, "packages/ui/src/i18n/locales");
const outDir = resolve(root, "crates/terra-app/i18n");

function flatten(value, prefix = "", out = {}) {
  for (const [key, child] of Object.entries(value)) {
    const dotted = prefix ? `${prefix}.${key}` : key;
    if (child && typeof child === "object") flatten(child, dotted, out);
    else out[dotted] = String(child);
  }
  return out;
}

const codes = readdirSync(localesDir)
  .filter((file) => file.endsWith(".ts"))
  .map((file) => basename(file, ".ts"))
  .sort();

mkdirSync(outDir, { recursive: true });
let keyCount = 0;
for (const code of codes) {
  const module = await import(resolve(localesDir, `${code}.ts`));
  const flat = flatten(module.default);
  keyCount = Object.keys(flat).length;
  writeFileSync(resolve(outDir, `${code}.json`), `${JSON.stringify(flat, null, 2)}\n`);
}
writeFileSync(resolve(outDir, "locales.json"), `${JSON.stringify(codes, null, 2)}\n`);
console.log(`wrote ${codes.length} catalogues (${keyCount} keys each) to ${outDir}`);
