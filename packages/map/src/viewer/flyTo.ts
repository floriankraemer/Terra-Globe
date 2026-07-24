import * as Cesium from "cesium";
import { geometryCenter, type PlacemarkGeometry } from "@webglobe/core";

const MIN_ALTITUDE_METERS = 2000;
const DEGREES_TO_METERS = 111_320;

function altitudeForGeometry(geometry: PlacemarkGeometry): number {
  switch (geometry.type) {
    case "Point":
      return 10_000;
    case "Circle":
      return Math.max(geometry.radiusMeters * 4, MIN_ALTITUDE_METERS);
    case "Rectangle": {
      const spanDegrees = Math.max(geometry.north - geometry.south, geometry.east - geometry.west);
      return Math.max(spanDegrees * DEGREES_TO_METERS * 2, MIN_ALTITUDE_METERS);
    }
    case "Polygon": {
      const lons = geometry.outerRing.map((p) => p.lon);
      const lats = geometry.outerRing.map((p) => p.lat);
      const spanDegrees = Math.max(
        Math.max(...lons) - Math.min(...lons),
        Math.max(...lats) - Math.min(...lats),
      );
      return Math.max(spanDegrees * DEGREES_TO_METERS * 2, MIN_ALTITUDE_METERS);
    }
    case "LineString": {
      const lons = geometry.path.map((p) => p.lon);
      const lats = geometry.path.map((p) => p.lat);
      const spanDegrees = Math.max(
        Math.max(...lons) - Math.min(...lons),
        Math.max(...lats) - Math.min(...lats),
      );
      return Math.max(spanDegrees * DEGREES_TO_METERS * 2, MIN_ALTITUDE_METERS);
    }
  }
}

/** Flies the camera to hover directly above a geometry's center, at a height scaled to its size. */
export function flyToGeometry(viewer: Cesium.Viewer, geometry: PlacemarkGeometry): void {
  const center = geometryCenter(geometry);
  viewer.camera.flyTo({
    destination: Cesium.Cartesian3.fromDegrees(
      center.lon,
      center.lat,
      altitudeForGeometry(geometry),
    ),
  });
}
