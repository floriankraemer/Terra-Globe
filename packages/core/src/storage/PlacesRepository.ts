import type { Folder, NewFolder } from "../domain/folder.js";
import type { NewPlacemark, Placemark } from "../domain/placemark.js";
import type { NewScreenOverlay, ScreenOverlay } from "../domain/screenOverlay.js";
import type { Style } from "../domain/style.js";

export interface DeleteFolderOptions {
  recursive: boolean;
}

export interface ImportBatchPayload {
  folders: Folder[];
  placemarks: Placemark[];
  styles: Style[];
  screenOverlays?: ScreenOverlay[];
}

/**
 * Storage port (dependency-inversion boundary). No adapter-specific types
 * (SQLite, IndexedDB, ...) may leak through this interface.
 */
export interface PlacesRepository {
  getFolder(id: string): Promise<Folder | null>;
  listFolders(parentId: string | null): Promise<Folder[]>;
  createFolder(folder: NewFolder): Promise<Folder>;
  updateFolder(id: string, patch: Partial<Omit<Folder, "id">>): Promise<Folder>;
  deleteFolder(id: string, opts?: DeleteFolderOptions): Promise<void>;

  getPlacemark(id: string): Promise<Placemark | null>;
  listPlacemarks(folderId: string | null): Promise<Placemark[]>;
  createPlacemark(placemark: NewPlacemark): Promise<Placemark>;
  updatePlacemark(id: string, patch: Partial<Omit<Placemark, "id">>): Promise<Placemark>;
  deletePlacemark(id: string): Promise<void>;

  getStyle(id: string): Promise<Style | null>;
  upsertStyle(style: Style): Promise<Style>;

  listScreenOverlays(folderId: string | null): Promise<ScreenOverlay[]>;
  createScreenOverlay(overlay: NewScreenOverlay): Promise<ScreenOverlay>;
  deleteScreenOverlay(id: string): Promise<void>;

  importBatch(payload: ImportBatchPayload): Promise<void>;
}

export class NotFoundError extends Error {
  constructor(kind: "Folder" | "Placemark" | "Style", id: string) {
    super(`${kind} not found: ${id}`);
    this.name = "NotFoundError";
  }
}
