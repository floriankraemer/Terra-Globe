import { useEffect, useRef } from "react";
import type * as Cesium from "cesium";
import { createViewer, type TileSource, type CesiumViewerHandle } from "@terra-globe/map";

export interface CesiumViewerHostProps {
  baseLayer: TileSource;
  sceneMode: Cesium.SceneMode;
  onReady?: (handle: CesiumViewerHandle) => void;
}

export function CesiumViewerHost({ baseLayer, sceneMode, onReady }: CesiumViewerHostProps) {
  const containerRef = useRef<HTMLDivElement>(null);
  const handleRef = useRef<CesiumViewerHandle | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;
    const handle = createViewer(containerRef.current, baseLayer);
    handleRef.current = handle;
    onReady?.(handle);
    return () => {
      handle.destroy();
      handleRef.current = null;
    };
    // Viewer is created once per mount; layer switches are handled below.
  }, []);

  useEffect(() => {
    handleRef.current?.setBaseLayer(baseLayer);
  }, [baseLayer]);

  useEffect(() => {
    handleRef.current?.setSceneMode(sceneMode);
  }, [sceneMode]);

  return <div ref={containerRef} data-testid="cesium-viewer" className="cesium-viewer-host" />;
}
