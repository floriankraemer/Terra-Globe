import * as Cesium from "cesium";
import {
  circleToPolygonRing,
  geometryCenter,
  type AltitudeMode,
  type CircleGeometry,
  type GeoPoint,
  type GroundOverlayGeometry,
  type LineStringGeometry,
  type ModelGeometry,
  type PlacemarkGeometry,
  type PointGeometry,
  type PolygonGeometry,
  type RectangleGeometry,
  type Style,
} from "@terra-globe/core";
import type { EntityHandle, IEntityFactory } from "./IEntityFactory.js";
import { markerIconCanvas } from "./markerIcons.js";

const HEIGHT_REFERENCE = {
  clampToGround: Cesium.HeightReference.CLAMP_TO_GROUND,
  relativeToGround: Cesium.HeightReference.RELATIVE_TO_GROUND,
  absolute: Cesium.HeightReference.NONE,
} as const;

// Without real elevation data, `HeightReference.NONE` places a marker at its
// literal stored altitude above the ellipsoid while the basemap imagery is
// draped flat at the ellipsoid surface - a marker with a KML "absolute"
// altitude (e.g. imported GPS elevation) then renders floating in mid-air.
// Once a real terrain provider is loaded, that same height reference
// correctly floats the marker above the actual 3D terrain instead, so this
// is re-checked on every render rather than cached at construction time.
function hasElevationTerrain(viewer: Cesium.Viewer): boolean {
  return !(viewer.terrainProvider instanceof Cesium.EllipsoidTerrainProvider);
}

// KML omits altitudeMode far more often than it sets it, and per spec that
// means "clampToGround" - not "absolute". Falling back to NONE (absolute)
// here was the actual cause of ordinary imported points floating: their
// <coordinates> altitude (e.g. GPS elevation) got treated as literal height
// above the ellipsoid. clampToGround also doubles as the "no DEM available"
// normalization: it snaps to the basemap surface when terrain is flat, and
// to the real 3D height model once a terrain provider supplies one.
function pointHeightReference(
  altitudeMode: AltitudeMode | undefined,
  viewer: Cesium.Viewer,
): Cesium.HeightReference {
  if (!hasElevationTerrain(viewer)) return Cesium.HeightReference.CLAMP_TO_GROUND;
  return HEIGHT_REFERENCE[altitudeMode ?? "clampToGround"];
}

// KML Model.Link can point at any 3D format (COLLADA .dae is the KML norm) -
// Cesium's ModelGraphics only loads glTF, so anything else falls back to a
// plain point marker rather than a new model-conversion subsystem.
function isGltfUri(uri: string): boolean {
  return /\.(gltf|glb)(\?.*)?$/i.test(uri);
}

function modelOrientation(geometry: ModelGeometry): Cesium.Property | undefined {
  if (
    geometry.heading === undefined &&
    geometry.tilt === undefined &&
    geometry.roll === undefined
  ) {
    return undefined;
  }
  const position = Cesium.Cartesian3.fromDegrees(
    geometry.position.lon,
    geometry.position.lat,
    geometry.position.altitude ?? 0,
  );
  const hpr = new Cesium.HeadingPitchRoll(
    Cesium.Math.toRadians(geometry.heading ?? 0),
    Cesium.Math.toRadians(geometry.tilt ?? 0),
    Cesium.Math.toRadians(geometry.roll ?? 0),
  );
  return new Cesium.ConstantProperty(Cesium.Transforms.headingPitchRollQuaternion(position, hpr));
}

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
    font: "bold 16px sans-serif",
    fillColor: Cesium.Color.WHITE,
    outlineColor: Cesium.Color.BLACK,
    outlineWidth: 2,
    style: Cesium.LabelStyle.FILL_AND_OUTLINE,
    // Marker billboard is CANVAS_PIXEL_SIZE (48px) tall, anchored bottom-up
    // at the entity position - offset above that, not just above the point,
    // so the label clears the icon instead of sitting behind it.
    verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
    pixelOffset: new Cesium.Cartesian2(0, -52),
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

/** Renders a Point placemark as a colored marker icon rather than a plain dot. */
function markerBillboard(
  style: Style | undefined,
  heightReference?: Cesium.HeightReference,
): Cesium.BillboardGraphics.ConstructorOptions {
  return {
    image: markerIconCanvas(
      style?.markerIcon,
      style?.fillColor ?? "#FF0000",
      style?.outlineColor ?? "#FFFFFF",
    ),
    verticalOrigin: Cesium.VerticalOrigin.BOTTOM,
    heightReference,
  };
}

