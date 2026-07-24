import type Database from "better-sqlite3";
import type { SqlDriver } from "../SqlDriver.js";

/** Node-based SqlDriver used for testing schema/migration/query logic outside Tauri. */
export class BetterSqlite3Driver implements SqlDriver {
  constructor(private readonly db: Database.Database) {}

  async execute(sql: string, params: unknown[] = []): Promise<void> {
    this.db.prepare(sql).run(...params);
  }

  async select<T>(sql: string, params: unknown[] = []): Promise<T[]> {
    return this.db.prepare(sql).all(...params) as T[];
  }
}
