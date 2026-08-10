import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

async function cameraHeight(page: import("@playwright/test").Page): Promise<number> {
  return page.evaluate(() => {
    const viewer = (
      window as unknown as {
        __terraGlobeViewer: { camera: { positionCartographic: { height: number } } };
      }
    ).__terraGlobeViewer;
    return viewer.camera.positionCartographic.height;
  });
}

test("camera position survives a reload", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;

  const heightBefore = await cameraHeight(page);
  await page.mouse.move(cx, cy);
  await page.mouse.wheel(0, -800);
  // Cesium settles the camera - and fires moveEnd, which is what triggers
  // the save - a beat after the wheel event itself.
  await page.waitForTimeout(1000);
  const heightAfterZoom = await cameraHeight(page);
  expect(heightAfterZoom).not.toBeCloseTo(heightBefore, 0);

  await page.reload();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);
  const heightAfterReload = await cameraHeight(page);

  expect(heightAfterReload).toBeCloseTo(heightAfterZoom, 0);
});
