import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

const fixturesDir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "fixtures");

test("Elevation Profile button appears for a track with altitude data and opens the profile panel", async ({
  page,
}) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const fileChooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Import KML/KMZ" }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(path.join(fixturesDir, "mixed-geometry.kml"));

  const notice = page.getByRole("alert");
  await expect(notice).toBeVisible({ timeout: 15_000 });

  await page.locator(".places-panel").getByText("Wanderweg").click();
  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();

  await expect(page.locator(".height-profile-panel")).not.toBeVisible();
  await editor.getByRole("button", { name: "Elevation Profile" }).click();

  await expect(page.locator(".height-profile-panel")).toBeVisible();
});

test("Elevation Profile button does not appear for a placemark without a LineString geometry", async ({
  page,
}) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const fileChooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Import KML/KMZ" }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(path.join(fixturesDir, "mixed-geometry.kml"));

  const notice = page.getByRole("alert");
  await expect(notice).toBeVisible({ timeout: 15_000 });

  await page.getByText("Hochsitz").click();
  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();

  await expect(editor.getByRole("button", { name: "Elevation Profile" })).not.toBeVisible();
});
