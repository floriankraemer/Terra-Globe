import {
  createStyle,
  type Folder,
  type NewPlacemark,
  type Placemark,
  type PlacemarkGeometry,
  type PlacesRepository,
} from "@terra-globe/core";
import type { IEntityFactory } from "../entities/IEntityFactory.js";

const FILL_OPACITY = 0.5;

export interface PlacemarkStyleEdits {
  outlineEnabled: boolean;
  outlineColor: string;
  outlineWidth: number;
  filled: boolean;
  fillColor: string;
}

/** Bridges PlacesRepository persistence with live entities in the map's IEntityFactory. */
export class EntitySynchronizer {
  constructor(
    private readonly repository: PlacesRepository,
    private readonly entityFactory: IEntityFactory,
  ) {}

  /** Recursively loads every persisted placemark and renders it as an entity. */
  async loadAll(): Promise<void> {
    const placemarks = await this.collectPlacemarks(null);
    await this.renderPlacemarks(placemarks);
  }

  /** Renders entities for placemarks already known to be persisted (e.g. just imported). */
  async renderPlacemarks(placemarks: Placemark[]): Promise<void> {
    for (const placemark of placemarks) {
      const style = placemark.styleId ? await this.repository.getStyle(placemark.styleId) : null;
      this.entityFactory.createEntity(
        placemark.geometry,
        placemark.id,
        style ?? undefined,
        placemark.name,
      );
    }
  }

  private async collectPlacemarks(folderId: string | null): Promise<Placemark[]> {
    const [placemarks, folders] = await Promise.all([
      this.repository.listPlacemarks(folderId),
      this.repository.listFolders(folderId),
    ]);
    const nested = await Promise.all(
      folders.map((folder: Folder) => this.collectPlacemarks(folder.id)),
    );
    return [...placemarks, ...nested.flat()];
  }

  async persistPlacemark(input: NewPlacemark): Promise<Placemark> {
    const placemark = await this.repository.createPlacemark(input);
    const style = placemark.styleId ? await this.repository.getStyle(placemark.styleId) : null;
    this.entityFactory.createEntity(
      placemark.geometry,
      placemark.id,
      style ?? undefined,
      placemark.name,
    );
    return placemark;
  }

  /** Updates a placemark's fields (name/description/...) and re-renders its live entity. */
  async updatePlacemark(id: string, patch: Partial<Omit<Placemark, "id">>): Promise<Placemark> {
    const updated = await this.repository.updatePlacemark(id, patch);
    const style = updated.styleId ? await this.repository.getStyle(updated.styleId) : null;
    this.entityFactory.updateEntity(
      { entityId: id },
      updated.geometry,
      style ?? undefined,
      updated.name,
    );
    return updated;
  }

  /**
   * Renders an in-progress edit (name/style) on the live entity without persisting anything.
   * Used by the placemark editor for live preview; Save later calls updatePlacemark/
   * savePlacemarkEdits to actually persist, Close/unmount calls this again with the
   * original name/style to discard the preview.
   */
  previewPlacemark(
    id: string,
    geometry: PlacemarkGeometry,
    name: string,
    styleEdits: PlacemarkStyleEdits,
  ): void {
    const style = createStyle({ ...styleEdits, fillOpacity: FILL_OPACITY });
    this.entityFactory.updateEntity({ entityId: id }, geometry, style, name);
  }

  /** Creates a solid-colored (outlined + filled) style for a placemark - used by markers. */
  async setPlacemarkColor(id: string, colorHex: string): Promise<Placemark> {
    return this.savePlacemarkEdits(id, {
      name: (await this.requirePlacemark(id)).name,
      style: {
        outlineEnabled: true,
        outlineColor: colorHex,
        outlineWidth: 2,
        filled: true,
        fillColor: colorHex,
      },
    });
  }

  /**
   * Combines a field edit (name/description) and an optional style change into
   * a single repository write. Doing these as separate updatePlacemark calls
   * (each its own read-modify-write) is a lost-update race - whichever
   * finishes last silently clobbers the other's change.
   */
  async savePlacemarkEdits(
    id: string,
    edits: { name: string; description?: string; style?: PlacemarkStyleEdits },
  ): Promise<Placemark> {
    const styleId = edits.style ? (await this.createStyleFromEdits(edits.style)).id : undefined;
    return this.updatePlacemark(id, {
      name: edits.name,
      description: edits.description,
      ...(styleId ? { styleId } : {}),
    });
  }

  private async requirePlacemark(id: string): Promise<Placemark> {
    const placemark = await this.repository.getPlacemark(id);
    if (!placemark) throw new Error(`Placemark not found: ${id}`);
    return placemark;
  }

  private async createStyleFromEdits(edits: PlacemarkStyleEdits) {
    const style = createStyle({
      outlineEnabled: edits.outlineEnabled,
      outlineColor: edits.outlineColor,
      outlineWidth: edits.outlineWidth,
      filled: edits.filled,
      fillColor: edits.fillColor,
      fillOpacity: FILL_OPACITY,
    });
    await this.repository.upsertStyle(style);
    return style;
  }

  async deletePlacemark(id: string): Promise<void> {
    await this.repository.deletePlacemark(id);
    this.entityFactory.removeEntity({ entityId: id });
  }
}
