import { describe, expect, it } from "vitest";
import { ringColor, ringRadii } from "../../src/entities/DistanceRingsRenderer.js";

describe("ringRadii", () => {
  it("returns exact multiples of spacing up to and including the disc radius", () => {
    expect(ringRadii(100, 500)).toEqual([100, 200, 300, 400, 500]);
  });

  it("truncates a partial ring when the disc radius isn't an exact multiple", () => {
    expect(ringRadii(100, 450)).toEqual([100, 200, 300, 400]);
  });

  it("returns no rings when spacing is zero or negative", () => {
    expect(ringRadii(0, 500)).toEqual([]);
    expect(ringRadii(-10, 500)).toEqual([]);
  });
});

describe("ringColor", () => {
  it("colors the innermost ring green", () => {
    const c = ringColor(0, 5);
    expect(c.red).toBeCloseTo(34 / 255, 2);
    expect(c.green).toBeCloseTo(197 / 255, 2);
    expect(c.blue).toBeCloseTo(94 / 255, 2);
  });

  it("colors the outermost ring red", () => {
    const c = ringColor(4, 5);
    expect(c.red).toBeCloseTo(239 / 255, 2);
    expect(c.green).toBeCloseTo(68 / 255, 2);
    expect(c.blue).toBeCloseTo(68 / 255, 2);
  });

  it("interpolates for a single ring by treating it as fully outermost (red)", () => {
    const c = ringColor(0, 1);
    expect(c.red).toBeCloseTo(239 / 255, 2);
  });
});
