import { describe, expect, it } from "vitest";
import { createPointGeometry } from "@webglobe/core";
import { FakeEntityFactory } from "../../src/entities/FakeEntityFactory.js";

describe("FakeEntityFactory", () => {
  it("generates an id when none is given", () => {
    const factory = new FakeEntityFactory();
    const handle = factory.createEntity(createPointGeometry({ lon: 0, lat: 0 }));
    expect(handle.entityId).toMatch(/^fake-entity-/);
  });

  it("uses the given id when provided", () => {
    const factory = new FakeEntityFactory();
    const handle = factory.createEntity(createPointGeometry({ lon: 0, lat: 0 }), "placemark-1");
    expect(handle.entityId).toBe("placemark-1");
    expect(factory.entities.has("placemark-1")).toBe(true);
  });
});
