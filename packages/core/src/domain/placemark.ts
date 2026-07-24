import { generateId } from "./ids.js";
import type { PlacemarkGeometry } from "./geometry.js";

export interface PlacemarkView {
  kind: "Camera" | "LookAt";
  /** Raw KML view params (longitude/latitude/altitude/heading/tilt/range/roll/altitudeMode), kept opaque. */
  params: Record<string, number | string>;
}

export interface PlacemarkRegion {
  /** Raw KML LatLonAltBox + Lod params, kept opaque - round-trip only, no LOD behavior. */
  raw: Record<string, number | string>;
}

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
  timeStamp?: string;
  timeSpanBegin?: string;
  timeSpanEnd?: string;
  snippet?: string;
  address?: string;
  phoneNumber?: string;
  view?: PlacemarkView;
  region?: PlacemarkRegion;
  /** Arbitrary KML Data/SimpleData/SchemaData, flattened name -> value. */
  extendedData?: Record<string, string>;
  /** Shared marker for placemarks that came from one KML MultiGeometry, so export can regroup them. */
  multiGeometryGroup?: string;
}

export interface NewPlacemark {
  name: string;
  folderId: string | null;
  geometry: PlacemarkGeometry;
  description?: string;
  styleId?: string | null;
  visibility?: boolean;
  order?: number;
  timeStamp?: string;
  timeSpanBegin?: string;
  timeSpanEnd?: string;
  snippet?: string;
  address?: string;
  phoneNumber?: string;
  view?: PlacemarkView;
  region?: PlacemarkRegion;
  extendedData?: Record<string, string>;
  multiGeometryGroup?: string;
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
    timeStamp: input.timeStamp,
    timeSpanBegin: input.timeSpanBegin,
    timeSpanEnd: input.timeSpanEnd,
    snippet: input.snippet,
    address: input.address,
    phoneNumber: input.phoneNumber,
    view: input.view,
    region: input.region,
    extendedData: input.extendedData,
    multiGeometryGroup: input.multiGeometryGroup,
  };
}
