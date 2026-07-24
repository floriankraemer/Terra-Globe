import * as Cesium from "cesium";
import { geometryCenter, type GeoPoint, type PlacemarkGeometry, type Style } from "@webglobe/core";
import type { EntityHandle, IEntityFactory } from "./IEntityFactory.js";

const DEFAULT_OUTLINE_COLOR = Cesium.Color.DODGERBLUE;
const DEFAULT_OUTLINE_WIDTH = 2;
const DEFAULT_FILL_COLOR = Cesium.Color.DODGERBLUE;
const DEFAULT_FILL_OPACITY = 0.4;

// Name labels only appear once the camera is closer than this (meters) - at
// wide zoom levels every placemark's name would otherwise clutter the globe.
const LABEL_MAX_DISTANCE_METERS = 50_000;
const LABEL_NEAR_DISTANCE_METERS = 1_000;
const LABEL_NEAR_SCALE = 1;
const LABEL_FAR_SCALE = 0.4;

function labelGraphics(
  name: string | undefined,
): Cesium.LabelGraphics.ConstructorOptions | undefined {
  if (!name) return undefined;
  return {
    text: name,
    font: "14px sans-serif",
    fillColor: Cesium.Color.WHITE,
    outlineColor: Cesium.Color.BLACK,
    outlineWidth: 2,
    style: Cesium.LabelStyle.FILL_AND_OUTLINE,
    verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
    pixelOffset: new Cesium.Cartesian2(0, -12),
    distanceDisplayCondition: new Cesium.DistanceDisplayCondition(0, LABEL_MAX_DISTANCE_METERS),
    scaleByDistance: new Cesium.NearFarScalar(
      LABEL_NEAR_DISTANCE_METERS,
      LABEL_NEAR_SCALE,
      LABEL_MAX_DISTANCE_METERS,
      LABEL_FAR_SCALE,
    ),
  };
}

function fillColor(style: Style | undefined): Cesium.Color {
  if (!style) return DEFAULT_FILL_COLOR.withAlpha(DEFAULT_FILL_OPACITY);
  return Cesium.Color.fromCssColorString(style.fillColor).withAlpha(style.fillOpacity);
}

function outlineColor(style: Style | undefined): Cesium.Color {
  if (!style) return DEFAULT_OUTLINE_COLOR;
  return Cesium.Color.fromCssColorString(style.outlineColor);
}

function outlineWidth(style: Style | undefined): number {
  return style?.outlineWidth ?? DEFAULT_OUTLINE_WIDTH;
}

function outlineEnabled(style: Style | undefined): boolean {
  return style?.outlineEnabled ?? true;
}

function filled(style: Style | undefined): boolean {
  return style?.filled ?? false;
}

function toEntityOptions(
  geometry: PlacemarkGeometry,
  style?: Style,
  name?: string,
): Cesium.Entity.ConstructorOptions {
  const options = toGeometryOptions(geometry, style);
  const label = labelGraphics(name);
  if (label) {
    // Rectangle/Polygon/LineString have no entity.position of their own
    // (their coordinates live on the rectangle/polygon/polyline graphics) -
    // give the label an anchor at the geometry's center.
    options.position ??= Cesium.Cartesian3.fromDegrees(
      geometryCenter(geometry).lon,
      geometryCenter(geometry).lat,
    );
    options.label = label;
  }
  return options;
}

