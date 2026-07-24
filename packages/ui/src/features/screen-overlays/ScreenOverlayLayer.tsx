import type { CSSProperties } from "react";
import type { ScreenOverlay, ScreenOverlayAnchor } from "@webglobe/core";

interface ScreenOverlayLayerProps {
  overlays: ScreenOverlay[];
}

// KML's vec2 origin is the screen's bottom-left, y increasing upward - the
// opposite of CSS's top-left/y-down. `insetPixels` (offset from the far
// edge) is treated the same as `pixels` here; that's a known simplification,
// not exact per spec, but visually close for the common fraction/pixels case.
function anchorToCss(anchor: ScreenOverlayAnchor, axis: "x" | "y"): string {
  const value = axis === "x" ? anchor.x : anchor.y;
  const units = axis === "x" ? anchor.xUnits : anchor.yUnits;
  if (units === "fraction") {
    const percent = axis === "y" ? (1 - value) * 100 : value * 100;
    return `${percent}%`;
  }
  return axis === "y" ? `calc(100% - ${value}px)` : `${value}px`;
}

function overlayStyle(overlay: ScreenOverlay): CSSProperties {
  const originX = overlay.overlayXY.xUnits === "fraction" ? overlay.overlayXY.x * 100 : 0;
  const originY = overlay.overlayXY.yUnits === "fraction" ? (1 - overlay.overlayXY.y) * 100 : 0;
  return {
    position: "absolute",
    left: anchorToCss(overlay.screenXY, "x"),
    top: anchorToCss(overlay.screenXY, "y"),
    width: overlay.sizeX !== undefined && overlay.sizeX >= 0 ? `${overlay.sizeX}px` : undefined,
    height: overlay.sizeY !== undefined && overlay.sizeY >= 0 ? `${overlay.sizeY}px` : undefined,
    transform: `translate(-${originX}%, -${originY}%)${overlay.rotation ? ` rotate(${overlay.rotation}deg)` : ""}`,
    pointerEvents: "none",
  };
}

/** Renders KML ScreenOverlay images pinned to the viewport, outside the Cesium canvas. */
export function ScreenOverlayLayer({ overlays }: ScreenOverlayLayerProps) {
  return (
    <div style={{ position: "absolute", inset: 0, pointerEvents: "none" }}>
      {overlays
        .filter((o) => o.visibility)
        .map((overlay) => (
          <img
            key={overlay.id}
            src={overlay.imageUrl}
            alt={overlay.name}
            style={overlayStyle(overlay)}
          />
        ))}
    </div>
  );
}
