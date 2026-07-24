import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("renders a Cesium globe with a WebGL canvas in the browser build", async ({ page }) => {
  const consoleErrors: string[] = [];
  page.on("console", (msg) => {
    if (msg.type() === "error") consoleErrors.push(msg.text());
  });

  await page.goto("/");
  await expect(page.getByTestId("cesium-viewer")).toBeVisible();

  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);
  const box = await canvas.boundingBox();
  expect(box?.width).toBeGreaterThan(0);
  expect(box?.height).toBeGreaterThan(0);

  expect(consoleErrors).toEqual([]);
});

test("switching the base layer dropdown does not error", async ({ page }) => {
  await page.goto("/");
  const basemapSelect = page.getByRole("combobox", { name: "Basemap" });
  await basemapSelect.selectOption("opentopomap");
  await expect(basemapSelect).toHaveValue("opentopomap");
});
