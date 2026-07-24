import Database from "better-sqlite3";
import { describe, expect, it } from "vitest";
import { BetterSqlite3Driver } from "../src/drivers/BetterSqlite3Driver.js";
import { migrate } from "../src/migrate.js";

describe("migrate", () => {
  it("creates the folders, placemarks and styles tables", async () => {
    const db = new Database(":memory:");
    const driver = new BetterSqlite3Driver(db);

    await migrate(driver);

    const tables = db
      .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
      .all()
      .map((r) => (r as { name: string }).name)
      .sort();

    expect(tables).toEqual([
      "folders",
      "placemarks",
      "schema_migrations",
      "screen_overlays",
      "styles",
    ]);
  });

  it("is idempotent - running twice does not error or duplicate schema_migrations rows", async () => {
    const db = new Database(":memory:");
    const driver = new BetterSqlite3Driver(db);

    await migrate(driver);
    await migrate(driver);

    const rows = db.prepare("SELECT version FROM schema_migrations").all();
    expect(rows).toEqual([{ version: 2 }]);
  });
});
