/**
 * Minimal SQL execution seam so SqlitePlacesRepository's query/migration logic
 * can run against a real @tauri-apps/plugin-sql connection in the desktop app,
 * and against better-sqlite3 in Node-based tests, without duplicating logic.
 */
export interface SqlDriver {
  execute(sql: string, params?: unknown[]): Promise<void>;
  select<T>(sql: string, params?: unknown[]): Promise<T[]>;
}
