export const SCHEMA_STATEMENTS: readonly string[] = [
  `CREATE TABLE IF NOT EXISTS schema_migrations (
     version INTEGER PRIMARY KEY
   )`,
  `CREATE TABLE IF NOT EXISTS folders (
     id TEXT PRIMARY KEY,
     parent_id TEXT,
     data TEXT NOT NULL
   )`,
  `CREATE INDEX IF NOT EXISTS idx_folders_parent_id ON folders(parent_id)`,
  `CREATE TABLE IF NOT EXISTS placemarks (
     id TEXT PRIMARY KEY,
     folder_id TEXT,
     data TEXT NOT NULL
   )`,
  `CREATE INDEX IF NOT EXISTS idx_placemarks_folder_id ON placemarks(folder_id)`,
  `CREATE TABLE IF NOT EXISTS styles (
     id TEXT PRIMARY KEY,
     data TEXT NOT NULL
   )`,
  `CREATE TABLE IF NOT EXISTS screen_overlays (
     id TEXT PRIMARY KEY,
     folder_id TEXT,
     data TEXT NOT NULL
   )`,
  `CREATE INDEX IF NOT EXISTS idx_screen_overlays_folder_id ON screen_overlays(folder_id)`,
];

export const SCHEMA_VERSION = 2;
