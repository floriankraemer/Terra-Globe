import type { PlacemarkGeometry, Style } from "@webglobe/core";

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
}
