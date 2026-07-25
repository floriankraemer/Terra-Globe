import { describe, expect, it } from "vitest";
import { act, renderHook } from "@testing-library/react";
import type { LibrarySnapshot } from "../folders/useLibrary.js";
import { MAX_HISTORY } from "./historyConfig.js";
import { useUndoRedo, type UndoableLibrary } from "./useUndoRedo.js";

function snapshot(tag: string): LibrarySnapshot {
  return {
    folders: [],
    placemarks: [],
    styles: [],
    screenOverlays: [{ id: tag } as unknown as LibrarySnapshot["screenOverlays"][number]],
  };
}

function fakeLibrary(): { library: UndoableLibrary; current: { snap: LibrarySnapshot } } {
  const current = { snap: snapshot("0") };
  const library: UndoableLibrary = {
    exportAll: async () => current.snap,
    restoreSnapshot: async (s) => {
      current.snap = s;
    },
  };
  return { library, current };
}

describe("useUndoRedo", () => {
  it("starts with nothing to undo or redo", () => {
    const { library } = fakeLibrary();
    const { result } = renderHook(() => useUndoRedo(library));
    expect(result.current.canUndo).toBe(false);
    expect(result.current.canRedo).toBe(false);
  });

  it("wrap records the pre-action state and enables undo", async () => {
    const { library, current } = fakeLibrary();
    const { result } = renderHook(() => useUndoRedo(library));

    await act(async () => {
      await result.current.wrap(async () => {
        current.snap = snapshot("1");
      });
    });

    expect(result.current.canUndo).toBe(true);
    expect(result.current.canRedo).toBe(false);
  });

  it("undo restores the prior snapshot and enables redo", async () => {
    const { library, current } = fakeLibrary();
    const { result } = renderHook(() => useUndoRedo(library));

    await act(async () => {
      await result.current.wrap(async () => {
        current.snap = snapshot("1");
      });
    });
    await act(async () => {
      await result.current.undo();
    });

    expect(current.snap.screenOverlays[0]!.id).toBe("0");
    expect(result.current.canUndo).toBe(false);
    expect(result.current.canRedo).toBe(true);
  });

  it("redo re-applies the undone snapshot", async () => {
    const { library, current } = fakeLibrary();
    const { result } = renderHook(() => useUndoRedo(library));

    await act(async () => {
      await result.current.wrap(async () => {
        current.snap = snapshot("1");
      });
    });
    await act(async () => {
      await result.current.undo();
    });
    await act(async () => {
      await result.current.redo();
    });

    expect(current.snap.screenOverlays[0]!.id).toBe("1");
    expect(result.current.canUndo).toBe(true);
    expect(result.current.canRedo).toBe(false);
  });

  it("a new wrapped action clears the redo stack", async () => {
    const { library, current } = fakeLibrary();
    const { result } = renderHook(() => useUndoRedo(library));

    await act(async () => {
      await result.current.wrap(async () => {
        current.snap = snapshot("1");
      });
    });
    await act(async () => {
      await result.current.undo();
    });
    await act(async () => {
      await result.current.wrap(async () => {
        current.snap = snapshot("2");
      });
    });

    expect(result.current.canRedo).toBe(false);
  });

  it("caps history at MAX_HISTORY, dropping the oldest entry", async () => {
    const { library, current } = fakeLibrary();
    const { result } = renderHook(() => useUndoRedo(library));

    for (let i = 1; i <= MAX_HISTORY + 5; i++) {
      const tag = String(i);
      await act(async () => {
        await result.current.wrap(async () => {
          current.snap = snapshot(tag);
        });
      });
    }

    for (let i = 0; i < MAX_HISTORY; i++) {
      await act(async () => {
        await result.current.undo();
      });
    }

    // The oldest MAX_HISTORY+5 - MAX_HISTORY = 5 entries should have been
    // dropped, so undo bottoms out at snapshot "5", not the very first ("0").
    expect(current.snap.screenOverlays[0]!.id).toBe("5");
    expect(result.current.canUndo).toBe(false);
  });
});
