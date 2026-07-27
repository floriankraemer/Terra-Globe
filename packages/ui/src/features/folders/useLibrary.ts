import { useEffect, useRef, useState } from "react";
import * as Cesium from "cesium";
import type {
  Folder,
  ImportBatchPayload,
  PlacemarkGeometry,
  Placemark,
  PlacesRepository,
  ScreenOverlay,
  Style,
} from "@terra-globe/core";
import { isPlacemarkVisible } from "@terra-globe/core";
import {
  CesiumEntityFactory,
  EntitySynchronizer,
  type PlacemarkStyleEdits,
} from "@terra-globe/map";
import { createRepository } from "../../platform/createRepository.js";

export interface UseLibraryResult {
  ready: boolean;
  folders: Folder[];
  placemarks: Placemark[];
  screenOverlays: ScreenOverlay[];
  selectedFolderId: string | null;
  selectFolder: (id: string | null) => void;
  createFolder: (parentId: string | null, name: string) => Promise<void>;
  renameFolder: (id: string, name: string) => Promise<void>;
  deleteFolder: (id: string) => Promise<void>;
  toggleFolderVisibility: (id: string) => Promise<void>;
  moveFolder: (id: string, parentId: string | null, index: number) => Promise<void>;
  deletePlacemark: (id: string) => Promise<void>;
  togglePlacemarkVisibility: (id: string) => Promise<void>;
  movePlacemark: (id: string, folderId: string | null, index: number) => Promise<void>;
  addPlacemark: (geometry: PlacemarkGeometry) => Promise<string | undefined>;
  savePlacemarkEdits: (
    id: string,
    edits: {
      name: string;
      description: string;
      style: PlacemarkStyleEdits;
      visibility: boolean;
      geometry: PlacemarkGeometry;
    },
  ) => Promise<void>;
  previewPlacemarkEdits: (
    id: string,
    edits: {
      name: string;
      style: PlacemarkStyleEdits;
      visibility: boolean;
      geometry: PlacemarkGeometry;
    },
  ) => void;
  getPlacemarkStyle: (id: string) => Promise<PlacemarkStyleEdits>;
  importPlaces: (payload: ImportBatchPayload) => Promise<void>;
  updatePlacemarkGeometry: (id: string, geometry: PlacemarkGeometry) => Promise<void>;
  beginPlacemarkDrag: (
    id: string,
    style: PlacemarkStyleEdits,
  ) => ((geometry: PlacemarkGeometry) => void) | undefined;
  exportAll: () => Promise<LibrarySnapshot>;
  restoreSnapshot: (snapshot: LibrarySnapshot) => Promise<void>;
}

export interface LibrarySnapshot {
  folders: Folder[];
  placemarks: Placemark[];
  styles: Style[];
  screenOverlays: ScreenOverlay[];
}

const DEFAULT_STYLE: PlacemarkStyleEdits = {
  outlineEnabled: true,
  outlineColor: "#ff0000",
  outlineWidth: 2,
  filled: false,
  fillColor: "#ff0000",
};

async function collectAllFolders(
  repo: PlacesRepository,
  parentId: string | null = null,
): Promise<Folder[]> {
  const children = await repo.listFolders(parentId);
  const nested = await Promise.all(children.map((f) => collectAllFolders(repo, f.id)));
  return [...children, ...nested.flat()];
}

/** True if `ancestorId` is `folderId` itself or one of its ancestors (i.e. moving `folderId` there would orphan it). */
function isFolderOrDescendant(
  folders: Folder[],
  folderId: string,
  candidateId: string | null,
): boolean {
  let current = candidateId;
  while (current !== null) {
    if (current === folderId) return true;
    current = folders.find((f) => f.id === current)?.parentId ?? null;
  }
  return false;
}

async function collectAllPlacemarks(
  repo: PlacesRepository,
  folders: Folder[],
): Promise<Placemark[]> {
  const folderIds: (string | null)[] = [null, ...folders.map((f) => f.id)];
  const groups = await Promise.all(folderIds.map((id) => repo.listPlacemarks(id)));
  return groups.flat();
}

async function collectAllScreenOverlays(
  repo: PlacesRepository,
  folders: Folder[],
): Promise<ScreenOverlay[]> {
  const folderIds: (string | null)[] = [null, ...folders.map((f) => f.id)];
  const groups = await Promise.all(folderIds.map((id) => repo.listScreenOverlays(id)));
  return groups.flat();
}

