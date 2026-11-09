import { describe, expect, it } from "vitest";
import {
  computeExportPlan,
  computeTileGrid,
  ExportTooLargeError,
  MAX_OUTPUT_DIMENSION_PX,
  MAX_OUTPUT_MEGAPIXELS,
  pickNiceScaleBarDistance,
} from "../../src/viewer/exportImage.js";

describe("computeExportPlan", () => {
  it("computes ground extent and pixel dimensions for a given scale and DPI", () => {
    // A small rectangle straddling the equator/prime meridian, ~11.1km wide/tall
    // (1 degree of latitude/longitude at the equator is ~111.19km, so 0.1deg ~ 11.119km).
    const bounds = { north: 0.05, south: -0.05, east: 0.05, west: -0.05 };
    const plan = computeExportPlan(bounds, 25_000, 300, 4096);

    expect(plan.groundWidthMeters).toBeCloseTo(11_119, -1);
    expect(plan.groundHeightMeters).toBeCloseTo(11_119, -1);

    // paperWidthMeters = 11119 / 25000 = 0.44476m; pixelWidth = 0.44476 * 300 / 0.0254
    const expectedPixelWidth = Math.round(((plan.groundWidthMeters / 25_000) * 300) / 0.0254);
    expect(plan.pixelWidth).toBe(expectedPixelWidth);
    expect(plan.pixelWidth).toBeGreaterThan(5000);
    expect(plan.pixelWidth).toBeLessThan(5300);
    expect(plan.scaleDenominator).toBe(25_000);
    expect(plan.dpiValue).toBe(300);
  });

  it("does not tile when the raster fits within the GPU per-tile limit", () => {
    const bounds = { north: 0.05, south: -0.05, east: 0.05, west: -0.05 };
    const plan = computeExportPlan(bounds, 50_000, 300, 4096);

    expect(plan.pixelWidth).toBeLessThan(4096);
    expect(plan.pixelHeight).toBeLessThan(4096);
    expect(plan.tileCount).toBe(1);
  });

  it("tiles when the raster exceeds the GPU per-tile limit but fits the absolute output ceiling", () => {
    const bounds = { north: 0.05, south: -0.05, east: 0.05, west: -0.05 };
    // Same raster as above (~5252x5252px), but with a much smaller per-tile limit.
    const plan = computeExportPlan(bounds, 25_000, 300, 2000);

    expect(plan.pixelWidth).toBeGreaterThan(2000);
    expect(plan.pixelHeight).toBeGreaterThan(2000);
    expect(plan.pixelWidth).toBeLessThan(MAX_OUTPUT_DIMENSION_PX);
    expect(plan.pixelHeight).toBeLessThan(MAX_OUTPUT_DIMENSION_PX);
    expect(plan.tileCount).toBeGreaterThan(1);
    // 5252px / 2000px per tile -> 3 columns, 3 rows.
    expect(plan.tileCount).toBe(9);
  });

  it("throws ExportTooLargeError when the raster exceeds the absolute output ceiling, even with tiling", () => {
    const bounds = { north: 1, south: -1, east: 1, west: -1 };

    // A generous per-tile limit doesn't help - the raster itself is too big to ever assemble.
    expect(() => computeExportPlan(bounds, 1_000, 300, 4096)).toThrow(ExportTooLargeError);

    try {
      computeExportPlan(bounds, 1_000, 300, 4096);
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(ExportTooLargeError);
      const tooLarge = error as ExportTooLargeError;
      expect(tooLarge.maxDimensionPx).toBe(MAX_OUTPUT_DIMENSION_PX);
      expect(tooLarge.maxMegapixels).toBe(MAX_OUTPUT_MEGAPIXELS);
      expect(tooLarge.pixelWidth).toBeGreaterThan(MAX_OUTPUT_DIMENSION_PX);
      expect(tooLarge.pixelHeight).toBeGreaterThan(MAX_OUTPUT_DIMENSION_PX);
    }
  });
});

describe("computeTileGrid", () => {
  it("returns a single tile spanning the whole raster when no tiling is needed", () => {
    const bounds = { north: 0.05, south: -0.05, east: 0.05, west: -0.05 };
    const plan = computeExportPlan(bounds, 50_000, 300, 4096);
    const tiles = computeTileGrid(bounds, plan, 4096);

    expect(tiles).toHaveLength(1);
    expect(tiles[0]).toEqual({
      bounds,
      pixelX: 0,
      pixelY: 0,
      pixelWidth: plan.pixelWidth,
      pixelHeight: plan.pixelHeight,
    });
  });

  it("splits into a seamless grid whose tiles cover the full raster with no gaps or overlaps", () => {
    const bounds = { north: 0.05, south: -0.05, east: 0.05, west: -0.05 };
    const plan = computeExportPlan(bounds, 25_000, 300, 2000);
    const tiles = computeTileGrid(bounds, plan, 2000);

    expect(tiles).toHaveLength(plan.tileCount);

    const totalPixels = tiles.reduce((sum, t) => sum + t.pixelWidth * t.pixelHeight, 0);
    expect(totalPixels).toBe(plan.pixelWidth * plan.pixelHeight);

    // Every tile's ground bounds must fall within the overall bounds.
    for (const tile of tiles) {
      expect(tile.bounds.west).toBeGreaterThanOrEqual(bounds.west);
      expect(tile.bounds.east).toBeLessThanOrEqual(bounds.east);
      expect(tile.bounds.south).toBeGreaterThanOrEqual(bounds.south);
      expect(tile.bounds.north).toBeLessThanOrEqual(bounds.north);
    }

    // The top-left tile starts exactly at the overall bounds' NW corner.
    const topLeft = tiles[0]!;
    expect(topLeft.pixelX).toBe(0);
    expect(topLeft.pixelY).toBe(0);
    expect(topLeft.bounds.west).toBeCloseTo(bounds.west, 10);
    expect(topLeft.bounds.north).toBeCloseTo(bounds.north, 10);

    // The bottom-right tile ends exactly at the overall bounds' SE corner.
    const bottomRight = tiles[tiles.length - 1]!;
    expect(bottomRight.pixelX + bottomRight.pixelWidth).toBe(plan.pixelWidth);
    expect(bottomRight.pixelY + bottomRight.pixelHeight).toBe(plan.pixelHeight);
    expect(bottomRight.bounds.east).toBeCloseTo(bounds.east, 10);
    expect(bottomRight.bounds.south).toBeCloseTo(bounds.south, 10);
  });
});

describe("pickNiceScaleBarDistance", () => {
  it.each([
    [100, 20],
    [1000, 200],
    [10_000, 2000],
    [50_000, 10_000],
    [500, 100],
    [37, 5],
  ])("picks a nice round distance for a %sm-wide ground extent", (groundWidthMeters, expected) => {
    const distance = pickNiceScaleBarDistance(groundWidthMeters);
    expect(distance).toBe(expected);
    expect(distance).toBeLessThanOrEqual(groundWidthMeters * 0.2);
  });

  it("picks the largest nice number that fits, not a smaller one", () => {
    // 20% of 9999 is 1999.8, so the largest nice (1/2/5 x 10^n) distance that fits is 1000,
    // not 500.
    const distance = pickNiceScaleBarDistance(9999);
    expect(distance).toBe(1000);
  });
});
