import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("toolbar and places panel chrome match the visual baseline", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("button", { name: "New Folder" }).click();
  await page.getByPlaceholder("Folder name").fill("Trips");
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page.getByText("Trips")).toBeVisible();

  // The Cesium canvas re-renders every frame (stars, atmosphere) and is
  // never pixel-stable across runs - hide it so only the static DOM chrome
  // (toolbar, panel) is compared.
  await page.evaluate(() => {
    const canvas = document.querySelector("[data-testid=cesium-viewer] canvas");
    if (canvas instanceof HTMLElement) canvas.style.visibility = "hidden";
  });

  await expect(page.locator("body")).toHaveScreenshot("app-chrome.png", {
    clip: { x: 0, y: 0, width: 1000, height: 210 },
    animations: "disabled",
  });
});
