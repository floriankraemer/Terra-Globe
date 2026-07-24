import { generateId } from "./ids.js";

export interface Folder {
  id: string;
  parentId: string | null;
  name: string;
  visibility: boolean;
  order: number;
  createdAt: string;
  updatedAt: string;
  description?: string;
  open?: boolean;
}

export interface NewFolder {
  name: string;
  parentId: string | null;
  order: number;
  visibility?: boolean;
  description?: string;
  open?: boolean;
}

export function createFolder(input: NewFolder): Folder {
  if (input.name.trim().length === 0) {
    throw new Error("name must not be empty");
  }
  const now = new Date().toISOString();
  return {
    id: generateId(),
    parentId: input.parentId,
    name: input.name,
    visibility: input.visibility ?? true,
    order: input.order,
    createdAt: now,
    updatedAt: now,
    description: input.description,
    open: input.open,
  };
}
