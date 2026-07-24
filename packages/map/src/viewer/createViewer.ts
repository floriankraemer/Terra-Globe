import * as Cesium from "cesium";
import { openStreetMapSource, type TileSource } from "./imageryProviders/index.js";

export interface CesiumViewerHandle {
  viewer: Cesium.Viewer;
  setBaseLayer(source: TileSource): void;
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
    homeButton: true,
    sceneModePicker: false,
    navigationHelpButton: false,
    timeline: false,
    animation: false,
    baseLayer: false,
    infoBox: false,
  });

  let currentLayer: Cesium.ImageryLayer | undefined;

  function setBaseLayer(source: TileSource): void {
    if (currentLayer) {
      viewer.imageryLayers.remove(currentLayer, true);
    }
    currentLayer = viewer.imageryLayers.addImageryProvider(toImageryProvider(source));
  }

  setBaseLayer(initialSource);

  return {
    viewer,
    setBaseLayer,
    destroy: () => viewer.destroy(),
  };
}
