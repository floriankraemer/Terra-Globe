import { generateId } from "./ids.js";
import type { PlacemarkGeometry } from "./geometry.js";

export interface Placemark {
  id: string;
  folderId: string | null;
  name: string;
  description?: string;
  geometry: PlacemarkGeometry;
  styleId: string | null;
  visibility: boolean;
  order: number;
  createdAt: string;
  updatedAt: string;
}

export interface NewPlacemark {
  name: string;
  folderId: string | null;
  geometry: PlacemarkGeometry;
  description?: string;
  styleId?: string | null;
  visibility?: boolean;
  order?: number;
}

export function createPlacemark(input: NewPlacemark): Placemark {
  if (input.name.trim().length === 0) {
    throw new Error("name must not be empty");
  }
  const now = new Date().toISOString();
  return {
    id: generateId(),
    folderId: input.folderId,
    name: input.name,
    description: input.description,
    geometry: input.geometry,
    styleId: input.styleId ?? null,
    visibility: input.visibility ?? true,
    order: input.order ?? 0,
    createdAt: now,
    updatedAt: now,
  };
}
