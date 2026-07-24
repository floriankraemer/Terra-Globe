import * as Cesium from "cesium";
import type { GeoPoint } from "@terra-globe/core";
import type { IScreenPicker, ScreenPosition } from "./IScreenPicker.js";

export class CesiumScreenPicker implements IScreenPicker {
  constructor(private readonly viewer: Cesium.Viewer) {}

  pickGround(position: ScreenPosition): GeoPoint | undefined {
    const cartesian = this.viewer.camera.pickEllipsoid(
      new Cesium.Cartesian2(position.x, position.y),
      this.viewer.scene.globe.ellipsoid,
    );
    if (!cartesian) return undefined;

    const carto = Cesium.Cartographic.fromCartesian(cartesian);
    return {
      lon: Cesium.Math.toDegrees(carto.longitude),
      lat: Cesium.Math.toDegrees(carto.latitude),
    };
  }
}
