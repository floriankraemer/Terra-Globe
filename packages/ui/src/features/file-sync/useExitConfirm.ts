import { useEffect, useRef, useState } from "react";
import { isTauri } from "../../platform/isTauri.js";

export interface UseExitConfirmResult {
  /** Whether the "save before closing?" dialog should be shown right now. */
  confirmOpen: boolean;
  onSave: () => Promise<void>;
  onDiscard: () => void;
  onCancel: () => void;
}

/**
 * Intercepts the Tauri window close request when there are unsaved changes
 * in manual-save mode, and shows a save/discard/cancel choice instead of
 * closing immediately. No-op in the browser build.
 */
export function useExitConfirm(
  dirty: boolean,
  autoSave: boolean,
  saveNow: () => Promise<void>,
): UseExitConfirmResult {
  const [confirmOpen, setConfirmOpen] = useState(false);
  const allowCloseRef = useRef(false);
  const dirtyRef = useRef(dirty);
  dirtyRef.current = dirty;
  const autoSaveRef = useRef(autoSave);
  autoSaveRef.current = autoSave;

  useEffect(() => {
    if (!isTauri()) return;
    let cancelled = false;
    let unlisten: (() => void) | undefined;
    import("@tauri-apps/api/window").then(({ getCurrentWindow }) => {
      if (cancelled) return;
      void getCurrentWindow()
        .onCloseRequested((event) => {
          if (allowCloseRef.current || autoSaveRef.current || !dirtyRef.current) return;
          event.preventDefault();
          setConfirmOpen(true);
        })
        .then((fn) => {
          if (cancelled) fn();
          else unlisten = fn;
        });
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  async function closeWindow(): Promise<void> {
    allowCloseRef.current = true;
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    await getCurrentWindow().close();
  }

  return {
    confirmOpen,
    onSave: async () => {
      setConfirmOpen(false);
      await saveNow();
      await closeWindow();
    },
    onDiscard: () => {
      setConfirmOpen(false);
      void closeWindow();
    },
    onCancel: () => setConfirmOpen(false),
  };
}
