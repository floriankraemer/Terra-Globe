import { DEFAULT_MARKER_ICON, type MarkerIconId } from "@terra-globe/core";

// Icon geometry (24x24 viewBox path/circle data) taken from Lucide
// (https://github.com/lucide-icons/lucide), ISC License.
interface IconShape {
  path?: string;
  circle?: { cx: number; cy: number; r: number };
}

const MARKER_ICON_SHAPES: Record<MarkerIconId, IconShape[]> = {
  "map-pin": [
    {
      path: "M20 10c0 4.993-5.539 10.193-7.399 11.799a1 1 0 0 1-1.202 0C9.539 20.193 4 14.993 4 10a8 8 0 0 1 16 0",
    },
    { circle: { cx: 12, cy: 10, r: 3 } },
  ],
  pin: [
    {
      path: "M9 10.76a2 2 0 0 1-1.11 1.79l-1.78.9A2 2 0 0 0 5 15.24V16a1 1 0 0 0 1 1h12a1 1 0 0 0 1-1v-.76a2 2 0 0 0-1.11-1.79l-1.78-.9A2 2 0 0 1 15 10.76V7a1 1 0 0 1 1-1 2 2 0 0 0 0-4H8a2 2 0 0 0 0 4 1 1 0 0 1 1 1z",
    },
  ],
  flag: [
    {
      path: "M4 22V4a1 1 0 0 1 .4-.8A6 6 0 0 1 8 2c3 0 5 2 7.333 2q2 0 3.067-.8A1 1 0 0 1 20 4v10a1 1 0 0 1-.4.8A6 6 0 0 1 16 16c-3 0-5-2-8-2a6 6 0 0 0-4 1.528V22",
    },
  ],
  star: [
    {
      path: "M11.525 2.295a.53.53 0 0 1 .95 0l2.31 4.679a2.123 2.123 0 0 0 1.595 1.16l5.166.756a.53.53 0 0 1 .294.904l-3.736 3.638a2.123 2.123 0 0 0-.611 1.878l.882 5.14a.53.53 0 0 1-.771.56l-4.618-2.428a2.122 2.122 0 0 0-1.973 0L6.396 21.01a.53.53 0 0 1-.77-.56l.881-5.139a2.122 2.122 0 0 0-.611-1.879L2.16 9.795a.53.53 0 0 1 .294-.906l5.165-.755a2.122 2.122 0 0 0 1.597-1.16z",
    },
  ],
  landmark: [
    { path: "M10 18v-7" },
    {
      path: "M11.119 2.205a2 2 0 0 1 1.762 0l7.84 3.846A.5.5 0 0 1 20.5 7h-17a.5.5 0 0 1-.22-.949z",
    },
    { path: "M14 18v-7" },
    { path: "M18 18v-7" },
    { path: "M3 22h18" },
    { path: "M6 18v-7" },
  ],
  navigation: [{ path: "M3 11 L22 2 L13 21 L11 13 Z" }],
};

const CANVAS_PIXEL_SIZE = 48;
const ICON_SCALE = CANVAS_PIXEL_SIZE / 24;

const canvasCache = new Map<string, HTMLCanvasElement>();

/** Rasterizes a built-in marker icon into a cached, colored canvas usable as a Cesium billboard image. */
export function markerIconCanvas(
  iconId: MarkerIconId | undefined,
  fillColor: string,
  outlineColor: string,
): HTMLCanvasElement {
  const id = iconId ?? DEFAULT_MARKER_ICON;
  const key = `${id}|${fillColor}|${outlineColor}`;
  const cached = canvasCache.get(key);
  if (cached) return cached;

  const canvas = document.createElement("canvas");
  canvas.width = CANVAS_PIXEL_SIZE;
  canvas.height = CANVAS_PIXEL_SIZE;
  const ctx = canvas.getContext("2d");
  if (!ctx) return canvas;

  ctx.scale(ICON_SCALE, ICON_SCALE);
  ctx.lineJoin = "round";
  ctx.lineCap = "round";
  ctx.lineWidth = 1;

  for (const shape of MARKER_ICON_SHAPES[id]) {
    if (shape.path) {
      const path = new Path2D(shape.path);
      ctx.fillStyle = fillColor;
      ctx.fill(path);
      ctx.strokeStyle = outlineColor;
      ctx.stroke(path);
    } else if (shape.circle) {
      // Punches a contrasting dot in the pin body (e.g. map-pin's center hole).
      ctx.beginPath();
      ctx.arc(shape.circle.cx, shape.circle.cy, shape.circle.r, 0, Math.PI * 2);
      ctx.fillStyle = outlineColor;
      ctx.fill();
    }
  }

  canvasCache.set(key, canvas);
  return canvas;
}
