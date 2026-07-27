import { describe, expect, it } from "vitest";
import { isFolderVisible, isPlacemarkVisible } from "../../src/domain/visibility.js";
import { createFolder } from "../../src/domain/folder.js";
import { createPlacemark } from "../../src/domain/placemark.js";
import { createPointGeometry } from "../../src/domain/geometry.js";

describe("isFolderVisible", () => {
  it("is true for a visible folder with no parent", () => {
    const folder = createFolder({ name: "A", parentId: null, order: 0 });
    expect(isFolderVisible([folder], folder.id)).toBe(true);
  });

  it("is false when the folder itself is hidden", () => {
    const folder = { ...createFolder({ name: "A", parentId: null, order: 0 }), visibility: false };
    expect(isFolderVisible([folder], folder.id)).toBe(false);
  });

  it("is false when an ancestor is hidden", () => {
    const parent = {
      ...createFolder({ name: "Parent", parentId: null, order: 0 }),
      visibility: false,
    };
    const child = createFolder({ name: "Child", parentId: parent.id, order: 0 });
    expect(isFolderVisible([parent, child], child.id)).toBe(false);
  });

  it("does not affect a sibling folder", () => {
    const parent = createFolder({ name: "Parent", parentId: null, order: 0 });
    const hiddenChild = {
      ...createFolder({ name: "Hidden", parentId: parent.id, order: 0 }),
      visibility: false,
    };
    const visibleChild = createFolder({ name: "Visible", parentId: parent.id, order: 1 });
    expect(isFolderVisible([parent, hiddenChild, visibleChild], hiddenChild.id)).toBe(false);
    expect(isFolderVisible([parent, hiddenChild, visibleChild], visibleChild.id)).toBe(true);
  });

  it("is true for null (root) folderId", () => {
    expect(isFolderVisible([], null)).toBe(true);
  });
});

describe("isPlacemarkVisible", () => {
  const geometry = createPointGeometry({ lon: 0, lat: 0 });

  it("is false when the placemark itself is hidden, even if the folder is visible", () => {
    const folder = createFolder({ name: "A", parentId: null, order: 0 });
    const placemark = {
      ...createPlacemark({ name: "P", folderId: folder.id, geometry }),
      visibility: false,
    };
    expect(isPlacemarkVisible([folder], placemark)).toBe(false);
  });

  it("is false when the folder is hidden, even if the placemark itself is visible", () => {
    const folder = {
      ...createFolder({ name: "A", parentId: null, order: 0 }),
      visibility: false,
    };
    const placemark = createPlacemark({ name: "P", folderId: folder.id, geometry });
    expect(isPlacemarkVisible([folder], placemark)).toBe(false);
  });

  it("is true when both the placemark and its folder chain are visible", () => {
    const folder = createFolder({ name: "A", parentId: null, order: 0 });
    const placemark = createPlacemark({ name: "P", folderId: folder.id, geometry });
    expect(isPlacemarkVisible([folder], placemark)).toBe(true);
  });
});
