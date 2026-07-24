import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("dragging a placemark onto a folder moves it inside that folder", async ({ page }) => {
  // Native HTML5 drag-and-drop plus a reload (on top of folder/marker setup)
  // runs noticeably slower than this suite's other tests under this sandbox's
  // software WebGL rendering; the default 30s budget is too tight here.
  test.setTimeout(60_000);
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("button", { name: "New Folder" }).first().click();
  await page.getByPlaceholder("Folder name").fill("Trips");
  await page.getByRole("button", { name: "Create" }).click();
  await expect(page.getByText("Trips")).toBeVisible();

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("button", { name: "Marker" })
    .click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page
    .getByRole("form", { name: "Edit placemark" })
    .getByRole("button", { name: "Close" })
    .click();
  await expect(page.getByText("Point 1")).toBeVisible();

  const placemarkRow = page
    .getByText("Point 1")
    .locator("xpath=ancestor::*[contains(@class,'tree-row')]");
  const tripsRow = page
    .getByText("Trips")
    .locator("xpath=ancestor::*[contains(@class,'tree-row')]");

  await placemarkRow.dragTo(tripsRow, { targetPosition: { x: 40, y: 14 } });

  const tripsListItem = page
    .locator("li", { has: page.getByText("Trips", { exact: true }) })
    .first();
  await expect(tripsListItem.getByText("Point 1")).toBeVisible({ timeout: 10_000 });

  // Survives a reload, i.e. the move was actually persisted, not just a
  // client-side reorder of in-memory state.
  await page.reload();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);
  await expect(tripsListItem.getByText("Point 1")).toBeVisible({ timeout: 10_000 });
});
