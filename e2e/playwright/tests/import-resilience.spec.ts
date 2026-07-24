import path from "node:path";
import { fileURLToPath } from "node:url";
import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

const fixturesDir = path.join(path.dirname(fileURLToPath(import.meta.url)), "..", "fixtures");

test("importing KML with MultiGeometry and a LineString imports everything, including the path", async ({
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
  await expect(notice).toContainText("Imported 4 placemark(s)");
  await expect(notice).not.toContainText("skipped");

  // MultiGeometry's two polygons become two placemarks; LineString and Point both import too.
  await expect(page.getByText("Aussengrenze", { exact: true })).toBeVisible();
  await expect(page.getByText("Aussengrenze (2)")).toBeVisible();
  await expect(page.getByText("Hochsitz")).toBeVisible();
  await expect(page.locator(".places-panel").getByText("Wanderweg")).toBeVisible();
});

test("importing KML with a genuinely unsupported geometry (e.g. a Model) shows a warning but still imports the rest", async ({
  page,
}) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const xml = `<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2">
  <Document>
    <Placemark>
      <name>3D Hut</name>
      <Model><Link><href>hut.dae</href></Link></Model>
    </Placemark>
    <Placemark>
      <name>Camp</name>
      <Point><coordinates>5,5</coordinates></Point>
    </Placemark>
  </Document>
</kml>`;
  await page.evaluate(async (contents) => {
    const dt = new DataTransfer();
    dt.items.add(
      new File([contents], "model.kml", { type: "application/vnd.google-earth.kml+xml" }),
    );
    const input = document.querySelector('input[type="file"]') as HTMLInputElement;
    input.files = dt.files;
    input.dispatchEvent(new Event("change", { bubbles: true }));
  }, xml);

  const notice = page.getByRole("alert");
  await expect(notice).toBeVisible({ timeout: 15_000 });
  await expect(notice).toContainText("Imported 1 placemark(s)");
  await expect(notice).toContainText("skipped");
  await expect(notice).toContainText("3D Hut");
  await expect(page.getByText("Camp")).toBeVisible();
  await expect(page.locator(".places-panel").getByText("3D Hut")).not.toBeVisible();
});

test("importing an invalid file shows an error notice instead of failing silently", async ({
  page,
}) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const fileChooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Import KML/KMZ" }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(path.join(fixturesDir, "invalid.kml"));

  const notice = page.getByRole("alert");
  await expect(notice).toBeVisible({ timeout: 15_000 });
  await expect(notice).toHaveClass(/notice-error/);
});

test("the error notice can be dismissed", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const fileChooserPromise = page.waitForEvent("filechooser");
  await page.getByRole("button", { name: "Import KML/KMZ" }).click();
  const fileChooser = await fileChooserPromise;
  await fileChooser.setFiles(path.join(fixturesDir, "invalid.kml"));

  const notice = page.getByRole("alert");
  await expect(notice).toBeVisible({ timeout: 15_000 });
  await page.getByRole("button", { name: "Dismiss" }).click();
  await expect(notice).not.toBeVisible();
});
