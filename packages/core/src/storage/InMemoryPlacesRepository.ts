import { createFolder, type Folder, type NewFolder } from "../domain/folder.js";
import { createPlacemark, type NewPlacemark, type Placemark } from "../domain/placemark.js";
import {
  createScreenOverlay,
  type NewScreenOverlay,
  type ScreenOverlay,
} from "../domain/screenOverlay.js";
import type { Style } from "../domain/style.js";
import {
  NotFoundError,
  type DeleteFolderOptions,
  type ImportBatchPayload,
  type PlacesRepository,
} from "./PlacesRepository.js";

export class InMemoryPlacesRepository implements PlacesRepository {
  private folders = new Map<string, Folder>();
  private placemarks = new Map<string, Placemark>();
  private styles = new Map<string, Style>();
  private screenOverlays = new Map<string, ScreenOverlay>();

  async getFolder(id: string): Promise<Folder | null> {
    return this.folders.get(id) ?? null;
  }

  async listFolders(parentId: string | null): Promise<Folder[]> {
    return [...this.folders.values()]
      .filter((folder) => folder.parentId === parentId)
      .sort((a, b) => a.order - b.order);
  }

  async createFolder(input: NewFolder): Promise<Folder> {
    const folder = createFolder(input);
    this.folders.set(folder.id, folder);
    return folder;
  }

  async updateFolder(id: string, patch: Partial<Omit<Folder, "id">>): Promise<Folder> {
    const existing = this.folders.get(id);
    if (!existing) throw new NotFoundError("Folder", id);
    const updated: Folder = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    this.folders.set(id, updated);
    return updated;
  }

  async deleteFolder(id: string, opts?: DeleteFolderOptions): Promise<void> {
    if (!this.folders.has(id)) throw new NotFoundError("Folder", id);
    const childFolders = [...this.folders.values()].filter((f) => f.parentId === id);
    const childPlacemarks = [...this.placemarks.values()].filter((p) => p.folderId === id);

    if ((childFolders.length > 0 || childPlacemarks.length > 0) && !opts?.recursive) {
      throw new Error(`Folder ${id} is not empty; pass { recursive: true } to delete it`);
    }

    for (const child of childFolders) {
      await this.deleteFolder(child.id, { recursive: true });
    }
    for (const placemark of childPlacemarks) {
      this.placemarks.delete(placemark.id);
    }
    this.folders.delete(id);
  }

  async getPlacemark(id: string): Promise<Placemark | null> {
    return this.placemarks.get(id) ?? null;
  }

  async listPlacemarks(folderId: string | null): Promise<Placemark[]> {
    return [...this.placemarks.values()]
      .filter((p) => p.folderId === folderId)
      .sort((a, b) => a.order - b.order);
  }

  async createPlacemark(input: NewPlacemark): Promise<Placemark> {
    const placemark = createPlacemark(input);
    this.placemarks.set(placemark.id, placemark);
    return placemark;
  }

  async updatePlacemark(id: string, patch: Partial<Omit<Placemark, "id">>): Promise<Placemark> {
    const existing = this.placemarks.get(id);
    if (!existing) throw new NotFoundError("Placemark", id);
    const updated: Placemark = { ...existing, ...patch, updatedAt: new Date().toISOString() };
    this.placemarks.set(id, updated);
    return updated;
  }

  async deletePlacemark(id: string): Promise<void> {
    if (!this.placemarks.has(id)) throw new NotFoundError("Placemark", id);
    this.placemarks.delete(id);
  }

  async getStyle(id: string): Promise<Style | null> {
    return this.styles.get(id) ?? null;
  }

  async upsertStyle(style: Style): Promise<Style> {
    this.styles.set(style.id, style);
    return style;
  }

  async listScreenOverlays(folderId: string | null): Promise<ScreenOverlay[]> {
    return [...this.screenOverlays.values()]
      .filter((overlay) => overlay.folderId === folderId)
      .sort((a, b) => a.order - b.order);
  }

  async createScreenOverlay(input: NewScreenOverlay): Promise<ScreenOverlay> {
    const overlay = createScreenOverlay(input);
    this.screenOverlays.set(overlay.id, overlay);
    return overlay;
  }

  async deleteScreenOverlay(id: string): Promise<void> {
    this.screenOverlays.delete(id);
  }

  async importBatch(payload: ImportBatchPayload): Promise<void> {
    for (const folder of payload.folders) this.folders.set(folder.id, folder);
    for (const placemark of payload.placemarks) this.placemarks.set(placemark.id, placemark);
    for (const style of payload.styles) this.styles.set(style.id, style);
    for (const overlay of payload.screenOverlays ?? []) {
      this.screenOverlays.set(overlay.id, overlay);
    }
  }
}
