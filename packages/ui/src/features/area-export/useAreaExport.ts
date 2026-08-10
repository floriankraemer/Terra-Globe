import { useEffect, useRef, useState } from "react";
import * as Cesium from "cesium";
import { useTranslation } from "react-i18next";
import type { RectangleBounds, RectangleGeometry, Style } from "@terra-globe/core";
import {
  AreaSelectController,
  CesiumEntityFactory,
  CesiumScreenPicker,
  captureAreaImage,
  computeExportPlan,
  ExportTooLargeError,
  type EntityHandle,
  type ExportPlan,
  type IEntityFactory,
} from "@terra-globe/map";
import { downloadBlob } from "../../lib/downloadBlob.js";

// `ContextLimits` is a real runtime export of the `cesium` package (re-exported
// from `@cesium/engine`) but is missing from the published `cesium` package's
// public .d.ts, so it has to be read off the module via an untyped cast.
const maxTextureSize = (): number =>
  (Cesium as unknown as { ContextLimits: { maximumTextureSize: number } }).ContextLimits
    .maximumTextureSize;

export interface UseAreaExportResult {
  active: boolean;
  bounds: RectangleBounds | null;
  scaleDenominator: number;
  dpi: number;
  exporting: boolean;
  error: string | null;
  plan: ExportPlan | null;
  planError: { pixelWidth: number; pixelHeight: number; maxDimensionPx: number } | null;
  start: () => void;
  cancel: () => void;
  redraw: () => void;
  setScale: (n: number) => void;
  setDpi: (n: number) => void;
  runExport: () => Promise<void>;
}

const RECTANGLE_HANDLE: EntityHandle = { entityId: "__area_export_rectangle__" };
const PREVIEW_HANDLE: EntityHandle = { entityId: "__area_export_preview__" };
const AREA_EXPORT_STYLE: Style = {
  id: "area-export-rectangle",
  outlineEnabled: true,
  outlineColor: "#2196f3",
  outlineWidth: 2,
  filled: true,
  fillColor: "#2196f3",
  fillOpacity: 0.15,
};

/** Wires AreaSelectController to a live Cesium viewer. Cesium glue - covered by E2E, not unit tests. */
export function useAreaExport(viewer: Cesium.Viewer | null): UseAreaExportResult {
  const { t } = useTranslation();
  const [active, setActive] = useState(false);
  const [bounds, setBounds] = useState<RectangleBounds | null>(null);
  const [scaleDenominator, setScaleDenominator] = useState(10000);
  const [dpi, setDpiState] = useState(150);
  const [exporting, setExporting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const controllerRef = useRef<AreaSelectController | null>(null);
  const entityFactoryRef = useRef<IEntityFactory | null>(null);
  const rectangleVisibleRef = useRef(false);
  const previewVisibleRef = useRef(false);
  const livePreviewRef = useRef<{ setGeometry: (geometry: RectangleGeometry) => void } | null>(
    null,
  );

  function clearPreview(): void {
    if (previewVisibleRef.current) {
      entityFactoryRef.current?.removeEntity(PREVIEW_HANDLE);
      previewVisibleRef.current = false;
      livePreviewRef.current = null;
    }
  }

  function clearRectangle(): void {
    if (rectangleVisibleRef.current) {
      entityFactoryRef.current?.removeEntity(RECTANGLE_HANDLE);
      rectangleVisibleRef.current = false;
    }
  }

  useEffect(() => {
    if (!viewer) return;

    const entityFactory = new CesiumEntityFactory(viewer);
    const controller = new AreaSelectController();
    const picker = new CesiumScreenPicker(viewer);
    controllerRef.current = controller;
    entityFactoryRef.current = entityFactory;

    const handler = new Cesium.ScreenSpaceEventHandler(viewer.scene.canvas);

    handler.setInputAction((event: Cesium.ScreenSpaceEventHandler.PositionedEvent) => {
      if (!controller.isActive) return;
      const point = picker.pickGround({ x: event.position.x, y: event.position.y });
      if (!point) return;
      const committed = controller.addPoint(point);
      if (committed) {
        clearPreview();
        entityFactory.createEntity(
          { type: "Rectangle", ...committed },
          RECTANGLE_HANDLE.entityId,
          AREA_EXPORT_STYLE,
        );
        rectangleVisibleRef.current = true;
        setBounds(committed);
      }
    }, Cesium.ScreenSpaceEventType.LEFT_CLICK);

    // Same rAF-throttled preview pattern as useRuler.ts.
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
            AREA_EXPORT_STYLE,
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
      clearPreview();
      clearRectangle();
      controllerRef.current = null;
      entityFactoryRef.current = null;
    };
  }, [viewer]);

  function start(): void {
    clearPreview();
    clearRectangle();
    controllerRef.current?.start();
    setActive(true);
    setBounds(null);
    setError(null);
  }

  function cancel(): void {
    controllerRef.current?.cancel();
    clearPreview();
    clearRectangle();
    setActive(false);
    setBounds(null);
    setError(null);
  }

  function redraw(): void {
    clearPreview();
    clearRectangle();
    controllerRef.current?.start();
    setBounds(null);
    setError(null);
  }

  function setScale(n: number): void {
    setScaleDenominator(n);
  }

  function setDpi(n: number): void {
    setDpiState(n);
  }

  async function runExport(): Promise<void> {
    if (!bounds || !viewer) return;
    setExporting(true);
    setError(null);
    try {
      const plan = computeExportPlan(bounds, scaleDenominator, dpi, maxTextureSize());
      const blob = await captureAreaImage(viewer, bounds, plan);
      downloadBlob(blob, `terra-globe-area-export-${Date.now()}.png`, "image/png");
    } catch (err) {
      if (err instanceof ExportTooLargeError) {
        setError(
          t("areaExport.tooLarge", {
            width: err.pixelWidth,
            height: err.pixelHeight,
            max: err.maxDimensionPx,
          }),
        );
      } else {
        setError(err instanceof Error ? err.message : String(err));
      }
    } finally {
      setExporting(false);
    }
  }

  let plan: ExportPlan | null = null;
  let planError: { pixelWidth: number; pixelHeight: number; maxDimensionPx: number } | null = null;
  if (bounds && viewer) {
    try {
      plan = computeExportPlan(bounds, scaleDenominator, dpi, maxTextureSize());
    } catch (err) {
      if (err instanceof ExportTooLargeError) {
        planError = {
          pixelWidth: err.pixelWidth,
          pixelHeight: err.pixelHeight,
          maxDimensionPx: err.maxDimensionPx,
        };
      }
    }
  }

  return {
    active,
    bounds,
    scaleDenominator,
    dpi,
    exporting,
    error,
    plan,
    planError,
    start,
    cancel,
    redraw,
    setScale,
    setDpi,
    runExport,
  };
}
