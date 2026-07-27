import type { PlacemarkGeometry, Style } from "@terra-globe/core";
import type { EntityHandle, IEntityFactory } from "./IEntityFactory.js";

export interface FakeEntity {
  geometry: PlacemarkGeometry;
  style?: Style;
  name?: string;
  visible: boolean;
}

/** In-memory test double for IEntityFactory used by unit tests. */
export class FakeEntityFactory implements IEntityFactory {
  private nextId = 1;
  readonly entities = new Map<string, FakeEntity>();
  readonly removed = new Set<string>();

  createEntity(
    geometry: PlacemarkGeometry,
    id?: string,
    style?: Style,
    name?: string,
  ): EntityHandle {
    const entityId = id ?? `fake-entity-${this.nextId++}`;
    this.entities.set(entityId, { geometry, style, name, visible: true });
    return { entityId };
  }

  updateEntity(
    handle: EntityHandle,
    geometry: PlacemarkGeometry,
    style?: Style,
    name?: string,
  ): void {
    const existing = this.entities.get(handle.entityId);
    if (!existing) {
      throw new Error(`Unknown entity: ${handle.entityId}`);
    }
    this.entities.set(handle.entityId, { geometry, style, name, visible: existing.visible });
  }

  removeEntity(handle: EntityHandle): void {
    this.entities.delete(handle.entityId);
    this.removed.add(handle.entityId);
  }

  setVisible(handle: EntityHandle, visible: boolean): void {
    const existing = this.entities.get(handle.entityId);
    if (existing) existing.visible = visible;
  }

  beginLiveGeometryEdit(
    handle: EntityHandle,
    initialGeometry: PlacemarkGeometry,
    style?: Style,
  ): (geometry: PlacemarkGeometry) => void {
    this.updateEntity(handle, initialGeometry, style);
    return (geometry: PlacemarkGeometry) => this.updateEntity(handle, geometry, style);
  }
}
