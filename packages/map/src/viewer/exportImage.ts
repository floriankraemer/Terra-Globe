import * as Cesium from "cesium";
import {
  createRectangleGeometry,
  rectangleHeightMeters,
  rectangleWidthMeters,
  type RectangleBounds,
} from "@terra-globe/core";
import { applyCameraView, getCameraView } from "./cameraView.js";

const METERS_PER_INCH = 0.0254;
const EXPORT_CAMERA_HEIGHT_METERS = 50_000;
const TILES_LOADED_TIMEOUT_MS = 15_000;

/**
 * Safe per-tile render size in pixels. Deliberately well under typical GPU
 * single-texture limits (Cesium.ContextLimits.maximumTextureSize is usually
 * 8192 or 16384) to leave headroom for the rest of the scene's GPU memory
 * use (terrain, imagery, other render targets). Callers should pass
 * min(maximumTextureSize, MAX_TILE_DIMENSION_PX) as computeExportPlan's/
 * captureAreaImage's maxTileDimensionPx.
 */
export const MAX_TILE_DIMENSION_PX = 4096;

/**
 * Hard ceiling on the total exported raster, independent of GPU tiling.
 * A 2D <canvas> also has browser-enforced area/dimension limits (Chrome
 * caps canvas area around ~268 million px, roughly 16384x16384; Firefox
 * around ~124 million px, roughly 11180x11180, or a 32767-per-dimension
 * cap) and allocating a canvas near those limits can silently fail or
 * crash the tab even when the pixels are theoretically computable. These
 * are picked conservatively below both browsers' real limits.
 */
export const MAX_OUTPUT_DIMENSION_PX = 12_000;
export const MAX_OUTPUT_MEGAPIXELS = 100;

export interface ExportPlan {
  groundWidthMeters: number;
  groundHeightMeters: number;
  pixelWidth: number;
  pixelHeight: number;
  scaleDenominator: number;
  dpiValue: number;
  /** Number of GPU render tiles this export will be split into (1 = no tiling needed). */
  tileCount: number;
}

/** A single render tile: a ground sub-rectangle and its pixel placement in the composite raster. */
export interface TileRect {
  bounds: RectangleBounds;
  pixelX: number;
  pixelY: number;
  pixelWidth: number;
  pixelHeight: number;
}

/** Thrown when the requested scale/DPI combination would exceed the allowed export dimensions, even with tiling. */
export class ExportTooLargeError extends Error {
  constructor(
    public readonly pixelWidth: number,
    public readonly pixelHeight: number,
    public readonly maxDimensionPx: number,
    public readonly maxMegapixels: number,
  ) {
    super(
      `Export dimensions ${pixelWidth}x${pixelHeight}px exceed the maximum of ${maxDimensionPx}px per side ` +
        `or ${maxMegapixels}MP total, even with tiling. Reduce the scale or DPI, or draw a smaller area.`,
    );
    this.name = "ExportTooLargeError";
  }
}

/**
 * Pure scale/DPI math: ground extent -> paper extent -> pixel dimensions for the export raster,
 * plus how many GPU render tiles that raster will need. Rejects only when the raster would exceed
 * the absolute output ceiling (MAX_OUTPUT_DIMENSION_PX / MAX_OUTPUT_MEGAPIXELS) - exceeding the
 * per-tile GPU limit alone just means tiling, not an error.
 */
export function computeExportPlan(
  bounds: RectangleBounds,
  scaleDenominator: number,
  dpiValue: number,
  maxTileDimensionPx: number,
): ExportPlan {
  const geometry = createRectangleGeometry(bounds);
  const groundWidthMeters = rectangleWidthMeters(geometry);
  const groundHeightMeters = rectangleHeightMeters(geometry);

  const paperWidthMeters = groundWidthMeters / scaleDenominator;
  const paperHeightMeters = groundHeightMeters / scaleDenominator;

  const pixelWidth = Math.round((paperWidthMeters * dpiValue) / METERS_PER_INCH);
  const pixelHeight = Math.round((paperHeightMeters * dpiValue) / METERS_PER_INCH);

  const exceedsAbsoluteCeiling =
    pixelWidth > MAX_OUTPUT_DIMENSION_PX ||
    pixelHeight > MAX_OUTPUT_DIMENSION_PX ||
    pixelWidth * pixelHeight > MAX_OUTPUT_MEGAPIXELS * 1_000_000;
  if (exceedsAbsoluteCeiling) {
    throw new ExportTooLargeError(
      pixelWidth,
      pixelHeight,
      MAX_OUTPUT_DIMENSION_PX,
      MAX_OUTPUT_MEGAPIXELS,
    );
  }

  const columns = Math.ceil(pixelWidth / maxTileDimensionPx);
  const rows = Math.ceil(pixelHeight / maxTileDimensionPx);

  return {
    groundWidthMeters,
    groundHeightMeters,
    pixelWidth,
    pixelHeight,
    scaleDenominator,
    dpiValue,
    tileCount: columns * rows,
  };
}

