import { useState } from "react";
import type { Folder, Placemark } from "@terra-globe/core";
import {
  resolveDrop,
  resolveDropOnContainer,
  resolveDropPosition,
  type DragItem,
  type DropPosition,
} from "./treeDragDrop.js";

export interface FolderTreeProps {
  disabled: boolean;
  folders: Folder[];
  placemarks: Placemark[];
  selectedFolderId: string | null;
  selectedPlacemarkId: string | null;
  onSelectFolder: (id: string | null) => void;
  onSelectPlacemark: (id: string) => void;
  onFlyToPlacemark: (id: string) => void;
  onCreateFolder: (parentId: string | null, name: string) => void;
  onRenameFolder: (id: string, name: string) => void;
  onDeleteFolder: (id: string) => void;
  onToggleFolderVisibility: (id: string) => void;
  onMoveFolder: (id: string, parentId: string | null, index: number) => void;
  onDeletePlacemark: (id: string) => void;
  onTogglePlacemarkVisibility: (id: string) => void;
  onMovePlacemark: (id: string, folderId: string | null, index: number) => void;
}

type DropTarget = { id: string; position: DropPosition };

interface DndContext {
  dragging: DragItem | null;
  dropTarget: DropTarget | null;
  startDrag: (item: DragItem) => void;
  endDrag: () => void;
  setDropTarget: (target: DropTarget | null) => void;
  drop: (targetKind: DragItem["kind"], targetId: string, targetParentId: string | null) => void;
  dropOnContainer: (parentId: string | null) => void;
}

function useDnd(props: FolderTreeProps): DndContext {
  const [dragging, setDragging] = useState<DragItem | null>(null);
  const [dropTarget, setDropTarget] = useState<DropTarget | null>(null);

  function move(kind: DragItem["kind"], id: string, parentId: string | null, index: number) {
    if (kind === "folder") props.onMoveFolder(id, parentId, index);
    else props.onMovePlacemark(id, parentId, index);
  }

  return {
    dragging,
    dropTarget,
    startDrag: setDragging,
    endDrag: () => {
      setDragging(null);
      setDropTarget(null);
    },
    setDropTarget,
    drop: (targetKind, targetId, targetParentId) => {
      const item = dragging;
      const position = dropTarget?.position;
      if (!item || !position) return;
      const result = resolveDrop(
        item,
        { kind: targetKind, id: targetId, parentId: targetParentId },
        position,
        props.folders,
        props.placemarks,
      );
      if (result) move(result.kind, result.id, result.parentId, result.index);
    },
    dropOnContainer: (parentId) => {
      const item = dragging;
      if (!item) return;
      const result = resolveDropOnContainer(item, parentId, props.folders, props.placemarks);
      move(result.kind, result.id, result.parentId, result.index);
    },
  };
}

function dropClassName(dnd: DndContext, id: string): string {
  if (dnd.dropTarget?.id !== id) return "";
  return ` drop-${dnd.dropTarget.position}`;
}

function handleRowDragOver(
  e: React.DragEvent,
  dnd: DndContext,
  id: string,
  allowInside: boolean,
): void {
  if (!dnd.dragging) return;
  e.preventDefault();
  e.stopPropagation();
  const rect = e.currentTarget.getBoundingClientRect();
  const ratio = (e.clientY - rect.top) / rect.height;
  dnd.setDropTarget({ id, position: resolveDropPosition(ratio, allowInside) });
}

function ChevronIcon({ expanded }: { expanded: boolean }) {
  return (
    <svg
      className={expanded ? "tree-chevron expanded" : "tree-chevron"}
      width="10"
      height="10"
      viewBox="0 0 10 10"
      aria-hidden="true"
    >
      <path d="M2 1 L7 5 L2 9" fill="none" stroke="currentColor" strokeWidth="1.5" />
    </svg>
  );
}

function FolderIcon() {
  return (
    <svg className="tree-folder-icon" width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
      <path
        d="M1.5 3.5A1 1 0 0 1 2.5 2.5h3.4a1 1 0 0 1 .8.4l.9 1.1h5.4a1 1 0 0 1 1 1v7a1 1 0 0 1-1 1h-11a1 1 0 0 1-1-1z"
        fill="currentColor"
      />
    </svg>
  );
}

