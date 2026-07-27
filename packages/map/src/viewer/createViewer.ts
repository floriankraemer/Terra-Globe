import * as Cesium from "cesium";
import { openStreetMapSource, type TileSource } from "./imageryProviders/index.js";

export interface CesiumViewerHandle {
  viewer: Cesium.Viewer;
  setBaseLayer(source: TileSource): void;
  setSceneMode(mode: Cesium.SceneMode): void;
  destroy(): void;
}

function toImageryProvider(source: TileSource): Cesium.UrlTemplateImageryProvider {
  return new Cesium.UrlTemplateImageryProvider({
    url: source.url,
    subdomains: source.subdomains,
    credit: source.credit,
    maximumLevel: source.maximumLevel,
  });
}

export function createViewer(
  container: HTMLElement | string,
  initialSource: TileSource = openStreetMapSource(),
): CesiumViewerHandle {
  const viewer = new Cesium.Viewer(container, {
    baseLayerPicker: false,
    geocoder: false,
    homeButton: false,
    sceneModePicker: false,
    navigationHelpButton: false,
    timeline: false,
    animation: false,
    baseLayer: false,
    infoBox: false,
    // Cesium's default indicator anchors to entity.position, which for
    // ground-clamped shapes (circle/rectangle/polygon) has no real altitude
    // and gets padded by the full terrain elevation range - it floats far
    // above the shape, worst at grazing camera angles. Nothing in the app
    // reads viewer.selectedEntity, so drop the indicator instead of chasing
    // a correct height for it.
    selectionIndicator: false,
  });

  let currentLayer: Cesium.ImageryLayer | undefined;

  function setBaseLayer(source: TileSource): void {
    if (currentLayer) {
      viewer.imageryLayers.remove(currentLayer, true);
    }
    currentLayer = viewer.imageryLayers.addImageryProvider(toImageryProvider(source));
  }

  setBaseLayer(initialSource);

  function setSceneMode(mode: Cesium.SceneMode): void {
    switch (mode) {
      case Cesium.SceneMode.SCENE2D:
        viewer.scene.morphTo2D(0);
        break;
      case Cesium.SceneMode.COLUMBUS_VIEW:
        viewer.scene.morphToColumbusView(0);
        break;
      default:
        viewer.scene.morphTo3D(0);
        break;
    }
  }

  return {
    viewer,
    setBaseLayer,
    setSceneMode,
    destroy: () => viewer.destroy(),
  };
}
