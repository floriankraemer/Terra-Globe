import { describe, expect, it } from "vitest";
import { createStyle } from "../../../src/domain/style.js";
import { kmlDataToStyle, styleToKmlData } from "../../../src/kml/mapping/styleKmlMapping.js";

describe("styleToKmlData", () => {
  it("converts line and fill colors to KML aabbggrr, folding fillOpacity into polyColor's alpha", () => {
    const style = createStyle({
      outlineColor: "#ff0000",
      outlineWidth: 2,
      fillColor: "#00ff00",
      fillOpacity: 0.5,
    });
    const data = styleToKmlData(style);
    expect(data.id).toBe(style.id);
    expect(data.lineColorKml).toBe("ff0000ff");
    expect(data.lineWidth).toBe(2);
    expect(data.polyColorKml).toBe("8000ff00");
  });

  it("carries over outlineEnabled and filled flags", () => {
    const style = createStyle({
      outlineColor: "#fff",
      outlineWidth: 1,
      fillColor: "#000",
      fillOpacity: 1,
      outlineEnabled: false,
      filled: true,
    });
    const data = styleToKmlData(style);
    expect(data.outlineEnabled).toBe(false);
    expect(data.filled).toBe(true);
  });

  it("carries over optional icon/label fields", () => {
    const style = createStyle({
      outlineColor: "#ffffff",
      outlineWidth: 1,
      fillColor: "#000000",
      fillOpacity: 1,
      iconUrl: "https://example.com/icon.png",
      iconScale: 1.5,
      labelColor: "#123456",
    });
    const data = styleToKmlData(style);
    expect(data.iconUrl).toBe("https://example.com/icon.png");
    expect(data.iconScale).toBe(1.5);
    expect(data.labelColorKml).toBe("ff563412");
  });
});

describe("kmlDataToStyle", () => {
  it("round-trips styleToKmlData output back into an equivalent Style", () => {
    const original = createStyle({
      outlineColor: "#3366cc",
      outlineWidth: 3,
      fillColor: "#cc6633",
      fillOpacity: 0.75,
      outlineEnabled: false,
      filled: true,
    });
    const restored = kmlDataToStyle(styleToKmlData(original));
    expect(restored.id).toBe(original.id);
    expect(restored.outlineColor).toBe(original.outlineColor);
    expect(restored.outlineWidth).toBe(original.outlineWidth);
    expect(restored.fillColor).toBe(original.fillColor);
    expect(restored.fillOpacity).toBeCloseTo(original.fillOpacity, 1);
    expect(restored.outlineEnabled).toBe(original.outlineEnabled);
    expect(restored.filled).toBe(original.filled);
  });
});