function closeRing(ring: GeoPoint[]): GeoPoint[] {
  const first = ring[0]!;
  const last = ring[ring.length - 1]!;
  return first.lon === last.lon && first.lat === last.lat ? ring : [...ring, first];
}

// Ellipse/Rectangle/Polygon graphics render their native `outline` as raw
// GL_LINES, whose width is capped to 1px on every browser except Firefox -
// so `outlineWidth` silently has no visible effect there. Draping a real
// PolylineGraphics around the boundary instead works everywhere, since
// polylines are rendered as camera-facing ribbons, not GL line primitives.
// Point/LineString aren't affected (point outlines aren't GL lines, and a
// LineString's own polyline already has correct width) - no companion needed.
function outlineBoundaryLoops(geometry: PlacemarkGeometry, style: Style | undefined): GeoPoint[][] {
  if (!outlineEnabled(style)) return [];
  switch (geometry.type) {
    case "Circle":
      return [circleToPolygonRing(geometry.center, geometry.radiusMeters)];
    case "Rectangle": {
      const { north, south, east, west } = geometry;
      return [
        closeRing([
          { lon: west, lat: south },
          { lon: east, lat: south },
          { lon: east, lat: north },
          { lon: west, lat: north },
        ]),
      ];
    }
    case "Polygon":
      return [closeRing(geometry.outerRing), ...(geometry.innerRings ?? []).map(closeRing)];
    default:
      return [];
  }
}

function toEntityOptions(
  geometry: PlacemarkGeometry,
  viewer: Cesium.Viewer,
  style?: Style,
  name?: string,
): Cesium.Entity.ConstructorOptions {
  const options = toGeometryOptions(geometry, viewer, style);
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
  viewer: Cesium.Viewer,
  style?: Style,
): Cesium.Entity.ConstructorOptions {
  switch (geometry.type) {
    case "Point":
      return {
        position: Cesium.Cartesian3.fromDegrees(
          geometry.coordinates.lon,
          geometry.coordinates.lat,
          geometry.coordinates.altitude,
        ),
        billboard: markerBillboard(
          style,
          pointHeightReference(geometry.coordinates.altitudeMode, viewer),
        ),
      };
    case "Circle":
      return {
        position: Cesium.Cartesian3.fromDegrees(geometry.center.lon, geometry.center.lat),
        ellipse: {
          semiMinorAxis: geometry.radiusMeters,
          semiMajorAxis: geometry.radiusMeters,
          fill: filled(style),
          material: fillColor(style),
          // Outline is drawn by a companion PolylineGraphics (see
          // outlineBoundaryLoops) - the native outline is always off here.
          outline: false,
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
          outline: false,
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
          outline: false,
          extrudedHeight: geometry.extrudeHeight,
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
          clampToGround: geometry.tessellate,
        },
      };
    case "GroundOverlay":
      return {
        rectangle: {
          coordinates: Cesium.Rectangle.fromDegrees(
            geometry.bounds.west,
            geometry.bounds.south,
            geometry.bounds.east,
            geometry.bounds.north,
          ),
          material: new Cesium.ImageMaterialProperty({ image: geometry.imageUrl }),
          rotation:
            geometry.rotation !== undefined ? Cesium.Math.toRadians(geometry.rotation) : undefined,
        },
      };
    case "Model": {
      const position = Cesium.Cartesian3.fromDegrees(
        geometry.position.lon,
        geometry.position.lat,
        geometry.position.altitude ?? 0,
      );
      if (!isGltfUri(geometry.modelUri)) {
        // Non-glTF (typically KML's usual COLLADA .dae) - show a plain
        // marker rather than skip the placemark entirely.
        return {
          position,
          billboard: markerBillboard(undefined),
        };
      }
      return {
        position,
        orientation: modelOrientation(geometry),
        model: { uri: geometry.modelUri, scale: geometry.scale },
      };
    }
    default: {
      const exhaustiveCheck: never = geometry;
      throw new Error(`Unhandled geometry type: ${JSON.stringify(exhaustiveCheck)}`);
    }
  }
}

