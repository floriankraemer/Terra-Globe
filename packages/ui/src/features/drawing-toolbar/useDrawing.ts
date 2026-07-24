import { useEffect, useRef, useState } from "react";
import * as Cesium from "cesium";
import type { PlacemarkGeometry } from "@webglobe/core";
import {
  CesiumEntityFactory,
  CesiumScreenPicker,
  DrawingController,
  type EntityHandle,
  type IEntityFactory,
} from "@webglobe/map";
import type { DrawingMode, DrawingTool } from "./DrawingToolbar.js";

export interface UseDrawingResult {
  mode: DrawingMode;
  selectTool: (tool: DrawingTool) => void;
  finishPolygon: () => void;
  cancel: () => void;
}

const PREVIEW_HANDLE: EntityHandle = { entityId: "__drawing_preview__" };

/** Wires DrawingController to a live Cesium viewer. Cesium glue - covered by E2E, not unit tests. */
export function useDrawing(
  viewer: Cesium.Viewer | null,
  onShapeCommitted?: (geometry: PlacemarkGeometry) => void,
  onEntityClicked?: (entityId: string) => void,
): UseDrawingResult {
  const [mode, setMode] = useState<DrawingMode>("idle");
  const controllerRef = useRef<DrawingController | null>(null);
  const entityFactoryRef = useRef<IEntityFactory | null>(null);
  const handlerRef = useRef<Cesium.ScreenSpaceEventHandler | null>(null);
  const previewVisibleRef = useRef(false);
  const livePreviewRef = useRef<{ setGeometry: (geometry: PlacemarkGeometry) => void } | null>(
    null,
  );
  const onShapeCommittedRef = useRef(onShapeCommitted);
  onShapeCommittedRef.current = onShapeCommitted;
  const onEntityClickedRef = useRef(onEntityClicked);
  onEntityClickedRef.current = onEntityClicked;

  function clearPreview(): void {
    if (previewVisibleRef.current) {
      entityFactoryRef.current?.removeEntity(PREVIEW_HANDLE);
      previewVisibleRef.current = false;
      livePreviewRef.current = null;
    }
  }

  useEffect(() => {
    if (!viewer) return;

    const entityFactory = new CesiumEntityFactory(viewer.entities);
    const controller = new DrawingController(entityFactory);
    const picker = new CesiumScreenPicker(viewer);
    controllerRef.current = controller;
    entityFactoryRef.current = entityFactory;

    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);

    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      if (!controller.isActive) {
        const picked = viewer.scene.pick(event.position);
        if (picked?.id instanceof Cesium.Entity && picked.id.id !== PREVIEW_HANDLE.entityId) {
          onEntityClickedRef.current?.(picked.id.id);
        }
        return;
      }
      const point = picker.pickGround({ x: event.position.x, y: event.position.y });
      if (!point) return;
      const geometry = controller.addPoint(point);
      clearPreview();
      if (!controller.isActive) setMode("idle");
      if (geometry) onShapeCommittedRef.current?.(geometry);
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    // Raw canvas mousemove fires far more often than Cesium renders a frame,
    // and rebuilding a whole ellipse/rectangle graphics object per event is
    // expensive (re-tessellation + fresh listener wiring each time). Coalesce
    // to one update per animation frame, and once the preview entity exists,
    // stop touching the entity/graphics objects at all - createLivePreview's
    // CallbackProperty reads the shape straight off a plain variable, so
    // Cesium re-tessellates on its own render tick instead of us forcing it
    // synchronously from the mousemove handler.
    let rafId: number | null = null;
    let pendingPosition: Cesium.Cartesian2 | null = null;

    function applyPendingPreview(): void {
      rafId = null;
      if (!pendingPosition || !controller.isActive) return;
      const point = picker.pickGround({ x: pendingPosition.x, y: pendingPosition.y });
      const geometry = point ? controller.previewGeometry(point) : undefined;
      if (geometry) {
        if (livePreviewRef.current) {
          livePreviewRef.current.setGeometry(geometry);
        } else {
          const live = entityFactory.createLivePreview(geometry, PREVIEW_HANDLE.entityId);
          livePreviewRef.current = live;
          previewVisibleRef.current = true;
        }
      } else {
        clearPreview();
      }
    }

    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.MotionEvent) => {
      if (!controller.isActive) return;
      pendingPosition = event.endPosition;
      if (rafId === null) rafId = requestAnimationFrame(applyPendingPreview);
    }, Cesium.ScreenSpaceEventType.MOUSE_MOVE);

    handlerRef.current = handler;

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      handler.destroy();
      controllerRef.current = null;
      entityFactoryRef.current = null;
      handlerRef.current = null;
    };
  }, [viewer]);

  function selectTool(tool: DrawingTool): void {
    const controller = controllerRef.current;
    if (!controller) return;
    clearPreview();
    if (tool === "point") controller.startPoint();
    if (tool === "rectangle") controller.startRectangle();
    if (tool === "circle") controller.startCircle();
    if (tool === "polygon") controller.startPolygon();
    setMode(tool);
  }

  function finishPolygon(): void {
    const geometry = controllerRef.current?.finishPolygon();
    clearPreview();
    setMode("idle");
    if (geometry) onShapeCommittedRef.current?.(geometry);
  }

  function cancel(): void {
    controllerRef.current?.cancel();
    clearPreview();
    setMode("idle");
  }

  return { mode, selectTool, finishPolygon, cancel };
}
