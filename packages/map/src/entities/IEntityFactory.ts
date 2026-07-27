import type { PlacemarkGeometry, Style } from "@terra-globe/core";

export interface EntityHandle {
  entityId: string;
}

/**
 * Seam between drawing logic and the concrete rendering engine (Cesium).
 * Keeps DrawingController/tools testable without a WebGL context.
 */
export interface IEntityFactory {
  createEntity(
    geometry: PlacemarkGeometry,
    id?: string,
    style?: Style,
    name?: string,
  ): EntityHandle;
  updateEntity(
    handle: EntityHandle,
    geometry: PlacemarkGeometry,
    style?: Style,
    name?: string,
  ): void;
  removeEntity(handle: EntityHandle): void;
  setVisible(handle: EntityHandle, visible: boolean): void;
  /**
   * Swaps an existing entity's graphics to a CallbackProperty-driven shape
   * (like createLivePreview, but mutating the real entity in place instead of
   * adding a new one) so a caller can cheaply update its geometry every
   * animation frame - e.g. during a drag - without Cesium re-tessellating a
   * brand-new graphics instance on every tick. Callers must finish the drag
   * with a normal updateEntity() call to restore optimized static graphics.
   */
  beginLiveGeometryEdit(
    handle: EntityHandle,
    initialGeometry: PlacemarkGeometry,
    style?: Style,
  ): (geometry: PlacemarkGeometry) => void;
}
