import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("clicking a marker on the globe opens its editor showing its location", async ({ page }) => {
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
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;
  await page.mouse.click(cx, cy);

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();
  await editor.getByRole("button", { name: "Close" }).click();
  await expect(editor).not.toBeVisible();

  // Click the marker entity itself (not the toolbar) - should re-open the editor.
  await page.mouse.click(cx, cy);
  await expect(editor).toBeVisible({ timeout: 10_000 });
  await expect(editor.getByText("Location")).toBeVisible();
});

test("double-clicking a placemark in the list flies the camera to it", async ({ page }) => {
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

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();
  const name = await editor.getByLabel("Name").inputValue();
  await editor.getByRole("button", { name: "Close" }).click();

  const heightBefore = await page.evaluate(() => {
    const viewer = (
      window as unknown as {
        __webglobeViewer: { camera: { positionCartographic: { height: number } } };
      }
    ).__webglobeViewer;
    return viewer.camera.positionCartographic.height;
  });

  await page.getByText(name).dblclick();
  await page.waitForTimeout(2500); // let the flyTo animation finish

  const heightAfter = await page.evaluate(() => {
    const viewer = (
      window as unknown as {
        __webglobeViewer: { camera: { positionCartographic: { height: number } } };
      }
    ).__webglobeViewer;
    return viewer.camera.positionCartographic.height;
  });

  // Marker fly-to targets a fixed 10km altitude, far below the default
  // whole-globe starting view - a large height drop confirms the camera moved.
  expect(heightAfter).toBeLessThan(heightBefore / 2);
});
