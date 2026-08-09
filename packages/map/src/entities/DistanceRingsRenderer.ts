import * as Cesium from "cesium";
import {
  circleToPolygonRing,
  formatDistance,
  type GeoPoint,
  type UnitSystem,
} from "@terra-globe/core";

const RING_WIDTH = 2;
const LABEL_FONT = "bold 12px sans-serif";
const RING_SEGMENTS = 64;
// circleToPolygonRing walks bearing 0..2π over RING_SEGMENTS points (see
// circleApproximation.ts), so these indices land exactly on N/E/S/W.
const LABEL_POINT_INDICES = [0, RING_SEGMENTS / 4, RING_SEGMENTS / 2, (3 * RING_SEGMENTS) / 4];

const GREEN: [number, number, number] = [34, 197, 94];
const RED: [number, number, number] = [239, 68, 68];

/** Multiples of `spacingMeters` up to (and including, if exact) `discRadiusMeters`. Truncates any partial ring. */
export function ringRadii(spacingMeters: number, discRadiusMeters: number): number[] {
  if (spacingMeters <= 0) return [];
  const count = Math.floor(discRadiusMeters / spacingMeters);
  return Array.from({ length: count }, (_, i) => (i + 1) * spacingMeters);
}

/** Green (innermost) to red (outermost) gradient, keyed by ring index among `totalRings`. */
export function ringColor(ringIndex: number, totalRings: number): Cesium.Color {
  const t = totalRings <= 1 ? 1 : ringIndex / (totalRings - 1);
  const [r, g, b] = GREEN.map((start, i) => start + (RED[i]! - start) * t);
  return Cesium.Color.fromBytes(r, g, b);
}

/**
 * One ring's worth of Cesium entity options: a polyline outline plus distance
 * labels at its N/E/S/W points. Session-only overlay (see useDistanceRings)
 * - no fill, mirrors the outline-polyline pattern in
 * CesiumEntityFactory.outlineBoundaryLoops. `ringIndex`/`totalRings` drive
 * the green-to-red gradient color (innermost ring green, outermost red).
 */
export function buildRingEntities(
  center: GeoPoint,
  radiusMeters: number,
  unitSystem: UnitSystem,
  ringIndex: number,
  totalRings: number,
): Cesium.Entity.ConstructorOptions[] {
  const ring = circleToPolygonRing(center, radiusMeters, RING_SEGMENTS);
  const color = ringColor(ringIndex, totalRings);
  const labelText = formatDistance(radiusMeters, unitSystem);

  const polylineEntity: Cesium.Entity.ConstructorOptions = {
    polyline: {
      positions: ring.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
      width: RING_WIDTH,
      material: color,
    },
  };

  const labelEntities: Cesium.Entity.ConstructorOptions[] = LABEL_POINT_INDICES.map((i) => {
    const labelPoint = ring[i]!;
    return {
      position: Cesium.Cartesian3.fromDegrees(labelPoint.lon, labelPoint.lat),
      label: {
        text: labelText,
        font: LABEL_FONT,
        fillColor: Cesium.Color.WHITE,
        outlineColor: Cesium.Color.BLACK,
        outlineWidth: 2,
        style: Cesium.LabelStyle.FILL_AND_OUTLINE,
        verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
        pixelOffset: new Cesium.Cartesian2(0, -4),
      },
    };
  });

  return [polylineEntity, ...labelEntities];
}
