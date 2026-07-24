import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("measuring 3 points shows 2 segments and a total distance", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }).click();
  await expect(
    page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }),
  ).toHaveAttribute("aria-pressed", "true");

  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");

  await page.mouse.click(box.x + box.width / 2 - 40, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2 + 40, box.y + box.height / 2);

  const panel = page.getByLabel("Ruler measurement");
  await expect(panel.getByText(/^Segment 1:/)).toBeVisible();
  await expect(panel.getByText(/^Segment 2:/)).toBeVisible();
  await expect(panel.getByText(/^Total:/)).toBeVisible();
});

test("Undo removes the last segment", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }).click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");

  await page.mouse.click(box.x + box.width / 2 - 40, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2 + 40, box.y + box.height / 2);

  const panel = page.getByLabel("Ruler measurement");
  await expect(panel.getByText(/^Segment 2:/)).toBeVisible();

  await page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Undo" }).click();

  await expect(panel.getByText(/^Segment 2:/)).not.toBeVisible();
  await expect(panel.getByText(/^Segment 1:/)).toBeVisible();
});

test("Cancel clears the measurement and hides the panel", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }).click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");

  await page.mouse.click(box.x + box.width / 2 - 40, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);

  await page
    .getByRole("toolbar", { name: "Ruler" })
    .getByRole("button", { name: "Cancel" })
    .click();

  await expect(
    page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }),
  ).toHaveAttribute("aria-pressed", "false");
  await expect(page.getByLabel("Ruler measurement")).not.toBeVisible();
});

test("Finish stops accepting further clicks but keeps the panel visible", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }).click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");

  await page.mouse.click(box.x + box.width / 2 - 40, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2 + 40, box.y + box.height / 2);

  await page
    .getByRole("toolbar", { name: "Ruler" })
    .getByRole("button", { name: "Finish" })
    .click();

  const panel = page.getByLabel("Ruler measurement");
  await expect(panel.getByText(/^Segment 2:/)).toBeVisible();

  // Ruler is no longer active - a further click must not add a new segment.
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2 - 40);
  await expect(panel.getByText(/^Segment 3:/)).not.toBeVisible();
});
