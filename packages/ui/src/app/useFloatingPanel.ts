import { useCallback, useEffect, useRef, useState } from "react";

export interface FloatingPanelGeometry {
  x: number;
  y: number;
  width: number;
  height: number;
}

export interface UseFloatingPanelOptions {
  initial: FloatingPanelGeometry;
  minWidth: number;
  minHeight: number;
  maxWidth: number;
  maxHeight: number;
  storageKey?: string;
}

export interface UseFloatingPanelResult {
  geometry: FloatingPanelGeometry;
  isDragging: boolean;
  isResizing: boolean;
  startDrag: (e: React.MouseEvent) => void;
  startResize: (e: React.MouseEvent) => void;
}

function clampGeometry(
  geometry: FloatingPanelGeometry,
  bounds: Pick<UseFloatingPanelOptions, "minWidth" | "minHeight" | "maxWidth" | "maxHeight">,
): FloatingPanelGeometry {
  const width = Math.min(bounds.maxWidth, Math.max(bounds.minWidth, geometry.width));
  const height = Math.min(bounds.maxHeight, Math.max(bounds.minHeight, geometry.height));
  const maxX = Math.max(0, window.innerWidth - 40);
  const maxY = Math.max(0, window.innerHeight - 40);
  const x = Math.min(maxX, Math.max(0, geometry.x));
  const y = Math.min(maxY, Math.max(0, geometry.y));
  return { x, y, width, height };
}

function readStoredGeometry(
  storageKey: string | undefined,
  fallback: FloatingPanelGeometry,
): FloatingPanelGeometry {
  if (!storageKey) return fallback;
  try {
    const raw = window.localStorage.getItem(storageKey);
    if (raw === null) return fallback;
    const parsed: unknown = JSON.parse(raw);
    if (
      typeof parsed !== "object" ||
      parsed === null ||
      !("x" in parsed) ||
      !("y" in parsed) ||
      !("width" in parsed) ||
      !("height" in parsed)
    ) {
      return fallback;
    }
    const { x, y, width, height } = parsed as FloatingPanelGeometry;
    if (![x, y, width, height].every(Number.isFinite)) return fallback;
    return { x, y, width, height };
  } catch {
    return fallback;
  }
}

/** Drag-to-move and drag-to-resize a floating panel, clamped to the viewport and size bounds, optionally persisted to localStorage. */
export function useFloatingPanel(options: UseFloatingPanelOptions): UseFloatingPanelResult {
  const { initial, minWidth, minHeight, maxWidth, maxHeight, storageKey } = options;
  const [geometry, setGeometry] = useState(() =>
    clampGeometry(readStoredGeometry(storageKey, initial), {
      minWidth,
      minHeight,
      maxWidth,
      maxHeight,
    }),
  );
  const [isDragging, setIsDragging] = useState(false);
  const [isResizing, setIsResizing] = useState(false);
  const dragStartRef = useRef({ startX: 0, startY: 0, startPanelX: 0, startPanelY: 0 });
  const resizeStartRef = useRef({ startX: 0, startY: 0, startWidth: 0, startHeight: 0 });

  const startDrag = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      dragStartRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        startPanelX: geometry.x,
        startPanelY: geometry.y,
      };
      setIsDragging(true);
    },
    [geometry.x, geometry.y],
  );

  const startResize = useCallback(
    (e: React.MouseEvent) => {
      e.preventDefault();
      resizeStartRef.current = {
        startX: e.clientX,
        startY: e.clientY,
        startWidth: geometry.width,
        startHeight: geometry.height,
      };
      setIsResizing(true);
    },
    [geometry.width, geometry.height],
  );

  useEffect(() => {
    if (!isDragging) return;

    function onMouseMove(e: MouseEvent): void {
      const { startX, startY, startPanelX, startPanelY } = dragStartRef.current;
      setGeometry((prev) =>
        clampGeometry(
          {
            ...prev,
            x: startPanelX + (e.clientX - startX),
            y: startPanelY + (e.clientY - startY),
          },
          { minWidth, minHeight, maxWidth, maxHeight },
        ),
      );
    }
    function onMouseUp(): void {
      setIsDragging(false);
    }

    window.addEventListener("mousemove", onMouseMove);
    window.addEventListener("mouseup", onMouseUp);
    return () => {
      window.removeEventListener("mousemove", onMouseMove);
      window.removeEventListener("mouseup", onMouseUp);
    };
  }, [isDragging, minWidth, minHeight, maxWidth, maxHeight]);

  useEffect(() => {
    if (!isResizing) return;

    function onMouseMove(e: MouseEvent): void {
      const { startX, startY, startWidth, startHeight } = resizeStartRef.current;
      setGeometry((prev) =>
        clampGeometry(
          {
            ...prev,
            width: startWidth + (e.clientX - startX),
            height: startHeight + (e.clientY - startY),
          },
          { minWidth, minHeight, maxWidth, maxHeight },
        ),
      );
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
  }, [isResizing, minWidth, minHeight, maxWidth, maxHeight]);

  useEffect(() => {
    if (!storageKey || isDragging || isResizing) return;
    try {
      window.localStorage.setItem(storageKey, JSON.stringify(geometry));
    } catch {
      // localStorage unavailable (e.g. private browsing) - geometry just won't persist.
    }
  }, [geometry, isDragging, isResizing, storageKey]);

  return { geometry, isDragging, isResizing, startDrag, startResize };
}
