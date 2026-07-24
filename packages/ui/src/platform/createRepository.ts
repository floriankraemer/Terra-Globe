import type { PlacesRepository } from "@webglobe/core";
import { IndexedDbPlacesRepository } from "@webglobe/storage-indexeddb";
import { SqlitePlacesRepository, TauriSqlDriver } from "@webglobe/storage-sqlite";
import { isTauri } from "./isTauri.js";

/**
 * Composition root: the only place that branches on platform. Everything
 * downstream depends solely on the PlacesRepository port.
 */
export async function createRepository(): Promise<PlacesRepository> {
  if (isTauri()) {
    const { default: Database } = await import("@tauri-apps/plugin-sql");
    const db = await Database.load("sqlite:webglobe.db");
    return new SqlitePlacesRepository(new TauriSqlDriver(db));
  }
  return new IndexedDbPlacesRepository("webglobe");
}
