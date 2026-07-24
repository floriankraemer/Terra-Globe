import Dexie, { type Table } from "dexie";
import {
  NotFoundError,
  createFolder,
  createPlacemark,
  createScreenOverlay,
  type DeleteFolderOptions,
  type Folder,
  type ImportBatchPayload,
  type NewFolder,
  type NewPlacemark,
  type NewScreenOverlay,
  type Placemark,
  type PlacesRepository,
  type ScreenOverlay,
  type Style,
} from "@terra-globe/core";

class TerraGlobeDexie extends Dexie {
  folders!: Table<Folder, string>;
  placemarks!: Table<Placemark, string>;
  styles!: Table<Style, string>;
  screenOverlays!: Table<ScreenOverlay, string>;

  constructor(dbName: string) {
    super(dbName);
    this.version(1).stores({
      folders: "id, parentId, order",
      placemarks: "id, folderId",
      styles: "id",
    });
    this.version(2).stores({
      folders: "id, parentId, order",
      placemarks: "id, folderId",
      styles: "id",
      screenOverlays: "id, folderId",
    });
  }
}

export class IndexedDbPlacesRepository implements PlacesRepository {
  private readonly db: TerraGlobeDexie;

  constructor(dbName = "terra-globe") {
    this.db = new TerraGlobeDexie(dbName);
  }

  async getFolder(id: string): Promise<Folder | null> {
    return (await this.db.folders.get(id)) ?? null;
  }

  async listFolders(parentId: string | null): Promise<Folder[]> {
    // IndexedDB key ranges cannot query for `null` directly, so root-level
    // (parentId === null) lookups fall back to an in-memory filter.
    const folders =
      parentId === null
        ? await this.db.folders.filter((f) => f.parentId === null).toArray()
        : await this.db.folders.where("parentId").equals(parentId).toArray();
    return folders.sort((a, b) => a.order - b.order);
  }

  async createFolder(input: NewFolder): Promise<Folder> {
    const folder = createFolder(input);
    await this.db.folders.add(folder);
    return folder;
  }

  async updateFolder(id: string, patch: Partial<Omit<Folder, "id">>): Promise<Folder> {
    const existing = await this.db.folders.get(id);
    if (!existing) throw new NotFoundError("Folder", id);
    const updated: Folder = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    await this.db.folders.put(updated);
    return updated;
  }

  async deleteFolder(id: string, opts?: DeleteFolderOptions): Promise<void> {
    const existing = await this.db.folders.get(id);
    if (!existing) throw new NotFoundError("Folder", id);

    const childFolders = await this.db.folders.where("parentId").equals(id).toArray();
    const childPlacemarks = await this.db.placemarks.where("folderId").equals(id).toArray();

    if ((childFolders.length > 0 || childPlacemarks.length > 0) && !opts?.recursive) {
      throw new Error(`Folder ${id} is not empty; pass { recursive: true } to delete it`);
    }

    for (const child of childFolders) {
      await this.deleteFolder(child.id, { recursive: true });
    }
    await this.db.placemarks.bulkDelete(childPlacemarks.map((p) => p.id));
    await this.db.folders.delete(id);
  }

  async getPlacemark(id: string): Promise<Placemark | null> {
    return (await this.db.placemarks.get(id)) ?? null;
  }

  async listPlacemarks(folderId: string | null): Promise<Placemark[]> {
    const placemarks =
      folderId === null
        ? await this.db.placemarks.filter((p) => p.folderId === null).toArray()
        : await this.db.placemarks.where("folderId").equals(folderId).toArray();
    return placemarks.sort((a, b) => a.order - b.order);
  }

  async createPlacemark(input: NewPlacemark): Promise<Placemark> {
    const placemark = createPlacemark(input);
    await this.db.placemarks.add(placemark);
    return placemark;
  }

  async updatePlacemark(id: string, patch: Partial<Omit<Placemark, "id">>): Promise<Placemark> {
    const existing = await this.db.placemarks.get(id);
    if (!existing) throw new NotFoundError("Placemark", id);
    const updated: Placemark = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    await this.db.placemarks.put(updated);
    return updated;
  }

  async deletePlacemark(id: string): Promise<void> {
    const existing = await this.db.placemarks.get(id);
    if (!existing) throw new NotFoundError("Placemark", id);
    await this.db.placemarks.delete(id);
  }

  async getStyle(id: string): Promise<Style | null> {
    return (await this.db.styles.get(id)) ?? null;
  }

  async upsertStyle(style: Style): Promise<Style> {
    await this.db.styles.put(style);
    return style;
  }

  async listScreenOverlays(folderId: string | null): Promise<ScreenOverlay[]> {
    const overlays =
      folderId === null
        ? await this.db.screenOverlays.filter((o) => o.folderId === null).toArray()
        : await this.db.screenOverlays.where("folderId").equals(folderId).toArray();
    return overlays.sort((a, b) => a.order - b.order);
  }

  async createScreenOverlay(input: NewScreenOverlay): Promise<ScreenOverlay> {
    const overlay = createScreenOverlay(input);
    await this.db.screenOverlays.add(overlay);
    return overlay;
  }

  async deleteScreenOverlay(id: string): Promise<void> {
    await this.db.screenOverlays.delete(id);
  }

  async importBatch(payload: ImportBatchPayload): Promise<void> {
    await this.db.transaction(
      "rw",
      this.db.folders,
      this.db.placemarks,
      this.db.styles,
      this.db.screenOverlays,
      async () => {
        await this.db.folders.bulkPut(payload.folders);
        await this.db.placemarks.bulkPut(payload.placemarks);
        await this.db.styles.bulkPut(payload.styles);
        await this.db.screenOverlays.bulkPut(payload.screenOverlays ?? []);
      },
    );
  }
}
