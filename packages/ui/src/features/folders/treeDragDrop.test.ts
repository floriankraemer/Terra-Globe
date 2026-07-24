import { describe, expect, it } from "vitest";
import type { Folder, Placemark } from "@webglobe/core";
import { resolveDrop, resolveDropOnContainer, resolveDropPosition } from "./treeDragDrop.js";

function folder(overrides: Partial<Folder>): Folder {
  return {
    id: "f1",
    parentId: null,
    name: "Folder",
    visibility: true,
    order: 0,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

function placemark(overrides: Partial<Placemark>): Placemark {
  return {
    id: "p1",
    folderId: null,
    name: "Placemark",
    geometry: { type: "Point", coordinates: { lon: 0, lat: 0 } },
    styleId: null,
    visibility: true,
    order: 0,
    createdAt: "now",
    updatedAt: "now",
    ...overrides,
  };
}

describe("resolveDropPosition", () => {
  it("resolves the middle band to 'inside' when allowed", () => {
    expect(resolveDropPosition(0.5, true)).toBe("inside");
    expect(resolveDropPosition(0.25, true)).toBe("inside");
    expect(resolveDropPosition(0.75, true)).toBe("inside");
  });

  it("never resolves to 'inside' when not allowed (e.g. over a placemark row)", () => {
    expect(resolveDropPosition(0.5, false)).not.toBe("inside");
  });

  it("resolves the top half to 'before' and bottom half to 'after'", () => {
    expect(resolveDropPosition(0.1, false)).toBe("before");
    expect(resolveDropPosition(0.9, false)).toBe("after");
    expect(resolveDropPosition(0.1, true)).toBe("before");
    expect(resolveDropPosition(0.9, true)).toBe("after");
  });
});

describe("resolveDrop", () => {
  it("dropping a placemark 'inside' a folder moves it in, appended at the end", () => {
    const folders = [folder({ id: "f1" })];
    const placemarks = [
      placemark({ id: "p1", folderId: null }),
      placemark({ id: "existing", folderId: "f1", order: 0 }),
    ];

    const result = resolveDrop(
      { kind: "placemark", id: "p1" },
      { kind: "folder", id: "f1", parentId: null },
      "inside",
      folders,
      placemarks,
    );

    expect(result).toEqual({ kind: "placemark", id: "p1", parentId: "f1", index: 1 });
  });

  it("dropping a placemark 'before' another placemark reorders within the same parent", () => {
    const placemarks = [
      placemark({ id: "p1", folderId: null, order: 0 }),
      placemark({ id: "p2", folderId: null, order: 1 }),
    ];

    const result = resolveDrop(
      { kind: "placemark", id: "p2" },
      { kind: "placemark", id: "p1", parentId: null },
      "before",
      [],
      placemarks,
    );

    expect(result).toEqual({ kind: "placemark", id: "p2", parentId: null, index: 0 });
  });

  it("dropping a placemark 'after' another placemark reorders after it", () => {
    const placemarks = [
      placemark({ id: "p1", folderId: null, order: 0 }),
      placemark({ id: "p2", folderId: null, order: 1 }),
      placemark({ id: "p3", folderId: null, order: 2 }),
    ];

    const result = resolveDrop(
      { kind: "placemark", id: "p3" },
      { kind: "placemark", id: "p1", parentId: null },
      "after",
      [],
      placemarks,
    );

    expect(result).toEqual({ kind: "placemark", id: "p3", parentId: null, index: 1 });
  });

  it("dropping a folder onto itself is a no-op", () => {
    const folders = [folder({ id: "f1" })];
    const result = resolveDrop(
      { kind: "folder", id: "f1" },
      { kind: "folder", id: "f1", parentId: null },
      "inside",
      folders,
      [],
    );
    expect(result).toBeNull();
  });

  it("dropping before/after a row of the other kind reparents, appended at the end of the dragged item's own list", () => {
    const folders = [folder({ id: "target-folder", parentId: "dest" })];
    const placemarks = [
      placemark({ id: "dragged", folderId: null }),
      placemark({ id: "other", folderId: "dest", order: 0 }),
    ];

    const result = resolveDrop(
      { kind: "placemark", id: "dragged" },
      { kind: "folder", id: "target-folder", parentId: "dest" },
      "before",
      folders,
      placemarks,
    );

    expect(result).toEqual({ kind: "placemark", id: "dragged", parentId: "dest", index: 1 });
  });
});

describe("resolveDropOnContainer", () => {
  it("appends the dragged item at the end of the target container's same-kind list", () => {
    const folders = [
      folder({ id: "a", parentId: "root", order: 0 }),
      folder({ id: "b", parentId: "root", order: 1 }),
    ];

    const result = resolveDropOnContainer({ kind: "folder", id: "a" }, "root", folders, []);

    expect(result).toEqual({ kind: "folder", id: "a", parentId: "root", index: 1 });
  });
});
