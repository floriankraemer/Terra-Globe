import { useEffect, useRef, useState } from "react";
import * as Cesium from "cesium";
import { computeTrackProfile, type LineStringGeometry, type Style } from "@webglobe/core";
import {
  CesiumEntityFactory,
  CesiumScreenPicker,
  RulerController,
  type EntityHandle,
  type IEntityFactory,
} from "@webglobe/map";

export interface RulerSegment {
  distanceMeters: number;
}

export interface UseRulerResult {
  active: boolean;
  vertexCount: number;
  segments: RulerSegment[];
  totalMeters: number;
  start: () => void;
  finish: () => void;
  cancel: () => void;
  undo: () => void;
}

const LINE_HANDLE: EntityHandle = { entityId: "__ruler_line__" };
const PREVIEW_HANDLE: EntityHandle = { entityId: "__ruler_preview__" };
const RULER_STYLE: Style = {
  id: "ruler-line",
  outlineEnabled: true,
  outlineColor: "#ffd700",
  outlineWidth: 3,
  filled: false,
  fillColor: "#ffd700",
  fillOpacity: 0,
};
const RULER_VERTEX_STYLE: Style = {
  ...RULER_STYLE,
  id: "ruler-vertex",
  filled: true,
  fillOpacity: 1,
};

function distanceStats(points: { lon: number; lat: number }[]): {
  segments: RulerSegment[];
  totalMeters: number;
} {
  if (points.length < 2) return { segments: [], totalMeters: 0 };
  const profile = computeTrackProfile({ type: "LineString", path: points });
  const segments = profile
    .slice(1)
    .map((p, i) => ({ distanceMeters: p.distanceMeters - profile[i]!.distanceMeters }));
  return { segments, totalMeters: profile.at(-1)!.distanceMeters };
}

/** Wires RulerController to a live Cesium viewer. Cesium glue - covered by E2E, not unit tests. */
export function useRuler(viewer: Cesium.Viewer | null): UseRulerResult {
  const [active, setActive] = useState(false);
  const [vertexCount, setVertexCount] = useState(0);
  const [segments, setSegments] = useState<RulerSegment[]>([]);
  const [totalMeters, setTotalMeters] = useState(0);
  const controllerRef = useRef<RulerController | null>(null);
  const entityFactoryRef = useRef<IEntityFactory | null>(null);
  const vertexHandlesRef = useRef<EntityHandle[]>([]);
  const previewVisibleRef = useRef(false);
  const lineVisibleRef = useRef(false);
  const livePreviewRef = useRef<{ setGeometry: (geometry: LineStringGeometry) => void } | null>(
    null,
  );

  function clearEntities(): void {
    const factory = entityFactoryRef.current;
    if (!factory) return;
    if (previewVisibleRef.current) {
      factory.removeEntity(PREVIEW_HANDLE);
      previewVisibleRef.current = false;
      livePreviewRef.current = null;
    }
    if (lineVisibleRef.current) {
      factory.removeEntity(LINE_HANDLE);
      lineVisibleRef.current = false;
    }
    vertexHandlesRef.current.forEach((handle) => factory.removeEntity(handle));
    vertexHandlesRef.current = [];
  }

  function refreshLine(): void {
    const controller = controllerRef.current;
    const factory = entityFactoryRef.current;
    if (!controller || !factory) return;
    const geometry = controller.currentGeometry();
    if (!geometry) {
      if (lineVisibleRef.current) {
        factory.removeEntity(LINE_HANDLE);
        lineVisibleRef.current = false;
      }
      return;
    }
    if (lineVisibleRef.current) {
      factory.updateEntity(LINE_HANDLE, geometry, RULER_STYLE);
    } else {
      factory.createEntity(geometry, LINE_HANDLE.entityId, RULER_STYLE);
      lineVisibleRef.current = true;
    }
  }

  function refreshStats(): void {
    const points = controllerRef.current?.getPoints() ?? [];
    const stats = distanceStats(points);
    setVertexCount(points.length);
    setSegments(stats.segments);
    setTotalMeters(stats.totalMeters);
  }

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
    const controller = new RulerController();
    const picker = new CesiumScreenPicker(viewer);
    controllerRef.current = controller;
    entityFactoryRef.current = entityFactory;

    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);

    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      if (!controller.isActive) return;
      const point = picker.pickGround({ x: event.position.x, y: event.position.y });
      if (!point) return;
      controller.addPoint(point);
      const handle = entityFactory.createEntity(
        { type: "Point", coordinates: point },
        undefined,
        RULER_VERTEX_STYLE,
      );
      vertexHandlesRef.current.push(handle);
      clearPreview();
      refreshLine();
      refreshStats();
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    // Raw canvas mousemove fires far more often than Cesium renders a frame,
    // and rebuilding a whole PolylineGraphics object per event is wasted
    // work. Coalesce to one update per animation frame, and once the preview
    // entity exists, stop touching the entity/graphics objects at all -
    // createLivePreview's CallbackProperty reads the path straight off a
    // plain variable, so Cesium re-tessellates on its own render tick
    // instead of us forcing it synchronously from the mousemove handler.
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
          const live = entityFactory.createLivePreview(
            geometry,
            PREVIEW_HANDLE.entityId,
            RULER_STYLE,
          );
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

    return () => {
      if (rafId !== null) cancelAnimationFrame(rafId);
      handler.destroy();
      clearEntities();
      controllerRef.current = null;
      entityFactoryRef.current = null;
    };
  }, [viewer]);

  function start(): void {
    clearEntities();
    controllerRef.current?.start();
    setActive(true);
    setVertexCount(0);
    setSegments([]);
    setTotalMeters(0);
  }

  function finish(): void {
    controllerRef.current?.finish();
    clearPreview();
    setActive(false);
  }

  function cancel(): void {
    controllerRef.current?.cancel();
    clearEntities();
    setActive(false);
    setVertexCount(0);
    setSegments([]);
    setTotalMeters(0);
  }

  function undo(): void {
    const controller = controllerRef.current;
    if (!controller) return;
    controller.undoLastVertex();
    const handle = vertexHandlesRef.current.pop();
    if (handle) entityFactoryRef.current?.removeEntity(handle);
    refreshLine();
    refreshStats();
  }

  return { active, vertexCount, segments, totalMeters, start, finish, cancel, undo };
}
