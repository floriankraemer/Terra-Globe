import * as Cesium from "cesium";
import { geometryCenter, type PlacemarkGeometry, type PlacemarkView } from "@webglobe/core";

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
    case "GroundOverlay": {
      const spanDegrees = Math.max(
        geometry.bounds.north - geometry.bounds.south,
        geometry.bounds.east - geometry.bounds.west,
      );
      return Math.max(spanDegrees * DEGREES_TO_METERS * 2, MIN_ALTITUDE_METERS);
    }
    case "Model":
      return 10_000;
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

function num(params: Record<string, number | string>, key: string, fallback: number): number {
  const value = params[key];
  return value === undefined ? fallback : Number(value);
}

/** Flies the camera to a KML Camera/LookAt view saved on a placemark. */
export function flyToView(viewer: Cesium.Viewer, view: PlacemarkView): void {
  const { params } = view;
  const longitude = num(params, "longitude", 0);
  const latitude = num(params, "latitude", 0);

  if (view.kind === "Camera") {
    viewer.camera.flyTo({
      destination: Cesium.Cartesian3.fromDegrees(
        longitude,
        latitude,
        num(params, "altitude", 1000),
      ),
      orientation: {
        heading: Cesium.Math.toRadians(num(params, "heading", 0)),
        pitch: Cesium.Math.toRadians(num(params, "tilt", 0) - 90),
        roll: Cesium.Math.toRadians(num(params, "roll", 0)),
      },
    });
    return;
  }

  viewer.camera.flyTo({
    destination: Cesium.Cartesian3.fromDegrees(longitude, latitude, num(params, "range", 1000)),
    orientation: {
      heading: Cesium.Math.toRadians(num(params, "heading", 0)),
      pitch: Cesium.Math.toRadians(num(params, "tilt", 0) - 90),
    },
  });
}