/**
 * Splits the export raster into a grid of GPU-sized render tiles, each covering a ground
 * sub-rectangle computed by linear interpolation of `bounds` (not re-measured geodesically per
 * tile), so every tile shares the exact same meters-per-pixel scale and stitches seamlessly.
 */
export function computeTileGrid(
  bounds: RectangleBounds,
  plan: ExportPlan,
  maxTileDimensionPx: number,
): TileRect[] {
  const columns = Math.ceil(plan.pixelWidth / maxTileDimensionPx);
  const rows = Math.ceil(plan.pixelHeight / maxTileDimensionPx);
  const lonSpan = bounds.east - bounds.west;
  const latSpan = bounds.north - bounds.south;

  const tiles: TileRect[] = [];
  for (let row = 0; row < rows; row++) {
    const pixelY = row * maxTileDimensionPx;
    const tileHeight = Math.min(maxTileDimensionPx, plan.pixelHeight - pixelY);
    const fracYStart = pixelY / plan.pixelHeight;
    const fracYEnd = (pixelY + tileHeight) / plan.pixelHeight;
    const north = bounds.north - fracYStart * latSpan;
    const south = bounds.north - fracYEnd * latSpan;

    for (let col = 0; col < columns; col++) {
      const pixelX = col * maxTileDimensionPx;
      const tileWidth = Math.min(maxTileDimensionPx, plan.pixelWidth - pixelX);
      const fracXStart = pixelX / plan.pixelWidth;
      const fracXEnd = (pixelX + tileWidth) / plan.pixelWidth;
      const west = bounds.west + fracXStart * lonSpan;
      const east = bounds.west + fracXEnd * lonSpan;

      tiles.push({
        bounds: { north, south, east, west },
        pixelX,
        pixelY,
        pixelWidth: tileWidth,
        pixelHeight: tileHeight,
      });
    }
  }
  return tiles;
}

/** Largest "nice" round distance (1/2/5 x 10^n meters) that fits within ~20% of the ground width. */
export function pickNiceScaleBarDistance(groundWidthMeters: number): number {
  const target = groundWidthMeters * 0.2;
  const exponent = Math.floor(Math.log10(target));
  const base = 10 ** exponent;
  const fraction = target / base;

  const niceFraction = fraction >= 5 ? 5 : fraction >= 2 ? 2 : 1;
  return niceFraction * base;
}

function formatDistanceLabel(distanceMeters: number): string {
  if (distanceMeters >= 1000) {
    const km = distanceMeters / 1000;
    return `${Number.isInteger(km) ? km : km.toFixed(1)} km`;
  }
  return `${Math.round(distanceMeters)} m`;
}

/** Draws a bottom-left scale bar and a fixed (north-up) top-right north arrow onto the export canvas. */
export function drawScaleBarAndNorthArrow(ctx: CanvasRenderingContext2D, plan: ExportPlan): void {
  const { pixelWidth, pixelHeight, groundWidthMeters } = plan;
  const pixelsPerMeter = pixelWidth / groundWidthMeters;
  const distanceMeters = pickNiceScaleBarDistance(groundWidthMeters);
  const barWidthPx = distanceMeters * pixelsPerMeter;

  const marginPx = 20;
  const barY = pixelHeight - marginPx;
  const barX = marginPx;
  const tickHeightPx = 8;

  ctx.save();
  ctx.strokeStyle = "#000000";
  ctx.fillStyle = "#000000";
  ctx.lineWidth = 2;

  ctx.beginPath();
  ctx.moveTo(barX, barY);
  ctx.lineTo(barX + barWidthPx, barY);
  ctx.moveTo(barX, barY - tickHeightPx);
  ctx.lineTo(barX, barY + tickHeightPx);
  ctx.moveTo(barX + barWidthPx, barY - tickHeightPx);
  ctx.lineTo(barX + barWidthPx, barY + tickHeightPx);
  ctx.stroke();

  ctx.font = "14px sans-serif";
  ctx.textAlign = "center";
  ctx.textBaseline = "bottom";
  ctx.fillText(formatDistanceLabel(distanceMeters), barX + barWidthPx / 2, barY - tickHeightPx - 4);

  const arrowX = pixelWidth - marginPx - 10;
  const arrowTopY = marginPx;
  const arrowBottomY = marginPx + 30;

  ctx.beginPath();
  ctx.moveTo(arrowX, arrowTopY);
  ctx.lineTo(arrowX - 8, arrowBottomY);
  ctx.lineTo(arrowX, arrowBottomY - 8);
  ctx.lineTo(arrowX + 8, arrowBottomY);
  ctx.closePath();
  ctx.fill();

  ctx.textAlign = "center";
  ctx.textBaseline = "top";
  ctx.fillText("N", arrowX, arrowBottomY + 2);
  ctx.restore();
}

