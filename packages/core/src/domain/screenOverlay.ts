import { generateId } from "./ids.js";

/** Fraction (0-1) or pixel offset from an edge, per KML's vec2 (x/y + xunits/yunits). */
export interface ScreenOverlayAnchor {
  x: number;
  y: number;
  xUnits: "fraction" | "pixels" | "insetPixels";
  yUnits: "fraction" | "pixels" | "insetPixels";
}

/** Screen-space image pinned to the viewport (KML ScreenOverlay) - not geo-anchored, not a globe entity. */
export interface ScreenOverlay {
  id: string;
  folderId: string | null;
  name: string;
  imageUrl: string;
  overlayXY: ScreenOverlayAnchor;
  screenXY: ScreenOverlayAnchor;
  sizeX?: number;
  sizeY?: number;
  rotation?: number;
  visibility: boolean;
  order: number;
  createdAt: string;
  updatedAt: string;
}

export interface NewScreenOverlay {
  name: string;
  folderId: string | null;
  imageUrl: string;
  overlayXY: ScreenOverlayAnchor;
  screenXY: ScreenOverlayAnchor;
  sizeX?: number;
  sizeY?: number;
  rotation?: number;
  visibility?: boolean;
  order?: number;
}

export function createScreenOverlay(input: NewScreenOverlay): ScreenOverlay {
  if (input.name.trim().length === 0) {
    throw new Error("name must not be empty");
  }
  if (input.imageUrl.trim().length === 0) {
    throw new Error("imageUrl must not be empty");
  }
  const now = new Date().toISOString();
  return {
    id: generateId(),
    folderId: input.folderId,
    name: input.name,
    imageUrl: input.imageUrl,
    overlayXY: input.overlayXY,
    screenXY: input.screenXY,
    sizeX: input.sizeX,
    sizeY: input.sizeY,
    rotation: input.rotation,
    visibility: input.visibility ?? true,
    order: input.order ?? 0,
    createdAt: now,
    updatedAt: now,
  };
}
