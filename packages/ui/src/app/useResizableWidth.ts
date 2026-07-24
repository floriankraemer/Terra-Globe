import { useCallback, useEffect, useRef, useState } from "react";

export interface UseResizableWidthResult {
  width: number;
  isResizing: boolean;
  startResize: (e: React.MouseEvent) => void;
}

function readStoredWidth(
  storageKey: string | undefined,
  fallback: number,
  min: number,
  max: number,
): number {
  if (!storageKey) return fallback;
  try {
    const raw = window.localStorage.getItem(storageKey);
    if (raw === null) return fallback;
    const parsed = Number(raw);
    if (!Number.isFinite(parsed)) return fallback;
    return Math.min(max, Math.max(min, parsed));
  } catch {
    return fallback;
  }
}

/** Drag-to-resize a horizontal dimension, clamped to [min, max], optionally persisted to localStorage. */
export function useResizableWidth(
  initial: number,
  min: number,
  max: number,
  storageKey?: string,
): UseResizableWidthResult {
  const [width, setWidth] = useState(() => readStoredWidth(storageKey, initial, min, max));
  const [isResizing, setIsResizing] = useState(false);
  const startRef = useRef({ startX: 0, startWidth: initial });

  const startResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      startRef.current = { startX: e.clientX, startWidth: width };
      setIsResizing(true);
    },
    [width],
  );

  useEffect(() => {
    if (!isResizing) return;

    function onMouseMove(e: MouseEvent): void {
      const delta = e.clientX - startRef.current.startX;
      const next = Math.min(max, Math.max(min, startRef.current.startWidth + delta));
      setWidth(next);
    }
    function onMouseUp(): void {
      setIsResizing(false);
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isResizing, min, max]);

  useEffect(() => {
    if (!storageKey || isResizing) return;
    try {
      window.localStorage.setItem(storageKey, String(width));
    } catch {
      // localStorage unavailable (e.g. private browsing) - width just won't persist.
    }
  }, [width, isResizing, storageKey]);

  return { width, isResizing, startResize };
}
