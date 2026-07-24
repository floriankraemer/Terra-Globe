import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("a drawn placemark persists across reload via IndexedDB", async ({ page }) => {
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

  await page.reload();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await expect(page.getByText("Point 1")).toBeVisible();
});

test("creating a folder persists across reload", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("button", { name: "New Folder" }).click();
  await page.getByPlaceholder("Folder name").fill("Trips");
  await page.getByRole("button", { name: "Create" }).click();

  await expect(page.getByText("Trips")).toBeVisible();

  await page.reload();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await expect(page.getByText("Trips")).toBeVisible();
});
