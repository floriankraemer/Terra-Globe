import { useEffect, useRef } from "react";
import * as Cesium from "cesium";
import type { Placemark, PlacemarkGeometry } from "@terra-globe/core";
import { translateGeometry } from "@terra-globe/core";
import { CesiumScreenPicker } from "@terra-globe/map";

/**
 * Lets the user click-and-hold a selected placemark's entity on the map and
 * drag it to a new position (whole-shape translation, not per-vertex
 * reshaping). Cesium glue - covered by E2E, not unit tests (same convention
 * as useDrawing.ts).
 */
export function useDragToMove(
  viewer: Cesium.Viewer | null,
  selected: Placemark | null,
  disabled: boolean,
  // Called once movement is first detected (not on every plain click) to
  // swap the entity to a cheap CallbackProperty-driven live shape - see
  // IEntityFactory.beginLiveGeometryEdit for why this must not happen on
  // every animation frame.
  beginDrag: (id: string) => ((geometry: PlacemarkGeometry) => void) | undefined,
  onCommit: (id: string, geometry: PlacemarkGeometry) => void,
): void {
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const disabledRef = useRef(disabled);
  disabledRef.current = disabled;
  const beginDragRef = useRef(beginDrag);
  beginDragRef.current = beginDrag;
  const onCommitRef = useRef(onCommit);
  onCommitRef.current = onCommit;

  useEffect(() => {
    if (!viewer) return;

    const picker = new CesiumScreenPicker(viewer);
    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);

    let draggingId: string | null = null;
    let originalGeometry: PlacemarkGeometry | null = null;
    let startPoint: { lon: number; lat: number } | null = null;
    let lastGeometry: PlacemarkGeometry | null = null;
    let setGeometry: ((geometry: PlacemarkGeometry) => void) | null = null;

    function endDrag(): void {
      if (draggingId === null) return;
      viewer!.scene.screenSpaceCameraController.enableInputs = true;
      draggingId = null;
      originalGeometry = null;
      startPoint = null;
      lastGeometry = null;
      setGeometry = null;
    }

    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      const current = selectedRef.current;
      if (disabledRef.current || !current) return;
      const picked = viewer.scene.pick(event.position);
      if (!(picked?.id instanceof Cesium.Entity) || picked.id.id !== current.id) return;
      const point = picker.pickGround({ x: event.position.x, y: event.position.y });
      if (!point) return;

      draggingId = current.id;
      originalGeometry = current.geometry;
      lastGeometry = current.geometry;
      startPoint = point;
      viewer.scene.screenSpaceCameraController.enableInputs = false;
    }, Cesium.ScreenSpaceEventType.LEFT_DOWN);

    let rafId: number | null = null;
    let pendingPosition: Cesium.Cartesian2 | null = null;

    function applyPendingDrag(): void {
      rafId = null;
      if (draggingId === null || !originalGeometry || !startPoint || !pendingPosition) return;
      const point = picker.pickGround({ x: pendingPosition.x, y: pendingPosition.y });
      if (!point) return;
      const geometry = translateGeometry(
        originalGeometry,
        point.lon - startPoint.lon,
        point.lat - startPoint.lat,
      );
      lastGeometry = geometry;
      // Deferred until the first real move so a plain click-select never
      // touches the entity's graphics at all.
      setGeometry ??= beginDragRef.current(draggingId) ?? null;
      setGeometry?.(geometry);
    }

    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.MotionEvent) => {
      if (draggingId === null) return;
      pendingPosition = event.endPosition;
      if (rafId === null) rafId = requestAnimationFrame(applyPendingDrag);
    }, Cesium.ScreenSpaceEventType.MOUSE_MOVE);

    handler.setInputAction(() => {
      if (
        draggingId !== null &&
        originalGeometry &&
        lastGeometry &&
        lastGeometry !== originalGeometry
      ) {
        onCommitRef.current(draggingId, lastGeometry);
      }
      endDrag();
    }, Cesium.ScreenSpaceEventType.LEFT_UP);

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      endDrag();
      handler.destroy();
    };
  }, [viewer]);
}
