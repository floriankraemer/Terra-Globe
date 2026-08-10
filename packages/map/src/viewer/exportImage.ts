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

export interface ExportPlan {
  groundWidthMeters: number;
  groundHeightMeters: number;
  pixelWidth: number;
  pixelHeight: number;
  scaleDenominator: number;
  dpiValue: number;
}

/** Thrown when the requested scale/DPI combination would exceed the allowed export dimensions. */
export class ExportTooLargeError extends Error {
  constructor(
    public readonly pixelWidth: number,
    public readonly pixelHeight: number,
    public readonly maxDimensionPx: number,
  ) {
    super(
      `Export dimensions ${pixelWidth}x${pixelHeight}px exceed the maximum of ${maxDimensionPx}px`,
    );
    this.name = "ExportTooLargeError";
  }
}

/** Pure scale/DPI math: ground extent -> paper extent -> pixel dimensions for the export raster. */
export function computeExportPlan(
  bounds: RectangleBounds,
  scaleDenominator: number,
  dpiValue: number,
  maxDimensionPx: number,
): ExportPlan {
  const geometry = createRectangleGeometry(bounds);
  const groundWidthMeters = rectangleWidthMeters(geometry);
  const groundHeightMeters = rectangleHeightMeters(geometry);

  const paperWidthMeters = groundWidthMeters / scaleDenominator;
  const paperHeightMeters = groundHeightMeters / scaleDenominator;

  const pixelWidth = Math.round((paperWidthMeters * dpiValue) / METERS_PER_INCH);
  const pixelHeight = Math.round((paperHeightMeters * dpiValue) / METERS_PER_INCH);

  if (pixelWidth > maxDimensionPx || pixelHeight > maxDimensionPx) {
    throw new ExportTooLargeError(pixelWidth, pixelHeight, maxDimensionPx);
  }

  return {
    groundWidthMeters,
    groundHeightMeters,
    pixelWidth,
    pixelHeight,
    scaleDenominator,
    dpiValue,
  };
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

/**
 * Renders the selected rectangle as a top-down orthographic PNG with scale bar and north arrow.
 * Cesium glue - covered by E2E, not unit tests: it drives real render loops and canvas resizing
 * that aren't meaningfully testable without a live viewer.
 */
export async function captureAreaImage(
  viewer: Cesium.Viewer,
  bounds: RectangleBounds,
  plan: ExportPlan,
): Promise<Blob> {
  const savedView = getCameraView(viewer);
  const savedWidth = viewer.canvas.width;
  const savedHeight = viewer.canvas.height;
  const savedResolutionScale = viewer.resolutionScale;

  try {
    viewer.useDefaultRenderLoop = false;

    viewer.camera.switchToOrthographicFrustum();
    const frustum = viewer.camera.frustum as Cesium.OrthographicFrustum;
    frustum.aspectRatio = plan.pixelWidth / plan.pixelHeight;
    frustum.width = plan.groundWidthMeters;
    frustum.near = 1;
    frustum.far = EXPORT_CAMERA_HEIGHT_METERS * 4;

    const centerLon = (bounds.east + bounds.west) / 2;
    const centerLat = (bounds.north + bounds.south) / 2;
    viewer.camera.setView({
      destination: Cesium.Cartesian3.fromDegrees(centerLon, centerLat, EXPORT_CAMERA_HEIGHT_METERS),
      orientation: { heading: 0, pitch: -Cesium.Math.PI_OVER_TWO, roll: 0 },
    });

    viewer.canvas.width = plan.pixelWidth;
    viewer.canvas.height = plan.pixelHeight;

    await waitForTilesLoaded(viewer);
    viewer.scene.render();

    const compositeCanvas = document.createElement("canvas");
    compositeCanvas.width = plan.pixelWidth;
    compositeCanvas.height = plan.pixelHeight;
    const ctx = compositeCanvas.getContext("2d");
    if (!ctx) throw new Error("Failed to acquire 2D canvas context for export composite");
    ctx.drawImage(viewer.canvas, 0, 0);
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
