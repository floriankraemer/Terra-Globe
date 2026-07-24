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
} from "@webglobe/core";
import { migrate } from "./migrate.js";
import type { SqlDriver } from "./SqlDriver.js";

interface Row {
  data: string;
}

export class SqlitePlacesRepository implements PlacesRepository {
  private readonly ready: Promise<void>;

  constructor(private readonly driver: SqlDriver) {
    this.ready = migrate(driver);
  }

  private async init(): Promise<void> {
    await this.ready;
  }

  async getFolder(id: string): Promise<Folder | null> {
    await this.init();
    const rows = await this.driver.select<Row>("SELECT data FROM folders WHERE id = ?", [id]);
    return rows[0] ? (JSON.parse(rows[0].data) as Folder) : null;
  }

  async listFolders(parentId: string | null): Promise<Folder[]> {
    await this.init();
    const rows = await this.driver.select<Row>(
      parentId === null
        ? "SELECT data FROM folders WHERE parent_id IS NULL"
        : "SELECT data FROM folders WHERE parent_id = ?",
      parentId === null ? [] : [parentId],
    );
    const folders = rows.map((r) => JSON.parse(r.data) as Folder);
    return folders.sort((a, b) => a.order - b.order);
  }

  async createFolder(input: NewFolder): Promise<Folder> {
    await this.init();
    const folder = createFolder(input);
    await this.driver.execute("INSERT INTO folders (id, parent_id, data) VALUES (?, ?, ?)", [
      folder.id,
      folder.parentId,
      JSON.stringify(folder),
    ]);
    return folder;
  }

  async updateFolder(id: string, patch: Partial<Omit<Folder, "id">>): Promise<Folder> {
    await this.init();
    const existing = await this.getFolder(id);
    if (!existing) throw new NotFoundError("Folder", id);
    const updated: Folder = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    await this.driver.execute("UPDATE folders SET parent_id = ?, data = ? WHERE id = ?", [
      updated.parentId,
      JSON.stringify(updated),
      id,
    ]);
    return updated;
  }

  async deleteFolder(id: string, opts?: DeleteFolderOptions): Promise<void> {
    await this.init();
    const existing = await this.getFolder(id);
    if (!existing) throw new NotFoundError("Folder", id);

    const childFolders = await this.listFolders(id);
    const childPlacemarks = await this.listPlacemarks(id);

    if ((childFolders.length > 0 || childPlacemarks.length > 0) && !opts?.recursive) {
      throw new Error(`Folder ${id} is not empty; pass { recursive: true } to delete it`);
    }

    for (const child of childFolders) {
      await this.deleteFolder(child.id, { recursive: true });
    }
    for (const placemark of childPlacemarks) {
      await this.driver.execute("DELETE FROM placemarks WHERE id = ?", [placemark.id]);
    }
    await this.driver.execute("DELETE FROM folders WHERE id = ?", [id]);
  }

  async getPlacemark(id: string): Promise<Placemark | null> {
    await this.init();
    const rows = await this.driver.select<Row>("SELECT data FROM placemarks WHERE id = ?", [id]);
    return rows[0] ? (JSON.parse(rows[0].data) as Placemark) : null;
  }

  async listPlacemarks(folderId: string | null): Promise<Placemark[]> {
    await this.init();
    const rows = await this.driver.select<Row>(
      folderId === null
        ? "SELECT data FROM placemarks WHERE folder_id IS NULL"
        : "SELECT data FROM placemarks WHERE folder_id = ?",
      folderId === null ? [] : [folderId],
    );
    return rows.map((r) => JSON.parse(r.data) as Placemark).sort((a, b) => a.order - b.order);
  }

  async createPlacemark(input: NewPlacemark): Promise<Placemark> {
    await this.init();
    const placemark = createPlacemark(input);
    await this.driver.execute("INSERT INTO placemarks (id, folder_id, data) VALUES (?, ?, ?)", [
      placemark.id,
      placemark.folderId,
      JSON.stringify(placemark),
    ]);
    return placemark;
  }

  async updatePlacemark(id: string, patch: Partial<Omit<Placemark, "id">>): Promise<Placemark> {
    await this.init();
    const existing = await this.getPlacemark(id);
    if (!existing) throw new NotFoundError("Placemark", id);
    const updated: Placemark = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    await this.driver.execute("UPDATE placemarks SET folder_id = ?, data = ? WHERE id = ?", [
      updated.folderId,
      JSON.stringify(updated),
      id,
    ]);
    return updated;
  }

  async deletePlacemark(id: string): Promise<void> {
    await this.init();
    const existing = await this.getPlacemark(id);
    if (!existing) throw new NotFoundError("Placemark", id);
    await this.driver.execute("DELETE FROM placemarks WHERE id = ?", [id]);
  }

  async getStyle(id: string): Promise<Style | null> {
    await this.init();
    const rows = await this.driver.select<Row>("SELECT data FROM styles WHERE id = ?", [id]);
    return rows[0] ? (JSON.parse(rows[0].data) as Style) : null;
  }

  async upsertStyle(style: Style): Promise<Style> {
    await this.init();
    await this.driver.execute("INSERT OR REPLACE INTO styles (id, data) VALUES (?, ?)", [
      style.id,
      JSON.stringify(style),
    ]);
    return style;
  }

  async listScreenOverlays(folderId: string | null): Promise<ScreenOverlay[]> {
    await this.init();
    const rows = await this.driver.select<Row>(
      folderId === null
        ? "SELECT data FROM screen_overlays WHERE folder_id IS NULL"
        : "SELECT data FROM screen_overlays WHERE folder_id = ?",
      folderId === null ? [] : [folderId],
    );
    return rows.map((r) => JSON.parse(r.data) as ScreenOverlay).sort((a, b) => a.order - b.order);
  }

  async createScreenOverlay(input: NewScreenOverlay): Promise<ScreenOverlay> {
    await this.init();
    const overlay = createScreenOverlay(input);
    await this.driver.execute(
      "INSERT INTO screen_overlays (id, folder_id, data) VALUES (?, ?, ?)",
      [overlay.id, overlay.folderId, JSON.stringify(overlay)],
    );
    return overlay;
  }

  async deleteScreenOverlay(id: string): Promise<void> {
    await this.init();
    await this.driver.execute("DELETE FROM screen_overlays WHERE id = ?", [id]);
  }

  async importBatch(payload: ImportBatchPayload): Promise<void> {
    await this.init();
    for (const folder of payload.folders) {
      await this.driver.execute(
        "INSERT OR REPLACE INTO folders (id, parent_id, data) VALUES (?, ?, ?)",
        [folder.id, folder.parentId, JSON.stringify(folder)],
      );
    }
    for (const placemark of payload.placemarks) {
      await this.driver.execute(
        "INSERT OR REPLACE INTO placemarks (id, folder_id, data) VALUES (?, ?, ?)",
        [placemark.id, placemark.folderId, JSON.stringify(placemark)],
      );
    }
    for (const style of payload.styles) {
      await this.driver.execute("INSERT OR REPLACE INTO styles (id, data) VALUES (?, ?)", [
        style.id,
        JSON.stringify(style),
      ]);
    }
    for (const overlay of payload.screenOverlays ?? []) {
      await this.driver.execute(
        "INSERT OR REPLACE INTO screen_overlays (id, folder_id, data) VALUES (?, ?, ?)",
        [overlay.id, overlay.folderId, JSON.stringify(overlay)],
      );
    }
  }
}