function NewFolderForm({
  disabled,
  onCreate,
}: {
  disabled: boolean;
  onCreate: (name: string) => void;
}) {
  const [open, setOpen] = useState(false);
  const [name, setName] = useState("");

  if (!open) {
    return (
      <button
        type="button"
        className="btn new-folder-btn"
        disabled={disabled}
        onClick={() => setOpen(true)}
      >
        New Folder
      </button>
    );
  }

  return (
    <form
      className="new-folder-form"
      onSubmit={(e) => {
        e.preventDefault();
        if (name.trim().length === 0) return;
        onCreate(name);
        setName("");
        setOpen(false);
      }}
    >
      <input
        placeholder="Folder name"
        value={name}
        onChange={(e) => setName(e.target.value)}
        autoFocus
      />
      <button type="submit" className="btn">
        Create
      </button>
      <button type="button" className="btn" onClick={() => setOpen(false)}>
        Cancel
      </button>
    </form>
  );
}

function FolderRow({
  folder,
  isSelected,
  expanded,
  disabled,
  dnd,
  onToggleExpand,
  onSelectFolder,
  onRenameFolder,
  onDeleteFolder,
  onToggleFolderVisibility,
}: {
  folder: Folder;
  isSelected: boolean;
  expanded: boolean;
  disabled: boolean;
  dnd: DndContext;
  onToggleExpand: () => void;
  onSelectFolder: (id: string) => void;
  onRenameFolder: (id: string, name: string) => void;
  onDeleteFolder: (id: string) => void;
  onToggleFolderVisibility: (id: string) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [draftName, setDraftName] = useState(folder.name);

  function commitRename() {
    setEditing(false);
    if (draftName.trim().length > 0 && draftName !== folder.name) {
      onRenameFolder(folder.id, draftName);
    }
  }

  return (
    <span
      className={`tree-row${dropClassName(dnd, folder.id)}`}
      draggable={!disabled}
      onDragStart={(e) => {
        e.stopPropagation();
        dnd.startDrag({ kind: "folder", id: folder.id });
      }}
      onDragEnd={() => dnd.endDrag()}
      onDragOver={(e) => handleRowDragOver(e, dnd, folder.id, true)}
      onDragLeave={() => dnd.setDropTarget(null)}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        dnd.drop("folder", folder.id, folder.parentId);
        dnd.endDrag();
      }}
    >
      <button
        type="button"
        className="tree-expand-toggle"
        onClick={onToggleExpand}
        aria-label={expanded ? `Collapse ${folder.name}` : `Expand ${folder.name}`}
        aria-expanded={expanded}
      >
        <ChevronIcon expanded={expanded} />
      </button>
      <input
        type="checkbox"
        checked={folder.visibility}
        onChange={() => onToggleFolderVisibility(folder.id)}
        aria-label={`Toggle visibility of ${folder.name}`}
      />
      <FolderIcon />
      {editing ? (
        <input
          className="tree-rename-input"
          value={draftName}
          onChange={(e) => setDraftName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") commitRename();
            if (e.key === "Escape") {
              setDraftName(folder.name);
              setEditing(false);
            }
          }}
          onBlur={commitRename}
          autoFocus
        />
      ) : (
        <button
          type="button"
          className={isSelected ? "tree-row-name selected" : "tree-row-name"}
          onClick={() => onSelectFolder(folder.id)}
        >
          {folder.name}
        </button>
      )}
      <span className="tree-row-actions">
        <button type="button" className="btn" onClick={() => setEditing(true)}>
          Rename
        </button>
        <button type="button" className="btn btn-danger" onClick={() => onDeleteFolder(folder.id)}>
          Delete
        </button>
      </span>
    </span>
  );
}

