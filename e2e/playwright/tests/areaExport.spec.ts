import { expect, type Page, test } from "@playwright/test";
import { waitForAppReady } from "./helpers.js";

interface Cartographic {
  longitude: number;
  latitude: number;
}
interface Ellipsoid {
  cartesianToCartographic(cartesian: unknown): Cartographic;
}
interface TestableViewer {
  camera: {
    frustum: { width?: number };
    pickEllipsoid(windowPosition: { x: number; y: number }, ellipsoid: Ellipsoid): unknown;
  };
  scene: {
    globe: { ellipsoid: Ellipsoid };
    render: (...args: unknown[]) => unknown;
  };
  canvas: { clientWidth: number; clientHeight: number };
}

/**
 * Picks the ground lon/lat under a canvas-relative screen position, using the same
 * viewer.camera.pickEllipsoid + ellipsoid.cartesianToCartographic technique the app itself uses
 * (see CesiumScreenPicker.pickGround). Reads the viewer off window.__terraGlobeViewer, which
 * App.tsx already exposes for E2E inspection (see map-interaction.spec.ts / geocoding.spec.ts) -
 * no production code changes needed for this.
 */
async function pickGroundPoint(
  page: Page,
  canvasX: number,
  canvasY: number,
): Promise<{ lon: number; lat: number } | null> {
  return page.evaluate(
    ({ x, y }) => {
      const viewer = (window as unknown as { __terraGlobeViewer: TestableViewer })
        .__terraGlobeViewer;
      const ellipsoid = viewer.scene.globe.ellipsoid;
      const cartesian = viewer.camera.pickEllipsoid({ x, y }, ellipsoid);
      if (!cartesian) return null;
      const carto = ellipsoid.cartesianToCartographic(cartesian);
      return { lon: (carto.longitude * 180) / Math.PI, lat: (carto.latitude * 180) / Math.PI };
    },
    { x: canvasX, y: canvasY },
  );
}

/**
 * Monkey-patches the exposed viewer's scene.render (test-side only, no production code touched)
 * to record the ground extent of every frame rendered while an orthographic export frustum is
 * active - OrthographicFrustum is the only frustum in this app with a `.width` property
 * (PerspectiveFrustum has `.fov` instead), so this only fires during captureAreaImage.
 */
async function recordExportFrames(page: Page): Promise<void> {
  await page.evaluate(() => {
    const viewer = (window as unknown as { __terraGlobeViewer: TestableViewer }).__terraGlobeViewer;
    (window as unknown as { __exportFrames: unknown[] }).__exportFrames = [];
    const originalRender = viewer.scene.render.bind(viewer.scene);
    viewer.scene.render = (...args: unknown[]) => {
      const result = originalRender(...args);
      const frustum = viewer.camera.frustum;
      if (typeof frustum.width === "number") {
        const ellipsoid = viewer.scene.globe.ellipsoid;
        const canvasEl = viewer.canvas;
        const tl = viewer.camera.pickEllipsoid({ x: 0, y: 0 }, ellipsoid);
        const br = viewer.camera.pickEllipsoid(
          { x: canvasEl.clientWidth, y: canvasEl.clientHeight },
          ellipsoid,
        );
        if (tl && br) {
          const tlC = ellipsoid.cartesianToCartographic(tl);
          const brC = ellipsoid.cartesianToCartographic(br);
          const toDeg = (rad: number) => (rad * 180) / Math.PI;
          (window as unknown as { __exportFrames: unknown[] }).__exportFrames.push({
            west: toDeg(tlC.longitude),
            north: toDeg(tlC.latitude),
            east: toDeg(brC.longitude),
            south: toDeg(brC.latitude),
          });
        }
      }
      return result;
    };
  });
}

test("drawing a rectangle and exporting it downloads a PNG framed to the drawn area", async ({
  page,
}) => {
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

  await recordExportFrames(page);

  await page
    .getByRole("toolbar", { name: "Area export" })
    .getByRole("button", { name: "Export Area" })
    .click();
  await expect(
    page.getByRole("toolbar", { name: "Area export" }).getByRole("button", { name: "Export Area" }),
  ).toHaveAttribute("aria-pressed", "true");

  const corner1Screen = { x: cx - 60, y: cy - 60 };
  const corner2Screen = { x: cx + 60, y: cy + 60 };
  // Ground points under the two rectangle corners, picked the same way the app itself picks
  // them on click (CesiumScreenPicker.pickGround) - this is the independently-computed ground
  // truth the exported image's framing is checked against below.
  const corner1 = await pickGroundPoint(page, corner1Screen.x - box.x, corner1Screen.y - box.y);
  const corner2 = await pickGroundPoint(page, corner2Screen.x - box.x, corner2Screen.y - box.y);
  if (!corner1 || !corner2) throw new Error("could not pick ground points for the drawn rectangle");
  const drawnBounds = {
    west: Math.min(corner1.lon, corner2.lon),
    east: Math.max(corner1.lon, corner2.lon),
    south: Math.min(corner1.lat, corner2.lat),
    north: Math.max(corner1.lat, corner2.lat),
  };

  await page.mouse.click(corner1Screen.x, corner1Screen.y);
  await page.mouse.click(corner2Screen.x, corner2Screen.y);

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

  // Framing correctness: the last frame rendered by captureAreaImage should closely match the
  // drawn rectangle's real-world bounds, not some much larger (or smaller) area. A generous
  // tolerance (20% of the rectangle's own span) still easily catches a gross regression like the
  // "captured area is many times larger than drawn" bug this guards against, while tolerating
  // minor rendering/picking variance.
  const frames = await page.evaluate(
    () =>
      (
        window as unknown as {
          __exportFrames: { west: number; east: number; north: number; south: number }[];
        }
      ).__exportFrames,
  );
  expect(frames.length).toBeGreaterThan(0);
  const lastFrame = frames[frames.length - 1]!;

  const lonTolerance = (drawnBounds.east - drawnBounds.west) * 0.2;
  const latTolerance = (drawnBounds.north - drawnBounds.south) * 0.2;
  expect(Math.abs(lastFrame.west - drawnBounds.west)).toBeLessThan(lonTolerance);
  expect(Math.abs(lastFrame.east - drawnBounds.east)).toBeLessThan(lonTolerance);
  expect(Math.abs(lastFrame.north - drawnBounds.north)).toBeLessThan(latTolerance);
  expect(Math.abs(lastFrame.south - drawnBounds.south)).toBeLessThan(latTolerance);
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