function toGeometryOptions(
  geometry: PlacemarkGeometry,
  style?: Style,
): Cesium.Entity.ConstructorOptions {
  switch (geometry.type) {
    case "Point":
      return {
        position: Cesium.Cartesian3.fromDegrees(geometry.coordinates.lon, geometry.coordinates.lat),
        point: {
          pixelSize: 10,
          color: style ? Cesium.Color.fromCssColorString(style.fillColor) : Cesium.Color.RED,
          outlineColor: outlineColor(style),
          outlineWidth: 1,
        },
      };
    case "Circle":
      return {
        position: Cesium.Cartesian3.fromDegrees(geometry.center.lon, geometry.center.lat),
        ellipse: {
          semiMinorAxis: geometry.radiusMeters,
          semiMajorAxis: geometry.radiusMeters,
          fill: filled(style),
          material: fillColor(style),
          outline: outlineEnabled(style),
          outlineColor: outlineColor(style),
          outlineWidth: outlineWidth(style),
        },
      };
    case "Rectangle":
      return {
        rectangle: {
          coordinates: Cesium.Rectangle.fromDegrees(
            geometry.west,
            geometry.south,
            geometry.east,
            geometry.north,
          ),
          fill: filled(style),
          material: fillColor(style),
          outline: outlineEnabled(style),
          outlineColor: outlineColor(style),
          outlineWidth: outlineWidth(style),
        },
      };
    case "Polygon":
      return {
        polygon: {
          hierarchy: new Cesium.PolygonHierarchy(
            geometry.outerRing.map((p: GeoPoint) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
            geometry.innerRings?.map(
              (ring: GeoPoint[]) =>
                new Cesium.PolygonHierarchy(
                  ring.map((p: GeoPoint) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
                ),
            ),
          ),
          fill: filled(style),
          material: fillColor(style),
          outline: outlineEnabled(style),
          outlineColor: outlineColor(style),
          outlineWidth: outlineWidth(style),
        },
      };
    case "LineString":
      return {
        polyline: {
          positions: geometry.path.map((p: GeoPoint) =>
            Cesium.Cartesian3.fromDegrees(p.lon, p.lat),
          ),
          width: outlineWidth(style),
          material: outlineColor(style),
        },
      };
    default: {
      const exhaustiveCheck: never = geometry;
      throw new Error(`Unhandled geometry type: ${JSON.stringify(exhaustiveCheck)}`);
    }
  }
}

export class CesiumEntityFactory implements IEntityFactory {
  constructor(private readonly entities: Cesium.EntityCollection) {}

  createEntity(
    geometry: PlacemarkGeometry,
    id?: string,
    style?: Style,
    name?: string,
  ): EntityHandle {
    const entity = this.entities.add({ id, ...toEntityOptions(geometry, style, name) });
    return { entityId: entity.id };
  }

  updateEntity(
    handle: EntityHandle,
    geometry: PlacemarkGeometry,
    style?: Style,
    name?: string,
  ): void {
    const entity = this.entities.getById(handle.entityId);
    if (!entity) throw new Error(`Unknown entity: ${handle.entityId}`);

    // Mutate the entity's existing graphics in place rather than remove+add.
    // Removing and re-adding forces Cesium to tear down and rebuild the GPU
    // primitives (expensive tessellation for ellipse/rectangle/polygon) on
    // every call - this is what made the rectangle/circle drag preview,
    // which calls updateEntity on every mouse-move tick, feel sluggish.
    const options = toEntityOptions(geometry, style, name);
    entity.position = options.position
      ? new Cesium.ConstantPositionProperty(options.position as Cesium.Cartesian3)
      : undefined;
    entity.point = options.point ? new Cesium.PointGraphics(options.point) : undefined;
    entity.ellipse = options.ellipse
      ? new Cesium.EllipseGraphics(options.ellipse as Cesium.EllipseGraphics.ConstructorOptions)
      : undefined;
    entity.rectangle = options.rectangle
      ? new Cesium.RectangleGraphics(
          options.rectangle as Cesium.RectangleGraphics.ConstructorOptions,
        )
      : undefined;
    entity.polygon = options.polygon
      ? new Cesium.PolygonGraphics(options.polygon as Cesium.PolygonGraphics.ConstructorOptions)
      : undefined;
    entity.polyline = options.polyline
      ? new Cesium.PolylineGraphics(options.polyline as Cesium.PolylineGraphics.ConstructorOptions)
      : undefined;
    entity.label = options.label ? new Cesium.LabelGraphics(options.label) : undefined;
  }

  removeEntity(handle: EntityHandle): void {
    const entity = this.entities.getById(handle.entityId);
    if (entity) this.entities.remove(entity);
  }
}