function PlacemarkRow({
  placemark,
  isSelected,
  disabled,
  dnd,
  onSelectPlacemark,
  onFlyToPlacemark,
  onDeletePlacemark,
  onTogglePlacemarkVisibility,
}: {
  placemark: Placemark;
  isSelected: boolean;
  disabled: boolean;
  dnd: DndContext;
  onSelectPlacemark: (id: string) => void;
  onFlyToPlacemark: (id: string) => void;
  onDeletePlacemark: (id: string) => void;
  onTogglePlacemarkVisibility: (id: string) => void;
}) {
  return (
    <span
      className={`tree-row${dropClassName(dnd, placemark.id)}`}
      draggable={!disabled}
      onDragStart={(e) => {
        e.stopPropagation();
        dnd.startDrag({ kind: "placemark", id: placemark.id });
      }}
      onDragEnd={() => dnd.endDrag()}
      onDragOver={(e) => handleRowDragOver(e, dnd, placemark.id, false)}
      onDragLeave={() => dnd.setDropTarget(null)}
      onDrop={(e) => {
        e.preventDefault();
        e.stopPropagation();
        dnd.drop("placemark", placemark.id, placemark.folderId);
        dnd.endDrag();
      }}
    >
      <span className="tree-expand-toggle" aria-hidden="true" />
      <input
        type="checkbox"
        checked={placemark.visibility}
        onChange={() => onTogglePlacemarkVisibility(placemark.id)}
        aria-label={`Toggle visibility of ${placemark.name}`}
      />
      <button
        type="button"
        className={isSelected ? "tree-row-name selected" : "tree-row-name"}
        onClick={() => onSelectPlacemark(placemark.id)}
        onDoubleClick={() => onFlyToPlacemark(placemark.id)}
      >
        {placemark.name}
      </button>
      <span className="tree-row-actions">
        <button
          type="button"
          className="btn btn-danger"
          onClick={() => onDeletePlacemark(placemark.id)}
        >
          Delete
        </button>
      </span>
    </span>
  );
}

function FolderNode(props: FolderTreeProps & { folder: Folder; dnd: DndContext }) {
  const [expanded, setExpanded] = useState(true);
  return (
    <li>
      <FolderRow
        folder={props.folder}
        isSelected={props.selectedFolderId === props.folder.id}
        expanded={expanded}
        disabled={props.disabled}
        dnd={props.dnd}
        onToggleExpand={() => setExpanded((v) => !v)}
        onSelectFolder={props.onSelectFolder}
        onRenameFolder={props.onRenameFolder}
        onDeleteFolder={props.onDeleteFolder}
        onToggleFolderVisibility={props.onToggleFolderVisibility}
      />
      {expanded && <FolderChildren {...props} parentId={props.folder.id} />}
    </li>
  );
}

function FolderChildren(props: FolderTreeProps & { parentId: string | null; dnd: DndContext }) {
  const childFolders = props.folders
    .filter((f) => f.parentId === props.parentId)
    .sort((a, b) => a.order - b.order);
  const childPlacemarks = props.placemarks
    .filter((p) => p.folderId === props.parentId)
    .sort((a, b) => a.order - b.order);

  return (
    <ul
      onDragOver={(e) => {
        if (!props.dnd.dragging) return;
        e.preventDefault();
      }}
      onDrop={(e) => {
        e.preventDefault();
        props.dnd.dropOnContainer(props.parentId);
        props.dnd.endDrag();
      }}
    >
      {childFolders.map((folder) => (
        <FolderNode key={folder.id} {...props} folder={folder} />
      ))}
      {childPlacemarks.map((placemark) => (
        <li key={placemark.id}>
          <PlacemarkRow
            placemark={placemark}
            isSelected={props.selectedPlacemarkId === placemark.id}
            disabled={props.disabled}
            dnd={props.dnd}
            onSelectPlacemark={props.onSelectPlacemark}
            onFlyToPlacemark={props.onFlyToPlacemark}
            onDeletePlacemark={props.onDeletePlacemark}
            onTogglePlacemarkVisibility={props.onTogglePlacemarkVisibility}
          />
        </li>
      ))}
      <li>
        <NewFolderForm
          disabled={props.disabled}
          onCreate={(name) => props.onCreateFolder(props.parentId, name)}
        />
      </li>
    </ul>
  );
}

export function FolderTree(props: FolderTreeProps) {
  const dnd = useDnd(props);
  return (
    <div className="tree">
      <FolderChildren {...props} parentId={null} dnd={dnd} />
    </div>
  );
}
