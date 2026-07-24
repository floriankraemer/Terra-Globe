import { describe, expect, it } from "vitest";
import { createPointGeometry } from "../domain/geometry.js";
import { NotFoundError, type PlacesRepository } from "./PlacesRepository.js";

/**
 * Shared behavioural contract every PlacesRepository adapter must satisfy.
 * Run this against every adapter (in-memory, IndexedDB, SQLite, ...) so
 * backends cannot silently diverge in behaviour.
 */
export function sharedRepositoryContractTests(
  createRepository: () => PlacesRepository | Promise<PlacesRepository>,
): void {
  describe("PlacesRepository contract", () => {
    it("creates and retrieves a folder", async () => {
      const repo = await createRepository();
      const created = await repo.createFolder({ name: "My Places", parentId: null, order: 0 });
      const fetched = await repo.getFolder(created.id);
      expect(fetched).toEqual(created);
    });

    it("returns null for a missing folder", async () => {
      const repo = await createRepository();
      expect(await repo.getFolder("does-not-exist")).toBeNull();
    });

    it("lists folders by parentId, ordered", async () => {
      const repo = await createRepository();
      const root = await repo.createFolder({ name: "Root", parentId: null, order: 0 });
      const second = await repo.createFolder({ name: "Second", parentId: root.id, order: 1 });
      const first = await repo.createFolder({ name: "First", parentId: root.id, order: 0 });

      const children = await repo.listFolders(root.id);
      expect(children.map((f) => f.id)).toEqual([first.id, second.id]);
    });

    it("updates a folder and bumps updatedAt", async () => {
      const repo = await createRepository();
      const folder = await repo.createFolder({ name: "Trips", parentId: null, order: 0 });
      const updated = await repo.updateFolder(folder.id, { name: "Travel" });
      expect(updated.name).toBe("Travel");
      expect(updated.id).toBe(folder.id);
    });

    it("throws NotFoundError updating a missing folder", async () => {
      const repo = await createRepository();
      await expect(repo.updateFolder("nope", { name: "x" })).rejects.toThrow(NotFoundError);
    });

    it("deletes an empty folder", async () => {
      const repo = await createRepository();
      const folder = await repo.createFolder({ name: "Empty", parentId: null, order: 0 });
      await repo.deleteFolder(folder.id);
      expect(await repo.getFolder(folder.id)).toBeNull();
    });

    it("refuses to delete a non-empty folder without recursive:true", async () => {
      const repo = await createRepository();
      const folder = await repo.createFolder({ name: "Parent", parentId: null, order: 0 });
      await repo.createFolder({ name: "Child", parentId: folder.id, order: 0 });
      await expect(repo.deleteFolder(folder.id)).rejects.toThrow(/not empty/i);
    });

    it("recursively deletes a folder and its descendants", async () => {
      const repo = await createRepository();
      const parent = await repo.createFolder({ name: "Parent", parentId: null, order: 0 });
      const child = await repo.createFolder({ name: "Child", parentId: parent.id, order: 0 });
      const placemark = await repo.createPlacemark({
        name: "Spot",
        folderId: child.id,
        geometry: createPointGeometry({ lon: 0, lat: 0 }),
      });

      await repo.deleteFolder(parent.id, { recursive: true });

      expect(await repo.getFolder(parent.id)).toBeNull();
      expect(await repo.getFolder(child.id)).toBeNull();
      expect(await repo.getPlacemark(placemark.id)).toBeNull();
    });

    it("creates and retrieves a placemark", async () => {
      const repo = await createRepository();
      const geometry = createPointGeometry({ lon: 13.4, lat: 52.5 });
      const created = await repo.createPlacemark({ name: "Berlin", folderId: null, geometry });
      const fetched = await repo.getPlacemark(created.id);
      expect(fetched).toEqual(created);
    });

    it("lists placemarks by folderId", async () => {
      const repo = await createRepository();
      const folder = await repo.createFolder({ name: "Cities", parentId: null, order: 0 });
      const geometry = createPointGeometry({ lon: 0, lat: 0 });
      const inFolder = await repo.createPlacemark({ name: "In", folderId: folder.id, geometry });
      await repo.createPlacemark({ name: "Out", folderId: null, geometry });

      const listed = await repo.listPlacemarks(folder.id);
      expect(listed.map((p) => p.id)).toEqual([inFolder.id]);
    });

    it("lists placemarks by folderId, ordered", async () => {
      const repo = await createRepository();
      const geometry = createPointGeometry({ lon: 0, lat: 0 });
      const second = await repo.createPlacemark({
        name: "Second",
        folderId: null,
        geometry,
        order: 1,
      });
      const first = await repo.createPlacemark({
        name: "First",
        folderId: null,
        geometry,
        order: 0,
      });

      const listed = await repo.listPlacemarks(null);
      expect(listed.map((p) => p.id)).toEqual([first.id, second.id]);
    });

    it("updates a placemark", async () => {
      const repo = await createRepository();
      const geometry = createPointGeometry({ lon: 0, lat: 0 });
      const placemark = await repo.createPlacemark({ name: "Old", folderId: null, geometry });
      const updated = await repo.updatePlacemark(placemark.id, { name: "New" });
      expect(updated.name).toBe("New");
    });

    it("throws NotFoundError updating a missing placemark", async () => {
      const repo = await createRepository();
      await expect(repo.updatePlacemark("nope", { name: "x" })).rejects.toThrow(NotFoundError);
    });

    it("deletes a placemark", async () => {
      const repo = await createRepository();
      const geometry = createPointGeometry({ lon: 0, lat: 0 });
      const placemark = await repo.createPlacemark({ name: "Gone", folderId: null, geometry });
      await repo.deletePlacemark(placemark.id);
      expect(await repo.getPlacemark(placemark.id)).toBeNull();
    });

    it("throws NotFoundError deleting a missing placemark", async () => {
      const repo = await createRepository();
      await expect(repo.deletePlacemark("nope")).rejects.toThrow(NotFoundError);
    });

    it("upserts and retrieves a style", async () => {
      const repo = await createRepository();
      const style = {
        id: "style-1",
        outlineColor: "#fff",
        outlineWidth: 1,
        outlineEnabled: true,
        fillColor: "#000",
        fillOpacity: 1,
        filled: false,
      };
      await repo.upsertStyle(style);
      expect(await repo.getStyle("style-1")).toEqual(style);
    });

    it("imports a batch of folders, placemarks and styles atomically visible", async () => {
      const repo = await createRepository();
      const geometry = createPointGeometry({ lon: 0, lat: 0 });
      const folder = {
        id: "f1",
        parentId: null,
        name: "Imported",
        visibility: true,
        order: 0,
        createdAt: "now",
        updatedAt: "now",
      };
      const placemark = {
        id: "p1",
        folderId: "f1",
        name: "Imported spot",
        geometry,
        styleId: null,
        visibility: true,
        order: 0,
        createdAt: "now",
        updatedAt: "now",
      };
      const style = {
        id: "s1",
        outlineColor: "#fff",
        outlineWidth: 1,
        outlineEnabled: true,
        fillColor: "#000",
        fillOpacity: 1,
        filled: false,
      };

      await repo.importBatch({ folders: [folder], placemarks: [placemark], styles: [style] });

      expect(await repo.getFolder("f1")).toEqual(folder);
      expect(await repo.getPlacemark("p1")).toEqual(placemark);
      expect(await repo.getStyle("s1")).toEqual(style);
    });

    it("creates, lists, and deletes screen overlays", async () => {
      const repo = await createRepository();
      const anchor = { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" } as const;
      const created = await repo.createScreenOverlay({
        name: "Legend",
        folderId: null,
        imageUrl: "legend.png",
        overlayXY: anchor,
        screenXY: anchor,
      });

      const listed = await repo.listScreenOverlays(null);
      expect(listed.map((o) => o.id)).toEqual([created.id]);

      await repo.deleteScreenOverlay(created.id);
      expect(await repo.listScreenOverlays(null)).toEqual([]);
    });

    it("imports a batch including screen overlays", async () => {
      const repo = await createRepository();
      const anchor = { x: 0, y: 1, xUnits: "fraction", yUnits: "fraction" } as const;
      const overlay = {
        id: "so1",
        folderId: null,
        name: "Watermark",
        imageUrl: "watermark.png",
        overlayXY: anchor,
        screenXY: anchor,
        visibility: true,
        order: 0,
        createdAt: "now",
        updatedAt: "now",
      };

      await repo.importBatch({
        folders: [],
        placemarks: [],
        styles: [],
        screenOverlays: [overlay],
      });

      expect(await repo.listScreenOverlays(null)).toEqual([overlay]);
    });
  });
}
