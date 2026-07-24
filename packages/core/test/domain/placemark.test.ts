import { describe, expect, it } from "vitest";
import { createPointGeometry } from "../../src/domain/geometry.js";
import { createPlacemark } from "../../src/domain/placemark.js";

describe("createPlacemark", () => {
  const geometry = createPointGeometry({ lon: 13.4, lat: 52.5 });

  it("creates a placemark with defaults", () => {
    const placemark = createPlacemark({ name: "Berlin", folderId: null, geometry });
    expect(placemark.id).toBeTruthy();
    expect(placemark.folderId).toBeNull();
    expect(placemark.name).toBe("Berlin");
    expect(placemark.geometry).toEqual(geometry);
    expect(placemark.styleId).toBeNull();
    expect(placemark.visibility).toBe(true);
    expect(placemark.createdAt).toBe(placemark.updatedAt);
  });

  it("rejects an empty name", () => {
    expect(() => createPlacemark({ name: "", folderId: null, geometry })).toThrow(/name/i);
  });

  it("accepts an optional description and styleId", () => {
    const placemark = createPlacemark({
      name: "Berlin",
      folderId: "folder-1",
      geometry,
      description: "Capital of Germany",
      styleId: "style-1",
    });
    expect(placemark.description).toBe("Capital of Germany");
    expect(placemark.styleId).toBe("style-1");
  });
});
