import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("drawing a marker opens the editor, and saving updates its name in the tree", async ({
  page,
}) => {
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

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();
  await expect(editor.getByLabel("Name")).toHaveValue("Point 1");

  await editor.getByLabel("Name").fill("My Favorite Spot");
  await editor.getByLabel("Description").fill("Great view here");
  await editor.getByLabel("Color").fill("#00ff00");
  await editor.getByRole("button", { name: "Save" }).click();

  await expect(editor).not.toBeVisible();
  await expect(page.getByText("My Favorite Spot")).toBeVisible({ timeout: 15_000 });
  await expect(page.getByText("Point 1")).not.toBeVisible();
});

test("name, description and color survive a reload", async ({ page }) => {
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

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await editor.getByLabel("Name").fill("Home");
  await editor.getByLabel("Color").fill("#0000ff");
  await editor.getByRole("button", { name: "Save" }).click();
  await expect(page.getByText("Home")).toBeVisible({ timeout: 15_000 });

  await page.reload();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);
  await expect(page.getByText("Home")).toBeVisible({ timeout: 15_000 });

  await page.getByText("Home").click();
  await expect(page.getByRole("form", { name: "Edit placemark" }).getByLabel("Color")).toHaveValue(
    "#0000ff",
    { timeout: 10_000 },
  );
});

test("Close discards the editor without saving changes", async ({ page }) => {
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

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await editor.getByLabel("Name").fill("Should not save");
  await editor.getByRole("button", { name: "Close" }).click();

  await expect(editor).not.toBeVisible();
  await expect(page.getByText("Point 1")).toBeVisible();
  await expect(page.getByText("Should not save")).not.toBeVisible();
});

test("Delete asks for confirmation, and only removes the placemark once confirmed", async ({
  page,
}) => {
  // Two confirm-dialog round trips on top of the Cesium canvas runs noticeably
  // slower under this sandbox's software WebGL rendering (see
  // tree-drag-drop.spec.ts); the default 30s budget is too tight here.
  test.setTimeout(60_000);
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

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();
  await editor.getByRole("button", { name: "Delete" }).click();

  const dialog = page.getByRole("alertdialog", { name: "Delete placemark?" });
  await expect(dialog).toBeVisible();
  await dialog.getByRole("button", { name: "Cancel" }).click();
  await expect(dialog).not.toBeVisible();
  await expect(editor).toBeVisible();
  await expect(page.getByText("Point 1")).toBeVisible();

  await editor.getByRole("button", { name: "Delete" }).click();
  await page
    .getByRole("alertdialog", { name: "Delete placemark?" })
    .getByRole("button", { name: "Delete" })
    .click();

  await expect(editor).not.toBeVisible();
  await expect(page.getByText("Point 1")).not.toBeVisible();
});
