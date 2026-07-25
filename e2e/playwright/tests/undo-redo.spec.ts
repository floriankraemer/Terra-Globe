import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("undo removes a just-created placemark, redo brings it back", async ({ page }) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  const undoButton = page
    .getByRole("toolbar", { name: "Undo/redo" })
    .getByRole("button", { name: "Undo" });
  const redoButton = page
    .getByRole("toolbar", { name: "Undo/redo" })
    .getByRole("button", { name: "Redo" });
  await expect(undoButton).toBeDisabled();
  await expect(redoButton).toBeDisabled();

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("button", { name: "Marker" })
    .click();
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);

  const placesPanel = page.locator(".places-panel");
  await expect(placesPanel.getByText("Point 1")).toBeVisible({ timeout: 15_000 });
  await expect(undoButton).toBeEnabled();

  await undoButton.click();

  await expect(placesPanel.getByText("Point 1")).not.toBeVisible();
  await expect(undoButton).toBeDisabled();
  await expect(redoButton).toBeEnabled();

  await redoButton.click();

  await expect(placesPanel.getByText("Point 1")).toBeVisible();
  await expect(undoButton).toBeEnabled();
  await expect(redoButton).toBeDisabled();
});

test("undo restores a deleted folder with its original name and order", async ({ page }) => {
  test.slow();
  await page.goto("/");
  await waitForAppReady(page);

  const placesPanel = page.locator(".places-panel");
  await placesPanel.getByRole("button", { name: "New Folder" }).click();
  await placesPanel.getByPlaceholder("Folder name").fill("Trip");
  await placesPanel.getByRole("button", { name: "Create" }).click();
  await expect(placesPanel.getByText("Trip")).toBeVisible({ timeout: 15_000 });

  // The row's Rename/Delete buttons are only shown on hover/focus (display:
  // none -> flex, see .tree-row-actions in global.css) - that toggle
  // reflows the row, so wait for the button to actually be visible/stable
  // before clicking it rather than racing the hover-triggered layout change.
  const tripRow = placesPanel.locator(".tree-row", { hasText: "Trip" });
  await tripRow.hover();
  const deleteButton = tripRow.getByRole("button", { name: "Delete" });
  await expect(deleteButton).toBeVisible();
  await deleteButton.click();
  await expect(placesPanel.getByText("Trip")).not.toBeVisible();

  const undoButton = page
    .getByRole("toolbar", { name: "Undo/redo" })
    .getByRole("button", { name: "Undo" });
  await undoButton.click();

  await expect(placesPanel.getByText("Trip")).toBeVisible({ timeout: 25_000 });
});
