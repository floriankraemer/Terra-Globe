import type { PlacemarkGeometry, Style } from "@webglobe/core";
import type { EntityHandle, IEntityFactory } from "./IEntityFactory.js";

export interface FakeEntity {
  geometry: PlacemarkGeometry;
  style?: Style;
  name?: string;
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
    this.entities.set(entityId, { geometry, style, name });
    return { entityId };
  }

  updateEntity(
    handle: EntityHandle,
    geometry: PlacemarkGeometry,
    style?: Style,
    name?: string,
  ): void {
    if (!this.entities.has(handle.entityId)) {
      throw new Error(`Unknown entity: ${handle.entityId}`);
    }
    this.entities.set(handle.entityId, { geometry, style, name });
  }

  removeEntity(handle: EntityHandle): void {
    this.entities.delete(handle.entityId);
    this.removed.add(handle.entityId);
  }
}