/** Points the orthographic camera at one tile's ground rectangle and renders it into viewer.canvas. */
async function renderTile(viewer: Cesium.Viewer, plan: ExportPlan, tile: TileRect): Promise<void> {
  const centerLon = (tile.bounds.east + tile.bounds.west) / 2;
  const centerLat = (tile.bounds.north + tile.bounds.south) / 2;
  viewer.camera.setView({
    destination: Cesium.Cartesian3.fromDegrees(centerLon, centerLat, EXPORT_CAMERA_HEIGHT_METERS),
    orientation: { heading: 0, pitch: -Cesium.Math.PI_OVER_TWO, roll: 0 },
  });

  // Must be set AFTER setView(): Cesium's Camera.setView() calls
  // _adjustOrthographicFrustum() internally, which recomputes frustum.width
  // from the camera's ground distance - silently clobbering any width set
  // beforehand. Setting it here, after the camera is positioned, is what
  // actually sticks.
  const metersPerPixel = plan.groundWidthMeters / plan.pixelWidth;
  const frustum = viewer.camera.frustum as Cesium.OrthographicFrustum;
  frustum.aspectRatio = tile.pixelWidth / tile.pixelHeight;
  frustum.width = metersPerPixel * tile.pixelWidth;
  frustum.near = 1;
  frustum.far = EXPORT_CAMERA_HEIGHT_METERS * 4;

  viewer.canvas.width = tile.pixelWidth;
  viewer.canvas.height = tile.pixelHeight;

  await waitForTilesLoaded(viewer);
  viewer.scene.render();
}

/**
 * Renders the selected rectangle as a top-down orthographic PNG with scale bar and north arrow.
 * Cesium glue - covered by E2E, not unit tests: it drives real render loops and canvas resizing
 * that aren't meaningfully testable without a live viewer.
 *
 * Large exports are rendered as a grid of GPU-sized tiles (see computeTileGrid) and stitched into
 * one composite raster; `maxTileDimensionPx` must be the same value passed to computeExportPlan so
 * the tile grid here matches plan.tileCount. There is no supported way to move Cesium/WebGL
 * rendering off the main thread in this codebase (no OffscreenCanvas-based Cesium setup exists
 * here), so instead of a literal background thread, `onProgress` reports per-tile completion and
 * each tile's `waitForTilesLoaded` await already yields to the event loop between tiles, keeping
 * the UI responsive during multi-tile exports.
 */
export async function captureAreaImage(
  viewer: Cesium.Viewer,
  bounds: RectangleBounds,
  plan: ExportPlan,
  maxTileDimensionPx: number,
  onProgress?: (done: number, total: number) => void,
): Promise<Blob> {
  const savedView = getCameraView(viewer);
  const savedWidth = viewer.canvas.width;
  const savedHeight = viewer.canvas.height;
  const savedResolutionScale = viewer.resolutionScale;

  const tiles = computeTileGrid(bounds, plan, maxTileDimensionPx);

  const compositeCanvas = document.createElement("canvas");
  compositeCanvas.width = plan.pixelWidth;
  compositeCanvas.height = plan.pixelHeight;
  const ctx = compositeCanvas.getContext("2d");
  if (!ctx) throw new Error("Failed to acquire 2D canvas context for export composite");

  try {
    viewer.useDefaultRenderLoop = false;
    viewer.camera.switchToOrthographicFrustum();

    for (let i = 0; i < tiles.length; i++) {
      const tile = tiles[i]!;
      await renderTile(viewer, plan, tile);
      ctx.drawImage(viewer.canvas, tile.pixelX, tile.pixelY);
      onProgress?.(i + 1, tiles.length);
    }

    drawScaleBarAndNorthArrow(ctx, plan);

    return await new Promise<Blob>((resolve, reject) => {
      compositeCanvas.toBlob((blob) => {
        if (!blob) {
          reject(new Error("Failed to encode export canvas to PNG"));
          return;
        }
        resolve(blob);
      }, "image/png");
    });
  } finally {
    viewer.canvas.width = savedWidth;
    viewer.canvas.height = savedHeight;
    viewer.camera.switchToPerspectiveFrustum();
    applyCameraView(viewer, savedView);
    viewer.resolutionScale = savedResolutionScale;
    viewer.useDefaultRenderLoop = true;
  }
}

function waitForTilesLoaded(viewer: Cesium.Viewer): Promise<void> {
  return new Promise((resolve) => {
    const startTime = performance.now();

    function step(): void {
      viewer.scene.render();
      const elapsed = performance.now() - startTime;
      if (viewer.scene.globe.tilesLoaded || elapsed >= TILES_LOADED_TIMEOUT_MS) {
        resolve();
        return;
      }
      requestAnimationFrame(step);
    }

    step();
  });
}