function toDynamicGeometryOptions(
  getGeometry: () => PlacemarkGeometry,
  viewer: Cesium.Viewer,
  style: Style | undefined,
): Cesium.Entity.ConstructorOptions {
  // Only the vertex/shape data changes every animation frame during a drag;
  // colors and flags come from `style`, which stays fixed for a preview.
  const shape = getGeometry();
  switch (shape.type) {
    case "Point":
      return {
        position: new Cesium.CallbackPositionProperty(() => {
          const g = getGeometry() as PointGeometry;
          return Cesium.Cartesian3.fromDegrees(
            g.coordinates.lon,
            g.coordinates.lat,
            g.coordinates.altitude,
          );
        }, false),
        billboard: markerBillboard(
          style,
          pointHeightReference(shape.coordinates.altitudeMode, viewer),
        ),
      };
    case "Circle":
      return {
        position: new Cesium.CallbackPositionProperty(() => {
          const g = getGeometry() as CircleGeometry;
          return Cesium.Cartesian3.fromDegrees(g.center.lon, g.center.lat);
        }, false),
        ellipse: {
          semiMinorAxis: new Cesium.CallbackProperty(
            () => (getGeometry() as CircleGeometry).radiusMeters,
            false,
          ),
          semiMajorAxis: new Cesium.CallbackProperty(
            () => (getGeometry() as CircleGeometry).radiusMeters,
            false,
          ),
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
          coordinates: new Cesium.CallbackProperty(() => {
            const g = getGeometry() as RectangleGeometry;
            return Cesium.Rectangle.fromDegrees(g.west, g.south, g.east, g.north);
          }, false),
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
          hierarchy: new Cesium.CallbackProperty(() => {
            const g = getGeometry() as PolygonGeometry;
            return new Cesium.PolygonHierarchy(
              g.outerRing.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
              g.innerRings?.map(
                (ring) =>
                  new Cesium.PolygonHierarchy(
                    ring.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
                  ),
              ),
            );
          }, false),
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
          positions: new Cesium.CallbackProperty(() => {
            const g = getGeometry() as LineStringGeometry;
            return g.path.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat));
          }, false),
          width: outlineWidth(style),
          material: outlineColor(style),
          clampToGround: shape.type === "LineString" ? shape.tessellate : undefined,
        },
      };
    case "GroundOverlay":
      return {
        rectangle: {
          coordinates: new Cesium.CallbackProperty(() => {
            const g = getGeometry() as GroundOverlayGeometry;
            return Cesium.Rectangle.fromDegrees(
              g.bounds.west,
              g.bounds.south,
              g.bounds.east,
              g.bounds.north,
            );
          }, false),
          material: new Cesium.ImageMaterialProperty({ image: shape.imageUrl }),
          rotation:
            shape.rotation !== undefined ? Cesium.Math.toRadians(shape.rotation) : undefined,
        },
      };
    case "Model":
      return {
        ...toGeometryOptions(shape, viewer, style),
        position: new Cesium.CallbackPositionProperty(() => {
          const g = getGeometry() as ModelGeometry;
          return Cesium.Cartesian3.fromDegrees(
            g.position.lon,
            g.position.lat,
            g.position.altitude ?? 0,
          );
        }, false),
      };
    default: {
      const exhaustiveCheck: never = shape;
      throw new Error(`Unhandled geometry type: ${JSON.stringify(exhaustiveCheck)}`);
    }
  }
}

export class CesiumEntityFactory implements IEntityFactory {
  private readonly entities: Cesium.EntityCollection;

  constructor(private readonly viewer: Cesium.Viewer) {
    this.entities = viewer.entities;
  }

  // Companion outline-polyline entities per main entity id (see
  // outlineBoundaryLoops). A placemark's geometry never changes after
  // creation (only its style/name do), so the loop count per id is stable -
  // only rebuilt here if it ever isn't.
  private readonly outlineEntityIds = new Map<string, string[]>();

