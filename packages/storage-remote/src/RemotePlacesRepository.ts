import type {
  DeleteFolderOptions,
  Folder,
  ImportBatchPayload,
  NewFolder,
  NewPlacemark,
  NewScreenOverlay,
  Placemark,
  PlacesRepository,
  ScreenOverlay,
  Style,
} from "@terra-globe/core";

/**
 * Future home of S3/WebDAV/self-hosted sync. For now this is an inert
 * pass-through to a local PlacesRepository, so the storage seam (and its
 * contract tests) is exercised from day one, before real sync exists.
 */
export class RemotePlacesRepository implements PlacesRepository {
  constructor(private readonly local: PlacesRepository) {}

  getFolder(id: string): Promise<Folder | null> {
    return this.local.getFolder(id);
  }

  listFolders(parentId: string | null): Promise<Folder[]> {
    return this.local.listFolders(parentId);
  }

  createFolder(folder: NewFolder): Promise<Folder> {
    return this.local.createFolder(folder);
  }

  updateFolder(id: string, patch: Partial<Omit<Folder, "id">>): Promise<Folder> {
    return this.local.updateFolder(id, patch);
  }

  deleteFolder(id: string, opts?: DeleteFolderOptions): Promise<void> {
    return this.local.deleteFolder(id, opts);
  }

  getPlacemark(id: string): Promise<Placemark | null> {
    return this.local.getPlacemark(id);
  }

  listPlacemarks(folderId: string | null): Promise<Placemark[]> {
    return this.local.listPlacemarks(folderId);
  }

  createPlacemark(placemark: NewPlacemark): Promise<Placemark> {
    return this.local.createPlacemark(placemark);
  }

  updatePlacemark(id: string, patch: Partial<Omit<Placemark, "id">>): Promise<Placemark> {
    return this.local.updatePlacemark(id, patch);
  }

  deletePlacemark(id: string): Promise<void> {
    return this.local.deletePlacemark(id);
  }

  getStyle(id: string): Promise<Style | null> {
    return this.local.getStyle(id);
  }

  upsertStyle(style: Style): Promise<Style> {
    return this.local.upsertStyle(style);
  }

  listScreenOverlays(folderId: string | null): Promise<ScreenOverlay[]> {
    return this.local.listScreenOverlays(folderId);
  }

  createScreenOverlay(overlay: NewScreenOverlay): Promise<ScreenOverlay> {
    return this.local.createScreenOverlay(overlay);
  }

  deleteScreenOverlay(id: string): Promise<void> {
    return this.local.deleteScreenOverlay(id);
  }

  importBatch(payload: ImportBatchPayload): Promise<void> {
    return this.local.importBatch(payload);
  }
}
