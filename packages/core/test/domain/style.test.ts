import { describe, expect, it } from "vitest";
import { createStyle } from "../../src/domain/style.js";

describe("createStyle", () => {
  it("creates a style with the given properties and a generated id", () => {
    const style = createStyle({
      outlineColor: "#ff0000",
      outlineWidth: 2,
      fillColor: "#00ff00",
      fillOpacity: 0.5,
    });
    expect(style.id).toBeTruthy();
    expect(style).toMatchObject({
      outlineColor: "#ff0000",
      outlineWidth: 2,
      fillColor: "#00ff00",
      fillOpacity: 0.5,
    });
  });

  it("defaults to outlineEnabled: true and filled: false", () => {
    const style = createStyle({
      outlineColor: "#fff",
      outlineWidth: 1,
      fillColor: "#fff",
      fillOpacity: 1,
    });
    expect(style.outlineEnabled).toBe(true);
    expect(style.filled).toBe(false);
  });

  it("allows overriding outlineEnabled and filled", () => {
    const style = createStyle({
      outlineColor: "#fff",
      outlineWidth: 1,
      fillColor: "#fff",
      fillOpacity: 1,
      outlineEnabled: false,
      filled: true,
    });
    expect(style.outlineEnabled).toBe(false);
    expect(style.filled).toBe(true);
  });

  it("rejects fillOpacity outside [0, 1]", () => {
    expect(() =>
      createStyle({ outlineColor: "#fff", outlineWidth: 1, fillColor: "#fff", fillOpacity: 1.5 }),
    ).toThrow(/opacity/i);
  });

  it("rejects a negative outlineWidth", () => {
    expect(() =>
      createStyle({ outlineColor: "#fff", outlineWidth: -1, fillColor: "#fff", fillOpacity: 1 }),
    ).toThrow(/outlineWidth/i);
  });

  it("allows optional icon fields", () => {
    const style = createStyle({
      outlineColor: "#fff",
      outlineWidth: 1,
      fillColor: "#fff",
      fillOpacity: 1,
      iconUrl: "https://example.com/icon.png",
      iconScale: 1.2,
      labelColor: "#000",
    });
    expect(style.iconUrl).toBe("https://example.com/icon.png");
    expect(style.iconScale).toBe(1.2);
    expect(style.labelColor).toBe("#000");
  });
});