  private syncOutlineEntities(mainId: string, geometry: PlacemarkGeometry, style?: Style): void {
    const loops = outlineBoundaryLoops(geometry, style);
    const existingIds = this.outlineEntityIds.get(mainId) ?? [];

    if (existingIds.length !== loops.length) {
      existingIds.forEach((id) => {
        const entity = this.entities.getById(id);
        if (entity) this.entities.remove(entity);
      });
      const createdIds = loops.map(
        (loop) =>
          this.entities.add({
            polyline: {
              positions: loop.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
              width: outlineWidth(style),
              material: outlineColor(style),
            },
          }).id,
      );
      this.outlineEntityIds.set(mainId, createdIds);
      return;
    }

    loops.forEach((loop, i) => {
      const entity = this.entities.getById(existingIds[i]!);
      if (!entity) return;
      entity.polyline = new Cesium.PolylineGraphics({
        positions: loop.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat)),
        width: outlineWidth(style),
        material: outlineColor(style),
      });
    });
  }

  private removeOutlineEntities(mainId: string): void {
    const ids = this.outlineEntityIds.get(mainId);
    if (!ids) return;
    ids.forEach((id) => {
      const entity = this.entities.getById(id);
      if (entity) this.entities.remove(entity);
    });
    this.outlineEntityIds.delete(mainId);
  }

  /**
   * Creates a preview entity whose shape is read from a CallbackProperty
   * instead of being rebuilt on every call. The entity/graphics objects are
   * constructed exactly once; each animation frame only overwrites a plain
   * `current` variable, and Cesium itself decides when to re-tessellate (once
   * per its own render tick) rather than us forcing a full
   * remove+recreate/graphics-replacement synchronously from the mousemove
   * handler.
   */
  createLivePreview(
    initialGeometry: PlacemarkGeometry,
    id?: string,
    style?: Style,
  ): { handle: EntityHandle; setGeometry: (geometry: PlacemarkGeometry) => void } {
    let current = initialGeometry;
    const entity = this.entities.add({
      id,
      ...toDynamicGeometryOptions(() => current, this.viewer, style),
    });
    return {
      handle: { entityId: entity.id },
      setGeometry: (geometry: PlacemarkGeometry) => {
        current = geometry;
      },
    };
  }

  createEntity(
    geometry: PlacemarkGeometry,
    id?: string,
    style?: Style,
    name?: string,
  ): EntityHandle {
    const entity = this.entities.add({
      id,
      ...toEntityOptions(geometry, this.viewer, style, name),
    });
    this.syncOutlineEntities(entity.id, geometry, style);
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
    const options = toEntityOptions(geometry, this.viewer, style, name);
    entity.position = options.position
      ? new Cesium.ConstantPositionProperty(options.position as Cesium.Cartesian3)
      : undefined;
    entity.billboard = options.billboard
      ? new Cesium.BillboardGraphics(options.billboard)
      : undefined;
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
    entity.model = options.model
      ? new Cesium.ModelGraphics(options.model as Cesium.ModelGraphics.ConstructorOptions)
      : undefined;
    entity.orientation = options.orientation as Cesium.Property | undefined;
    entity.label = options.label ? new Cesium.LabelGraphics(options.label) : undefined;

    this.syncOutlineEntities(handle.entityId, geometry, style);
  }

  removeEntity(handle: EntityHandle): void {
    const entity = this.entities.getById(handle.entityId);
    if (entity) this.entities.remove(entity);
    this.removeOutlineEntities(handle.entityId);
  }

  setVisible(handle: EntityHandle, visible: boolean): void {
    const entity = this.entities.getById(handle.entityId);
    if (entity) entity.show = visible;
    const outlineIds = this.outlineEntityIds.get(handle.entityId) ?? [];
    outlineIds.forEach((id) => {
      const outline = this.entities.getById(id);
      if (outline) outline.show = visible;
    });
  }

  beginLiveGeometryEdit(
    handle: EntityHandle,
    initialGeometry: PlacemarkGeometry,
    style?: Style,
  ): (geometry: PlacemarkGeometry) => void {
    const entity = this.entities.getById(handle.entityId);
    if (!entity) throw new Error(`Unknown entity: ${handle.entityId}`);

    let current = initialGeometry;
    const options = toDynamicGeometryOptions(() => current, this.viewer, style);

    // Rectangle/Polygon/LineString have no entity.position of their own in
    // toDynamicGeometryOptions (same as the static path) - give them a
    // callback-driven center so the label and Cesium's own selection
    // indicator both track the shape as it moves, instead of staying pinned
    // to the pre-drag center.
    entity.position =
      (options.position as Cesium.PositionProperty | undefined) ??
      new Cesium.CallbackPositionProperty(() => {
        const center = geometryCenter(current);
        return Cesium.Cartesian3.fromDegrees(center.lon, center.lat);
      }, false);
    entity.billboard = options.billboard
      ? new Cesium.BillboardGraphics(options.billboard)
      : undefined;
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

    // Companion outline loops (Circle/Rectangle/Polygon) would otherwise be
    // rebuilt with a brand-new PolylineGraphics every frame via
    // syncOutlineEntities - swap them to a single CallbackProperty-driven
    // polyline instead, for the same reason as the main shape above.
    const outlineIds = this.outlineEntityIds.get(handle.entityId) ?? [];
    outlineIds.forEach((id, i) => {
      const outline = this.entities.getById(id);
      if (!outline) return;
      outline.polyline = new Cesium.PolylineGraphics({
        positions: new Cesium.CallbackProperty(() => {
          const loop = outlineBoundaryLoops(current, style)[i] ?? [];
          return loop.map((p) => Cesium.Cartesian3.fromDegrees(p.lon, p.lat));
        }, false),
        width: outlineWidth(style),
        material: outlineColor(style),
      });
    });

    return (geometry: PlacemarkGeometry) => {
      current = geometry;
    };
  }
}
