import { describe, expect, it } from "vitest";
import {
  computeExportPlan,
  ExportTooLargeError,
  pickNiceScaleBarDistance,
} from "../../src/viewer/exportImage.js";

describe("computeExportPlan", () => {
  it("computes ground extent and pixel dimensions for a given scale and DPI", () => {
    // A small rectangle straddling the equator/prime meridian, ~11.1km wide/tall
    // (1 degree of latitude/longitude at the equator is ~111.19km, so 0.1deg ~ 11.119km).
    const bounds = { north: 0.05, south: -0.05, east: 0.05, west: -0.05 };
    const plan = computeExportPlan(bounds, 25_000, 300, 20_000);

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

  it("throws ExportTooLargeError with the computed dimensions when the raster exceeds the max", () => {
    const bounds = { north: 1, south: -1, east: 1, west: -1 };

    expect(() => computeExportPlan(bounds, 1_000, 300, 1000)).toThrow(ExportTooLargeError);

    try {
      computeExportPlan(bounds, 1_000, 300, 1000);
      expect.unreachable();
    } catch (error) {
      expect(error).toBeInstanceOf(ExportTooLargeError);
      const tooLarge = error as ExportTooLargeError;
      expect(tooLarge.maxDimensionPx).toBe(1000);
      expect(tooLarge.pixelWidth).toBeGreaterThan(1000);
      expect(tooLarge.pixelHeight).toBeGreaterThan(1000);

      const plan = computeExportPlan(bounds, 1_000, 300, Number.MAX_SAFE_INTEGER);
      expect(tooLarge.pixelWidth).toBe(plan.pixelWidth);
      expect(tooLarge.pixelHeight).toBe(plan.pixelHeight);
    }
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
