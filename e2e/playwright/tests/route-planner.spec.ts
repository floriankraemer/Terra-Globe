import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("planning a route accumulates stops and shows total distance/time", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Route planner" })
    .getByRole("button", { name: "Route" })
    .click();
  await expect(
    page.getByRole("toolbar", { name: "Route planner" }).getByRole("button", { name: "Route" }),
  ).toHaveAttribute("aria-pressed", "true");

  // The panel opens immediately on "Route", before any stop exists.
  const panel = page.locator(".route-planner-panel");
  await expect(panel).toBeVisible();
  await expect(panel.getByText(/^Stop 1:/)).not.toBeVisible();

  // Train mode uses a local straight-line estimate (no network dependency),
  // keeping this test deterministic regardless of routing-API reachability.
  await page.getByLabel("Mode").selectOption("train");

  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2 - 60, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2 - 40);
  await page.mouse.click(box.x + box.width / 2 + 60, box.y + box.height / 2);

  await expect(panel.getByText(/^Stop 1:/)).toBeVisible();
  await expect(panel.getByText(/^Stop 3:/)).toBeVisible();
  await expect(panel.locator(".route-planner-panel-total")).toContainText("min");

  await page
    .getByRole("toolbar", { name: "Route planner" })
    .getByRole("button", { name: "Cancel" })
    .click();
  await expect(panel).not.toBeVisible();
});

test("adding a stop via the panel's search box shows its address, not coordinates", async ({
  page,
}) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Route planner" })
    .getByRole("button", { name: "Route" })
    .click();
  const panel = page.locator(".route-planner-panel");
  await expect(panel).toBeVisible();

  await panel.getByLabel("Address").fill("Berlin, Germany");
  await panel.getByRole("button", { name: "Search" }).click();
  await panel
    .getByRole("button", { name: /Berlin/ })
    .first()
    .click();

  const firstStop = panel.locator(".route-planner-panel-stops li").first();
  await expect(firstStop).toContainText("Berlin");
  await expect(firstStop).not.toContainText(/^Stop 1: -?\d+\.\d+, -?\d+\.\d+$/);
});

test("dragging a stop reorders the list", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Route planner" })
    .getByRole("button", { name: "Route" })
    .click();
  await page.getByLabel("Mode").selectOption("train");

  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2 - 60, box.y + box.height / 2 - 60);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2 + 60, box.y + box.height / 2 + 60);

  const panel = page.locator(".route-planner-panel");
  const items = panel.locator(".route-planner-panel-stops li");
  await expect(items).toHaveCount(3);
  const thirdStopId = await items.nth(2).getAttribute("data-stop-id");

  await items.nth(2).dragTo(items.nth(0));

  await expect(items.nth(0)).toHaveAttribute("data-stop-id", thirdStopId!);
});
