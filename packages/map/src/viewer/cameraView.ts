import * as Cesium from "cesium";

/** A camera pose that can be serialized and later restored. */
export interface CameraView {
  longitude: number;
  latitude: number;
  height: number;
  heading: number;
  pitch: number;
  roll: number;
}

/** Snapshots the viewer's current camera pose. */
export function getCameraView(viewer: Cesium.Viewer): CameraView {
  const { camera } = viewer;
  const cartographic = camera.positionCartographic;
  return {
    longitude: Cesium.Math.toDegrees(cartographic.longitude),
    latitude: Cesium.Math.toDegrees(cartographic.latitude),
    height: cartographic.height,
    heading: camera.heading,
    pitch: camera.pitch,
    roll: camera.roll,
  };
}

/** Restores a camera pose instantly (no fly animation). */
export function applyCameraView(viewer: Cesium.Viewer, view: CameraView): void {
  viewer.camera.setView({
    destination: Cesium.Cartesian3.fromDegrees(view.longitude, view.latitude, view.height),
    orientation: {
      heading: view.heading,
      pitch: view.pitch,
      roll: view.roll,
    },
  });
}
