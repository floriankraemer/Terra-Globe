import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

// 1x1 transparent PNG, used to stub a tile response with a valid image/* body.
const PNG_1X1 = Buffer.from(
  "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII=",
  "base64",
);

async function openProvidersTab(page: import("@playwright/test").Page): Promise<void> {
  await page.getByRole("button", { name: "View Home" }).click();
  await page.getByRole("button", { name: "Providers" }).click();
}

test("adding and testing a tile provider makes it available in the Basemap dropdown", async ({
  page,
}) => {
  // Settings modal open/close plus several form interactions on top of the
  // Cesium canvas runs noticeably slower under this sandbox's software WebGL
  // rendering (see tree-drag-drop.spec.ts); the default 30s budget is too tight.
  test.setTimeout(60_000);
  await page.route(
    "https://api.mapbox.com/styles/v1/mapbox/streets-v12/tiles/**",
    async (route) => {
      await route.fulfill({ status: 200, contentType: "image/png", body: PNG_1X1 });
    },
  );

  await page.goto("/");
  await waitForAppReady(page);
  await openProvidersTab(page);

  await page.getByLabel("Type").selectOption("tile");
  await page.getByLabel("Provider").selectOption("mapbox-streets");
  await page.getByLabel("Name").fill("My Mapbox");
  await page.getByLabel("API Key").fill("test-key");
  await page.getByRole("button", { name: "Add Provider" }).click();

  await page.getByRole("button", { name: "Test" }).click();
  await expect(page.getByText("success")).toBeVisible();

  await page.getByRole("button", { name: "Close" }).click();

  await expect(page.getByLabel("Basemap")).toContainText("My Mapbox");
  await page.getByLabel("Basemap").selectOption({ label: "My Mapbox" });
});

test("adding and testing a geocoding provider makes it the active search backend", async ({
  page,
}) => {
  test.setTimeout(60_000);
  await page.route("https://api.opencagedata.com/geocode/v1/json**", async (route) => {
    await route.fulfill({
      status: 200,
      contentType: "application/json",
      body: JSON.stringify({
        results: [{ formatted: "Berlin, Germany", geometry: { lat: 52.5200066, lng: 13.404954 } }],
      }),
    });
  });

  await page.goto("/");
  await waitForAppReady(page);
  await openProvidersTab(page);

  await page.getByLabel("Type").selectOption("geocoding");
  await page.getByLabel("Provider").selectOption("opencage");
  await page.getByLabel("Name").fill("My OpenCage");
  await page.getByLabel("API Key").fill("test-key");
  await page.getByRole("button", { name: "Add Provider" }).click();

  await page.getByRole("button", { name: "Test" }).click();
  await expect(page.getByText("success")).toBeVisible();

  await page.getByRole("button", { name: "Close" }).click();

  await page.locator(".app-topbar").getByLabel("Address").fill("Berlin");
  await page.getByRole("button", { name: "Search" }).click();
  await expect(page.getByRole("button", { name: "Berlin, Germany" })).toBeVisible();
});
