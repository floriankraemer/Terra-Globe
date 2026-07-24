import type { SqlDriver } from "./SqlDriver.js";
import { SCHEMA_STATEMENTS, SCHEMA_VERSION } from "./schema.js";

interface MigrationRow {
  version: number;
}

export async function migrate(driver: SqlDriver): Promise<void> {
  await driver.execute(
    "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY)",
  );
  const rows = await driver.select<MigrationRow>("SELECT version FROM schema_migrations");
  const applied = new Set(rows.map((r) => r.version));

  if (applied.has(SCHEMA_VERSION)) return;

  for (const statement of SCHEMA_STATEMENTS) {
    await driver.execute(statement);
  }
  await driver.execute("INSERT INTO schema_migrations (version) VALUES (?)", [SCHEMA_VERSION]);
}
