import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("drawing a rectangle and exporting it downloads a PNG", async ({ page }) => {
  test.setTimeout(90_000);
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  const cx = box.x + box.width / 2;
  const cy = box.y + box.height / 2;

  // The default camera frames the whole globe, so a screen-space rectangle
  // there spans a huge ground area and would blow the export pixel-dimension
  // cap regardless of scale/DPI. Zoom in first, as a real user would before
  // exporting a small area.
  for (let i = 0; i < 25; i++) {
    await page.mouse.move(cx, cy);
    await page.mouse.wheel(0, -300);
  }

  await page
    .getByRole("toolbar", { name: "Area export" })
    .getByRole("button", { name: "Export Area" })
    .click();
  await expect(
    page.getByRole("toolbar", { name: "Area export" }).getByRole("button", { name: "Export Area" }),
  ).toHaveAttribute("aria-pressed", "true");

  await page.mouse.click(cx - 60, cy - 60);
  await page.mouse.click(cx + 60, cy + 60);

  const panel = page.getByLabel("Area export settings");
  await expect(panel).toBeVisible();
  await expect(panel.getByRole("button", { name: "Redraw Area" })).toBeVisible();

  await page.getByLabel("Scale").selectOption("10000");
  await page.getByLabel("DPI").selectOption("150");

  const downloadPromise = page.waitForEvent("download", { timeout: 30_000 });
  await panel.getByRole("button", { name: "Export PNG" }).click();
  const download = await downloadPromise;

  expect(download.suggestedFilename()).toMatch(/^terra-globe-area-export-.*\.png$/);

  const downloadPath = await download.path();
  const fs = await import("node:fs/promises");
  const stats = await fs.stat(downloadPath!);
  expect(stats.size).toBeGreaterThan(1000);
});

test("starting area export cancels an active Ruler measurement", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }).click();
  await expect(
    page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }),
  ).toHaveAttribute("aria-pressed", "true");

  await page
    .getByRole("toolbar", { name: "Area export" })
    .getByRole("button", { name: "Export Area" })
    .click();

  await expect(
    page.getByRole("toolbar", { name: "Ruler" }).getByRole("button", { name: "Ruler" }),
  ).toHaveAttribute("aria-pressed", "false");
  await expect(
    page.getByRole("toolbar", { name: "Area export" }).getByRole("button", { name: "Export Area" }),
  ).toHaveAttribute("aria-pressed", "true");
});
