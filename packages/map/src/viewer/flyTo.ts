import * as Cesium from "cesium";
import {
  circleToPolygonRing,
  type GeoPoint,
  type PlacemarkGeometry,
  type PlacemarkView,
} from "@terra-globe/core";

// A single point (Point/Model) has zero extent - without a floor, the
// bounding sphere would have radius 0 and fly the camera down to ground
// level instead of a sane overview height.
const MIN_BOUNDING_RADIUS_METERS = 500;

function toCartesian(point: GeoPoint): Cesium.Cartesian3 {
  return Cesium.Cartesian3.fromDegrees(point.lon, point.lat, point.altitude ?? 0);
}

function geometryPoints(geometry: PlacemarkGeometry): GeoPoint[] {
  switch (geometry.type) {
    case "Point":
      return [geometry.coordinates];
    case "Model":
      return [geometry.position];
    case "Circle":
      return circleToPolygonRing(geometry.center, geometry.radiusMeters);
    case "Rectangle":
      return [
        { lon: geometry.west, lat: geometry.south },
        { lon: geometry.east, lat: geometry.north },
      ];
    case "GroundOverlay":
      return [
        { lon: geometry.bounds.west, lat: geometry.bounds.south },
        { lon: geometry.bounds.east, lat: geometry.bounds.north },
      ];
    case "Polygon":
      return geometry.outerRing;
    case "LineString":
      return geometry.path;
  }
}

/** Flies the camera to frame a geometry's full extent (not just its center point). */
export function flyToGeometry(viewer: Cesium.Viewer, geometry: PlacemarkGeometry): void {
  const positions = geometryPoints(geometry).map(toCartesian);
  const sphere = Cesium.BoundingSphere.fromPoints(positions);
  sphere.radius = Math.max(sphere.radius, MIN_BOUNDING_RADIUS_METERS);
  // Cesium's flyToBoundingSphere defaults to a fixed 45deg pitch (Camera.DEFAULT_OFFSET)
  // when no offset is given, forcing a tilt in 3D mode no matter the current view.
  // Look straight down instead, matching the top-down framing 2D/Columbus already use.
  viewer.camera.flyToBoundingSphere(sphere, {
    offset: new Cesium.HeadingPitchRange(0, -Cesium.Math.PI_OVER_TWO, 0),
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
