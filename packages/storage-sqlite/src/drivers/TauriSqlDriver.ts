import type Database from "@tauri-apps/plugin-sql";
import type { SqlDriver } from "../SqlDriver.js";

/**
 * Real desktop driver backed by the Tauri SQL plugin. Only runs inside a Tauri
 * webview - not exercised by unit tests (see BetterSqlite3Driver for that),
 * only by the app's E2E suite.
 */
export class TauriSqlDriver implements SqlDriver {
  constructor(private readonly db: Database) {}

  async execute(sql: string, params: unknown[] = []): Promise<void> {
    await this.db.execute(sql, params);
  }

  async select<T>(sql: string, params: unknown[] = []): Promise<T[]> {
    return this.db.select<T[]>(sql, params);
  }
}
