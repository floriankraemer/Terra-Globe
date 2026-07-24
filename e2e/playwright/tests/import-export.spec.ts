import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

const fixturesDir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "fixtures");

test("importing a real Google-Earth-style KML shows its folder and placemark", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const fileChooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Import KML/KMZ" }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(path.join(fixturesDir, "sample.kml"));

  await expect(page.getByText("My Places")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Brandenburg Gate")).toBeVisible();
});

test("exporting KML downloads a file containing the drawn placemark", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("button", { name: "Marker" })
    .click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(page.getByText("Point 1")).toBeVisible();

  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export KML" }).click();
  const download = await downloadPromise;

  expect(download.suggestedFilename()).toBe("webglobe-export.kml");
  const downloadPath = await download.path();
  const fs = await import("node:fs/promises");
  const contents = await fs.readFile(downloadPath!, "utf-8");
  expect(contents).toContain("<Placemark");
  expect(contents).toContain("Point 1");
});

test("importing an exported KMZ round-trips a drawn placemark", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("button", { name: "Marker" })
    .click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await expect(page.getByText("Point 1")).toBeVisible();

  const downloadPromise = page.waitForEvent("download");
  await page.getByRole("button", { name: "Export KMZ" }).click();
  const download = await downloadPromise;
  const downloadedPath = await download.path();

  // Simulate a fresh device/browser profile by wiping local storage, rather
  // than asserting on the already-covered persist-across-reload behavior.
  await page.evaluate(() => indexedDB.deleteDatabase("webglobe"));
  await page.reload();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);
  await expect(page.getByText("Point 1")).not.toBeVisible();

  const fileChooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Import KML/KMZ" }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles({
    name: "webglobe-export.kmz",
    mimeType: "application/vnd.google-earth.kmz",
    buffer: await (await import("node:fs/promises")).readFile(downloadedPath!),
  });

  await expect(page.getByText("Point 1")).toBeVisible();
});
