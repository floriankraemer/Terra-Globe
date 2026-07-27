import type { Folder } from "./folder.js";
import type { Placemark } from "./placemark.js";

export function isFolderVisible(folders: Folder[], folderId: string | null): boolean {
  let currentId = folderId;
  const byId = new Map(folders.map((f) => [f.id, f]));
  while (currentId !== null) {
    const folder = byId.get(currentId);
    if (!folder) break;
    if (!folder.visibility) return false;
    currentId = folder.parentId;
  }
  return true;
}

export function isPlacemarkVisible(folders: Folder[], placemark: Placemark): boolean {
  return placemark.visibility && isFolderVisible(folders, placemark.folderId);
}
