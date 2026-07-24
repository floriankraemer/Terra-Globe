import { expect, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

test("a drawn rectangle defaults to outlined, not filled, and its style is editable", async ({
  page,
}) => {
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
  await page.mouse.click(box.x + box.width / 2 - 40, box.y + box.height / 2 - 40);
  await page.mouse.click(box.x + box.width / 2 + 40, box.y + box.height / 2 + 40);

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();

  // Defaults: outline on, fill off.
  await expect(editor.getByLabel("Outline", { exact: true })).toBeChecked();
  await expect(editor.getByLabel("Filled", { exact: true })).not.toBeChecked();
  await expect(editor.getByLabel("Fill Color")).toBeDisabled();
  await expect(editor.getByLabel("Outline Color")).toBeEnabled();

  await editor.getByLabel("Filled", { exact: true }).check();
  await expect(editor.getByLabel("Fill Color")).toBeEnabled();
  await editor.getByLabel("Fill Color").fill("#00ff00");
  await editor.getByLabel("Outline Width").fill("5");
  await editor.getByRole("button", { name: "Save" }).click();

  await expect(editor).not.toBeVisible();
});

test("unchecking Outline and re-opening the editor persists outlineEnabled: false", async ({
  page,
}) => {
  await page.goto("/");
  const canvas = page.locator("[data-testid=cesium-viewer] canvas").first();
  await expect(canvas).toBeVisible();
  await waitForAppReady(page);

  await page
    .getByRole("toolbar", { name: "Drawing tools" })
    .getByRole("combobox", { name: "Geometry" })
    .selectOption("circle");
  const box = await canvas.boundingBox();
  if (!box) throw new Error("canvas has no bounding box");
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
  await page.mouse.click(box.x + box.width / 2 + 40, box.y + box.height / 2);

  const editor = page.getByRole("form", { name: "Edit placemark" });
  await expect(editor).toBeVisible();
  const name = await editor.getByLabel("Name").inputValue();

  await editor.getByLabel("Outline", { exact: true }).uncheck();
  await editor.getByRole("button", { name: "Save" }).click();
  await expect(editor).not.toBeVisible();

  await expect(page.getByText(name)).toBeVisible({ timeout: 15_000 });
  await page.getByText(name).click();

  await expect(
    page.getByRole("form", { name: "Edit placemark" }).getByLabel("Outline", { exact: true }),
  ).not.toBeChecked({
    timeout: 10_000,
  });
});
