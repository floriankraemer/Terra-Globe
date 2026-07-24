import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("selecting the Marker tool and clicking the globe places a point entity", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("button", { name: "Marker" })
    .click();
  await expect(
    page.getByRole("toolbar", { name: "Drawing tools" }).getByRole("button", { name: "Marker" }),
  ).toHaveAttribute("aria-pressed", "true");

  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);

  // After committing, the tool toggles back off (mode returns to idle).
  await expect(
    page.getByRole("toolbar", { name: "Drawing tools" }).getByRole("button", { name: "Marker" }),
  ).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByRole("button", { name: "Cancel" })).not.toBeVisible();
});

test("drawing a polygon accumulates vertices until Finish is clicked", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("combobox", { name: "Geometry" })
    .selectOption("polygon");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");

  await page.mouse.click(box.x + box.width / 2 - 40, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2 + 40, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2 - 40);

  // Still in polygon mode - only Finish commits it.
  await expect(
    page
      .getByRole("toolbar", { name: "Drawing tools" })
      .getByRole("combobox", { name: "Geometry" }),
  ).toHaveValue("polygon");

  await page.getByRole("button", { name: "Finish" }).click();

  await expect(
    page
      .getByRole("toolbar", { name: "Drawing tools" })
      .getByRole("combobox", { name: "Geometry" }),
  ).toHaveValue("");
  await expect(page.getByRole("button", { name: "Finish" })).not.toBeVisible();
});

test("Cancel discards an in-progress shape", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("combobox", { name: "Geometry" })
    .selectOption("rectangle");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2 - 20, box.y + box.height / 2);

  await page.getByRole("button", { name: "Cancel" }).click();

  await expect(
    page
      .getByRole("toolbar", { name: "Drawing tools" })
      .getByRole("combobox", { name: "Geometry" }),
  ).toHaveValue("");
  await expect(page.getByRole("button", { name: "Cancel" })).not.toBeVisible();
});
