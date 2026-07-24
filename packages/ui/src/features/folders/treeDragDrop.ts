import type { Folder, Placemark } from "@terra-globe/core";

export type TreeItemKind = "folder" | "placemark";
export type DropPosition = "before" | "after" | "inside";

export interface DragItem {
  kind: TreeItemKind;
  id: string;
}

export interface MoveResult {
  kind: TreeItemKind;
  id: string;
  parentId: string | null;
  index: number;
}

/** Resolves a pointer's vertical position within a row (0 = top, 1 = bottom) to a drop zone. */
export function resolveDropPosition(ratio: number, allowInside: boolean): DropPosition {
  if (allowInside && ratio >= 0.25 && ratio <= 0.75) return "inside";
  return ratio < 0.5 ? "before" : "after";
}

function siblingIndexExcluding(
  kind: TreeItemKind,
  parentId: string | null,
  excludeId: string,
  beforeId: string | null,
  folders: Folder[],
  placemarks: Placemark[],
): number {
  const siblings =
    kind === "folder"
      ? folders
          .filter((f) => f.parentId === parentId && f.id !== excludeId)
          .sort((a, b) => a.order - b.order)
      : placemarks
          .filter((p) => p.folderId === parentId && p.id !== excludeId)
          .sort((a, b) => a.order - b.order);
  if (beforeId === null) return siblings.length;
  const idx = siblings.findIndex((s) => s.id === beforeId);
  return idx === -1 ? siblings.length : idx;
}

/**
 * Resolves a drag-and-drop onto a specific tree row into a move (or `null` if
 * the drop should be a no-op, e.g. dropping a folder onto itself).
 */
export function resolveDrop(
  item: DragItem,
  target: { kind: TreeItemKind; id: string; parentId: string | null },
  position: DropPosition,
  folders: Folder[],
  placemarks: Placemark[],
): MoveResult | null {
  if (item.kind === "folder" && item.id === target.id) return null;

  if (position === "inside") {
    const index = siblingIndexExcluding(item.kind, target.id, item.id, null, folders, placemarks);
    return { kind: item.kind, id: item.id, parentId: target.id, index };
  }

  if (item.kind === target.kind) {
    const targetIndex = siblingIndexExcluding(
      item.kind,
      target.parentId,
      item.id,
      target.id,
      folders,
      placemarks,
    );
    return {
      kind: item.kind,
      id: item.id,
      parentId: target.parentId,
      index: position === "before" ? targetIndex : targetIndex + 1,
    };
  }

  // Dropping before/after a row of the other kind: reparent, appended at the
  // end of this item's own kind-list (folders and placemarks each keep their
  // own order sequence, so there's no meaningful interleaved position here).
  const index = siblingIndexExcluding(
    item.kind,
    target.parentId,
    item.id,
    null,
    folders,
    placemarks,
  );
  return { kind: item.kind, id: item.id, parentId: target.parentId, index };
}

/** Resolves a drop onto a container's empty space (not over any specific row) as an append. */
export function resolveDropOnContainer(
  item: DragItem,
  parentId: string | null,
  folders: Folder[],
  placemarks: Placemark[],
): MoveResult {
  const index = siblingIndexExcluding(item.kind, parentId, item.id, null, folders, placemarks);
  return { kind: item.kind, id: item.id, parentId, index };
}
