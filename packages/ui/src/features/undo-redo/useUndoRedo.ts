import { useRef, useState } from "react";
import type { LibrarySnapshot } from "../folders/useLibrary.js";
import { MAX_HISTORY } from "./historyConfig.js";

export interface UndoableLibrary {
  exportAll: () => Promise<LibrarySnapshot>;
  restoreSnapshot: (snapshot: LibrarySnapshot) => Promise<void>;
}

export interface UseUndoRedoResult {
  canUndo: boolean;
  canRedo: boolean;
  undo: () => Promise<void>;
  redo: () => Promise<void>;
  /** Wraps a library mutation so it becomes undoable: snapshots state before running it. */
  wrap: <T>(action: () => Promise<T>) => Promise<T>;
}

function pushCapped(stack: LibrarySnapshot[], entry: LibrarySnapshot): void {
  stack.push(entry);
  if (stack.length > MAX_HISTORY) stack.shift();
}

export function useUndoRedo(library: UndoableLibrary): UseUndoRedoResult {
  const undoStack = useRef<LibrarySnapshot[]>([]);
  const redoStack = useRef<LibrarySnapshot[]>([]);
  const [canUndo, setCanUndo] = useState(false);
  const [canRedo, setCanRedo] = useState(false);
  // Set while undo()/redo() itself is restoring a snapshot, so `wrap` calls
  // made incidentally during that restore (there are none today, but future
  // mutators routed through `wrap` should stay safe) don't get recorded as a
  // new undoable action.
  const restoringRef = useRef(false);

  async function wrap<T>(action: () => Promise<T>): Promise<T> {
    if (restoringRef.current) return action();
    const before = await library.exportAll();
    const result = await action();
    pushCapped(undoStack.current, before);
    redoStack.current = [];
    setCanUndo(true);
    setCanRedo(false);
    return result;
  }

  async function undo(): Promise<void> {
    const before = undoStack.current.pop();
    if (!before) return;
    restoringRef.current = true;
    const current = await library.exportAll();
    await library.restoreSnapshot(before);
    restoringRef.current = false;
    pushCapped(redoStack.current, current);
    setCanUndo(undoStack.current.length > 0);
    setCanRedo(true);
  }

  async function redo(): Promise<void> {
    const after = redoStack.current.pop();
    if (!after) return;
    restoringRef.current = true;
    const current = await library.exportAll();
    await library.restoreSnapshot(after);
    restoringRef.current = false;
    pushCapped(undoStack.current, current);
    setCanRedo(redoStack.current.length > 0);
    setCanUndo(true);
  }

  return { canUndo, canRedo, undo, redo, wrap };
}
