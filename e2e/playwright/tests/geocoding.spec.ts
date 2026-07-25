import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("searching an address and selecting a result flies the camera there", async ({ page }) => {
  await page.route("https://nominatim.openstreetmap.org/search**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify([
        { display_name: "Berlin, Germany", lat: "52.5200066", lon: "13.4049540" },
      ]),
    });
  });

  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const heightBefore = await page.evaluate(() => {
    const viewer = (
      window as unknown as {
        __terraGlobeViewer: { camera: { positionCartographic: { height: number } } };
      }
    ).__terraGlobeViewer;
    return viewer.camera.positionCartographic.height;
  });

  await page.locator(".app-topbar").getByLabel("Address").fill("Berlin");
  await page.getByRole("button", { name: "Search" }).click();
  await page.getByRole("button", { name: "Berlin, Germany" }).click();
  await page.waitForTimeout(2500); // let the flyTo animation finish

  const heightAfter = await page.evaluate(() => {
    const viewer = (
      window as unknown as {
        __terraGlobeViewer: { camera: { positionCartographic: { height: number } } };
      }
    ).__terraGlobeViewer;
    return viewer.camera.positionCartographic.height;
  });

  expect(heightAfter).toBeLessThan(heightBefore / 2);
});

test("shows an empty state when a search returns no results", async ({ page }) => {
  await page.route("https://nominatim.openstreetmap.org/search**", async (route) => {
    await route.fulfill({ status: 200, contentType: "application/json", body: "[]" });
  });

  await page.goto("/");
  await waitForAppReady(page);

  await page.locator(".app-topbar").getByLabel("Address").fill("asdkjhasdkjhasd");
  await page.getByRole("button", { name: "Search" }).click();

  await expect(page.getByText("No results")).toBeVisible();
});
