import { describe, expect, it } from "vitest";
import { createFolder } from "../../src/domain/folder.js";

describe("createFolder", () => {
  it("creates a root folder with defaults", () => {
    const folder = createFolder({ name: "My Places", parentId: null, order: 0 });
    expect(folder.id).toBeTruthy();
    expect(folder.parentId).toBeNull();
    expect(folder.name).toBe("My Places");
    expect(folder.visibility).toBe(true);
    expect(folder.createdAt).toBe(folder.updatedAt);
  });

  it("rejects an empty name", () => {
    expect(() => createFolder({ name: "  ", parentId: null, order: 0 })).toThrow(/name/i);
  });

  it("accepts a parentId referencing another folder", () => {
    const folder = createFolder({ name: "Trips", parentId: "parent-1", order: 1 });
    expect(folder.parentId).toBe("parent-1");
  });
});
