import { useEffect, useRef } from "react";
import * as Cesium from "cesium";
import type { GeoPoint, UnitSystem } from "@terra-globe/core";
import { buildRingEntities, ringRadii } from "@terra-globe/map";

export interface DistanceRingsSpec {
  spacingMeters: number;
  discRadiusMeters: number;
}

export interface UseDistanceRingsResult {
  set: (center: GeoPoint, rings: DistanceRingsSpec | null, unitSystem?: UnitSystem) => void;
}

/**
 * Session-only distance ring overlay for the PlacemarkEditor. No picking/
 * interaction, so unlike useRuler this only needs to add/remove entities -
 * covered by E2E, not unit tests (Cesium glue, same as useRuler).
 */
export function useDistanceRings(viewer: Cesium.Viewer | null): UseDistanceRingsResult {
  const viewerRef = useRef(viewer);
  viewerRef.current = viewer;
  const entityIdsRef = useRef<string[]>([]);

  function removeAll(v: Cesium.Viewer): void {
    entityIdsRef.current.forEach((id) => {
      const entity = v.entities.getById(id);
      if (entity) v.entities.remove(entity);
    });
    entityIdsRef.current = [];
  }

  useEffect(() => {
    return () => {
      const v = viewerRef.current;
      if (v) removeAll(v);
    };
  }, [viewer]);

  function set(
    center: GeoPoint,
    rings: DistanceRingsSpec | null,
    unitSystem: UnitSystem = "metric",
  ): void {
    const v = viewerRef.current;
    if (!v) return;

    removeAll(v);

    if (!rings) return;

    const radii = ringRadii(rings.spacingMeters, rings.discRadiusMeters);
    entityIdsRef.current = radii.flatMap((radiusMeters, index) =>
      buildRingEntities(center, radiusMeters, unitSystem, index, radii.length).map(
        (options) => v.entities.add(options).id,
      ),
    );
  }

  return { set };
}