/** Owns the PlacesRepository, keeps folder/placemark UI state and live Cesium entities in sync. */
export function useLibrary(viewer: Cesium.Viewer | null): UseLibraryResult {
  const [ready, setReady] = useState(false);
  const [folders, setFolders] = useState<Folder[]>([]);
  const [placemarks, setPlacemarks] = useState<Placemark[]>([]);
  const [screenOverlays, setScreenOverlays] = useState<ScreenOverlay[]>([]);
  const [selectedFolderId, setSelectedFolderId] = useState<string | null>(null);
  const repoRef = useRef<PlacesRepository | null>(null);
  const syncRef = useRef<EntitySynchronizer | null>(null);
  // Concurrent actions (e.g. a drag-move right after a rename) can each kick
  // off their own refresh(); nothing guarantees they *resolve* in the order
  // they were called. Without this guard, a slower-but-older refresh can
  // resolve last and silently overwrite the UI with stale pre-move data even
  // though the write itself succeeded - only ever apply the most recently
  // started refresh's result.
  const refreshSeqRef = useRef(0);

  function applyVisibility(allFolders: Folder[], allPlacemarks: Placemark[]): void {
    const sync = syncRef.current;
    if (!sync) return;
    for (const placemark of allPlacemarks) {
      sync.setVisible(placemark.id, isPlacemarkVisible(allFolders, placemark));
    }
  }

  async function refresh(): Promise<void> {
    const repo = repoRef.current;
    if (!repo) return;
    const seq = ++refreshSeqRef.current;
    const allFolders = await collectAllFolders(repo);
    const allPlacemarks = await collectAllPlacemarks(repo, allFolders);
    const allScreenOverlays = await collectAllScreenOverlays(repo, allFolders);
    if (refreshSeqRef.current !== seq) return;
    setFolders(allFolders);
    setPlacemarks(allPlacemarks);
    setScreenOverlays(allScreenOverlays);
    applyVisibility(allFolders, allPlacemarks);
  }

  useEffect(() => {
    if (!viewer) return;
    let cancelled = false;

    createRepository().then(async (repo) => {
      if (cancelled) return;
      repoRef.current = repo;
      const synchronizer = new EntitySynchronizer(repo, new CesiumEntityFactory(viewer.entities));
      syncRef.current = synchronizer;
      await synchronizer.loadAll();
      await refresh();
      if (!cancelled) setReady(true);
    });

    return () => {
      cancelled = true;
    };
  }, [viewer]);

  return {
    ready,
    folders,
    placemarks,
    screenOverlays,
    selectedFolderId,
    selectFolder: setSelectedFolderId,
    async createFolder(parentId, name) {
      const repo = repoRef.current;
      if (!repo) return;
      await repo.createFolder({
        name,
        parentId,
        order: folders.filter((f) => f.parentId === parentId).length,
      });
      await refresh();
    },
    async renameFolder(id, name) {
      const repo = repoRef.current;
      if (!repo) return;
      await repo.updateFolder(id, { name });
      await refresh();
    },
    async deleteFolder(id) {
      const repo = repoRef.current;
      if (!repo) return;
      await repo.deleteFolder(id, { recursive: true });
      if (selectedFolderId === id) setSelectedFolderId(null);
      await refresh();
    },
    async toggleFolderVisibility(id) {
      const repo = repoRef.current;
      const folder = folders.find((f) => f.id === id);
      if (!repo || !folder) return;
      await repo.updateFolder(id, { visibility: !folder.visibility });
      await refresh();
    },
    async moveFolder(id, parentId, index) {
      const repo = repoRef.current;
      if (!repo) return;
      // Dropping a folder inside itself or one of its own descendants would
      // orphan the whole subtree - silently refuse rather than corrupt the tree.
      if (id === parentId || isFolderOrDescendant(folders, id, parentId)) return;

      const moved = folders.find((f) => f.id === id);
      if (!moved) return;
      const siblings = folders
        .filter((f) => f.parentId === parentId && f.id !== id)
        .sort((a, b) => a.order - b.order);
      const clampedIndex = Math.min(Math.max(index, 0), siblings.length);
      const reordered = [
        ...siblings.slice(0, clampedIndex),
        moved,
        ...siblings.slice(clampedIndex),
      ];

      await Promise.all(reordered.map((f, i) => repo.updateFolder(f.id, { parentId, order: i })));
      await refresh();
    },
    async deletePlacemark(id) {
      const sync = syncRef.current;
      if (!sync) return;
      await sync.deletePlacemark(id);
      await refresh();
    },
    async togglePlacemarkVisibility(id) {
      const repo = repoRef.current;
      const placemark = placemarks.find((p) => p.id === id);
      if (!repo || !placemark) return;
      await repo.updatePlacemark(id, { visibility: !placemark.visibility });
      await refresh();
    },
    async movePlacemark(id, folderId, index) {
      const repo = repoRef.current;
      if (!repo) return;
      const moved = placemarks.find((p) => p.id === id);
      if (!moved) return;
      const siblings = placemarks
        .filter((p) => p.folderId === folderId && p.id !== id)
        .sort((a, b) => a.order - b.order);
      const clampedIndex = Math.min(Math.max(index, 0), siblings.length);
      const reordered = [
        ...siblings.slice(0, clampedIndex),
        moved,
        ...siblings.slice(clampedIndex),
      ];

      await Promise.all(
        reordered.map((p, i) => repo.updatePlacemark(p.id, { folderId, order: i })),
      );
      await refresh();
    },
    async addPlacemark(geometry) {
      const sync = syncRef.current;
      if (!sync) return undefined;
      const placemark = await sync.persistPlacemark({
        name: `${geometry.type} ${placemarks.length + 1}`,
        folderId: selectedFolderId,
        geometry,
        order: placemarks.filter((p) => p.folderId === selectedFolderId).length,
      });
      await refresh();
      return placemark.id;
    },
    async savePlacemarkEdits(id, edits) {
      const sync = syncRef.current;
      if (!sync) return;
      await sync.savePlacemarkEdits(id, {
        name: edits.name,
        description: edits.description.trim().length > 0 ? edits.description : undefined,
        style: edits.style,
        visibility: edits.visibility,
        geometry: edits.geometry,
      });
      await refresh();
    },
    previewPlacemarkEdits(id, edits) {
      const sync = syncRef.current;
      const placemark = placemarks.find((p) => p.id === id);
      if (!sync || !placemark) return;
      sync.previewPlacemark(id, edits.geometry, edits.name, edits.style, edits.visibility);
    },
    async getPlacemarkStyle(id) {
      const repo = repoRef.current;
      const placemark = placemarks.find((p) => p.id === id);
      if (!repo || !placemark?.styleId) return DEFAULT_STYLE;
      const style = await repo.getStyle(placemark.styleId);
      if (!style) return DEFAULT_STYLE;
      return {
        outlineEnabled: style.outlineEnabled,
        outlineColor: style.outlineColor,
        outlineWidth: style.outlineWidth,
        filled: style.filled,
        fillColor: style.fillColor,
      };
    },
    async importPlaces(payload) {
      const repo = repoRef.current;
      const sync = syncRef.current;
      if (!repo || !sync) return;
      await repo.importBatch(payload);
      await sync.renderPlacemarks(payload.placemarks);
      await refresh();
    },
    async updatePlacemarkGeometry(id, geometry) {
      const sync = syncRef.current;
      if (!sync) return;
      await sync.updatePlacemark(id, { geometry });
      await refresh();
    },
    beginPlacemarkDrag(id, style) {
      const sync = syncRef.current;
      const placemark = placemarks.find((p) => p.id === id);
      if (!sync || !placemark) return undefined;
      return sync.beginLiveGeometryEdit(id, placemark.geometry, style);
    },
    async exportAll() {
      const repo = repoRef.current;
      if (!repo) return { folders: [], placemarks: [], styles: [], screenOverlays: [] };
      const allFolders = await collectAllFolders(repo);
      const allPlacemarks = await collectAllPlacemarks(repo, allFolders);
      const allScreenOverlays = await collectAllScreenOverlays(repo, allFolders);
      const styleIds = [
        ...new Set(allPlacemarks.map((p) => p.styleId).filter((id): id is string => id !== null)),
      ];
      const styles = (await Promise.all(styleIds.map((id) => repo.getStyle(id)))).filter(
        (s): s is Style => s !== null,
      );
      return {
        folders: allFolders,
        placemarks: allPlacemarks,
        styles,
        screenOverlays: allScreenOverlays,
      };
    },
    async restoreSnapshot(snapshot) {
      const repo = repoRef.current;
      const sync = syncRef.current;
      if (!repo || !sync) return;

      const currentFolders = await collectAllFolders(repo);
      const currentPlacemarks = await collectAllPlacemarks(repo, currentFolders);
      const currentScreenOverlays = await collectAllScreenOverlays(repo, currentFolders);

      const targetFolderIds = new Set(snapshot.folders.map((f) => f.id));
      const targetPlacemarkIds = new Set(snapshot.placemarks.map((p) => p.id));
      const targetOverlayIds = new Set(snapshot.screenOverlays.map((o) => o.id));

      for (const p of currentPlacemarks) {
        if (!targetPlacemarkIds.has(p.id)) await repo.deletePlacemark(p.id);
      }
      for (const o of currentScreenOverlays) {
        if (!targetOverlayIds.has(o.id)) await repo.deleteScreenOverlay(o.id);
      }
      for (const f of currentFolders) {
        if (targetFolderIds.has(f.id)) continue;
        // A prior recursive delete in this loop may already have removed this
        // folder as a descendant of an earlier one - snapshots are internally
        // consistent (a folder's ancestors are always included whenever the
        // folder is), so skipping already-gone folders is safe.
        if (await repo.getFolder(f.id)) await repo.deleteFolder(f.id, { recursive: true });
      }

      await repo.importBatch(snapshot);

      sync.removeEntities(currentPlacemarks.map((p) => p.id));
      await sync.renderPlacemarks(snapshot.placemarks);

      await refresh();
    },
  };
}
