import { describe, expect, it } from "vitest";
import { cssColorToKmlColor, kmlColorToCssColor } from "../../../src/kml/mapping/colorMapping.js";

describe("cssColorToKmlColor", () => {
  it("converts opaque red to KML's aabbggrr order", () => {
    expect(cssColorToKmlColor("#ff0000", 1)).toBe("ff0000ff");
  });

  it("converts opaque green", () => {
    expect(cssColorToKmlColor("#00ff00", 1)).toBe("ff00ff00");
  });

  it("converts opaque blue", () => {
    expect(cssColorToKmlColor("#0000ff", 1)).toBe("ffff0000");
  });

  it("encodes fractional opacity into the alpha channel", () => {
    expect(cssColorToKmlColor("#ffffff", 0.5)).toBe("80ffffff");
  });
});

describe("kmlColorToCssColor", () => {
  it("round-trips an opaque red", () => {
    const { hex, opacity } = kmlColorToCssColor("ff0000ff");
    expect(hex).toBe("#ff0000");
    expect(opacity).toBeCloseTo(1, 2);
  });

  it("round-trips a half-opacity white", () => {
    const { hex, opacity } = kmlColorToCssColor("80ffffff");
    expect(hex).toBe("#ffffff");
    expect(opacity).toBeCloseTo(0.5, 1);
  });
});

describe("round trip", () => {
  it("cssColorToKmlColor -> kmlColorToCssColor recovers the original color and opacity", () => {
    const { hex, opacity } = kmlColorToCssColor(cssColorToKmlColor("#3366cc", 0.75));
    expect(hex).toBe("#3366cc");
    expect(opacity).toBeCloseTo(0.75, 1);
  });
});
