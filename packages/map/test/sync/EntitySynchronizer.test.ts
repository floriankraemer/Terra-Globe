import { describe, expect, it } from "vitest";
import { InMemoryPlacesRepository, createPointGeometry, createStyle } from "@terra-globe/core";
import { FakeEntityFactory } from "../../src/entities/FakeEntityFactory.js";
import { EntitySynchronizer } from "../../src/sync/EntitySynchronizer.js";

describe("EntitySynchronizer", () => {
  it("loads placemarks from the root folder and creates matching entities", async () => {
    const repo = new InMemoryPlacesRepository();
    const placemark = await repo.createPlacemark({
      name: "Berlin",
      folderId: null,
      geometry: createPointGeometry({ lon: 13.4, lat: 52.5 }),
    });
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);

    await sync.loadAll();

    expect(factory.entities.has(placemark.id)).toBe(true);
  });

  it("recursively loads placemarks nested inside folders", async () => {
    const repo = new InMemoryPlacesRepository();
    const folder = await repo.createFolder({ name: "Trips", parentId: null, order: 0 });
    const nested = await repo.createFolder({ name: "2026", parentId: folder.id, order: 0 });
    const placemark = await repo.createPlacemark({
      name: "Camp",
      folderId: nested.id,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);

    await sync.loadAll();

    expect(factory.entities.has(placemark.id)).toBe(true);
  });

  it("creates a matching entity when persisting a newly drawn placemark", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);

    const placemark = await sync.persistPlacemark({
      name: "New spot",
      folderId: null,
      geometry: createPointGeometry({ lon: 1, lat: 1 }),
    });

    expect(factory.entities.has(placemark.id)).toBe(true);
    expect(await repo.getPlacemark(placemark.id)).toEqual(placemark);
  });

  it("renders entities for a batch of already-persisted placemarks (e.g. after an import)", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await repo.createPlacemark({
      name: "Imported",
      folderId: null,
      geometry: createPointGeometry({ lon: 5, lat: 5 }),
    });

    await sync.renderPlacemarks([placemark]);

    expect(factory.entities.has(placemark.id)).toBe(true);
  });

  it("resolves and applies the placemark's style when rendering an entity", async () => {
    const repo = new InMemoryPlacesRepository();
    const style = createStyle({
      outlineColor: "#00ff00",
      outlineWidth: 3,
      fillColor: "#ff0000",
      fillOpacity: 0.8,
    });
    await repo.upsertStyle(style);
    const placemark = await repo.createPlacemark({
      name: "Colored",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
      styleId: style.id,
    });
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);

    await sync.loadAll();

    expect(factory.entities.get(placemark.id)?.style).toEqual(style);
  });

  it("updates a placemark's fields, style and live entity", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await sync.persistPlacemark({
      name: "Old name",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });

    const updated = await sync.updatePlacemark(placemark.id, {
      name: "New name",
      description: "A description",
    });

    expect(updated.name).toBe("New name");
    expect(updated.description).toBe("A description");
    expect(await repo.getPlacemark(placemark.id)).toEqual(updated);
  });

  it("creates and applies a new style when updating a placemark's color", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await sync.persistPlacemark({
      name: "Spot",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });

    const updated = await sync.setPlacemarkColor(placemark.id, "#00ff00");

    expect(updated.styleId).toBeTruthy();
    const style = await repo.getStyle(updated.styleId!);
    expect(style?.fillColor).toBe("#00ff00");
    expect(factory.entities.get(placemark.id)?.style?.fillColor).toBe("#00ff00");
  });

  it("savePlacemarkEdits updates name, description and color in one write", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await sync.persistPlacemark({
      name: "Old",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });

    const updated = await sync.savePlacemarkEdits(placemark.id, {
      name: "New",
      description: "Desc",
      style: {
        outlineEnabled: true,
        outlineColor: "#00ff00",
        outlineWidth: 3,
        filled: true,
        fillColor: "#00ff00",
      },
    });

    expect(updated.name).toBe("New");
    expect(updated.description).toBe("Desc");
    const style = await repo.getStyle(updated.styleId!);
    expect(style?.fillColor).toBe("#00ff00");
    expect(style?.outlineWidth).toBe(3);
    expect(style?.filled).toBe(true);
    expect(await repo.getPlacemark(placemark.id)).toEqual(updated);
  });

  it("savePlacemarkEdits without a style leaves the existing style untouched", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await sync.persistPlacemark({
      name: "Old",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });
    await sync.setPlacemarkColor(placemark.id, "#00ff00");

    const updated = await sync.savePlacemarkEdits(placemark.id, { name: "New" });

    expect(updated.name).toBe("New");
    const style = await repo.getStyle(updated.styleId!);
    expect(style?.fillColor).toBe("#00ff00");
  });

  it("previewPlacemark updates the live entity without persisting anything", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await sync.persistPlacemark({
      name: "Old",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });

    sync.previewPlacemark(placemark.id, placemark.geometry, "Draft name", {
      outlineEnabled: true,
      outlineColor: "#00ff00",
      outlineWidth: 3,
      filled: true,
      fillColor: "#00ff00",
    });

    expect(factory.entities.get(placemark.id)?.name).toBe("Draft name");
    expect(factory.entities.get(placemark.id)?.style?.fillColor).toBe("#00ff00");
    const stored = await repo.getPlacemark(placemark.id);
    expect(stored?.name).toBe("Old");
    expect(stored?.styleId).toBeNull();
  });

  it("removes the entity when a placemark is deleted", async () => {
    const repo = new InMemoryPlacesRepository();
    const factory = new FakeEntityFactory();
    const sync = new EntitySynchronizer(repo, factory);
    const placemark = await sync.persistPlacemark({
      name: "Gone",
      folderId: null,
      geometry: createPointGeometry({ lon: 0, lat: 0 }),
    });

    await sync.deletePlacemark(placemark.id);

    expect(factory.entities.has(placemark.id)).toBe(false);
    expect(await repo.getPlacemark(placemark.id)).toBeNull();
  });
});
