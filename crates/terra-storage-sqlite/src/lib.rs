//! SQLite storage adapter (desktop).
//!
//! Rows are `(id, parent_id|folder_id, data)` where `data` is the same JSON the
//! TypeScript app writes, so an existing database opens unchanged and a
//! database written here stays readable by the old app.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;
use rusqlite::OptionalExtension;
use rusqlite::params;
use serde::Serialize;
use serde::de::DeserializeOwned;
use terra_core::domain::folder::{Folder, FolderPatch, NewFolder, create_folder};
use terra_core::domain::geometry::PlacemarkGeometry;
use terra_core::domain::placemark::{NewPlacemark, Placemark, PlacemarkPatch, create_placemark};
use terra_core::domain::screen_overlay::{NewScreenOverlay, ScreenOverlay, create_screen_overlay};
use terra_core::domain::style::Style;
use terra_core::now_iso8601;
use terra_storage::{ImportBatch, PlacesRepository, StorageError};

pub const DATABASE_FILE_NAME: &str = "terra-globe.db";
pub const SCHEMA_VERSION: i64 = 2;

/// Identical to `packages/storage-sqlite/src/schema.ts`.
pub const SCHEMA_STATEMENTS: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS schema_migrations (version INTEGER PRIMARY KEY)",
    "CREATE TABLE IF NOT EXISTS folders (id TEXT PRIMARY KEY, parent_id TEXT, data TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS idx_folders_parent_id ON folders(parent_id)",
    "CREATE TABLE IF NOT EXISTS placemarks (id TEXT PRIMARY KEY, folder_id TEXT, data TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS idx_placemarks_folder_id ON placemarks(folder_id)",
    "CREATE TABLE IF NOT EXISTS styles (id TEXT PRIMARY KEY, data TEXT NOT NULL)",
    "CREATE TABLE IF NOT EXISTS screen_overlays (id TEXT PRIMARY KEY, folder_id TEXT, data TEXT NOT NULL)",
    "CREATE INDEX IF NOT EXISTS idx_screen_overlays_folder_id ON screen_overlays(folder_id)",
];

#[derive(Debug, thiserror::Error)]
pub enum SqliteError {
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Creates the schema if needed and records the schema version.
pub fn migrate(connection: &mut rusqlite::Connection) -> Result<(), SqliteError> {
    let transaction = connection.transaction()?;
    for statement in SCHEMA_STATEMENTS {
        transaction.execute_batch(statement)?;
    }
    transaction.execute(
        "INSERT OR IGNORE INTO schema_migrations (version) VALUES (?1)",
        [SCHEMA_VERSION],
    )?;
    transaction.commit()?;
    Ok(())
}

/// Resolves the database path inside a platform data directory, matching the
/// file name the Tauri build already uses.
pub fn database_path(data_dir: &Path) -> PathBuf {
    data_dir.join(DATABASE_FILE_NAME)
}

#[derive(Debug, Clone, Copy)]
enum Table {
    Folders,
    Placemarks,
    Styles,
}

impl Table {
    fn name(self) -> &'static str {
        match self {
            Self::Folders => "folders",
            Self::Placemarks => "placemarks",
            Self::Styles => "styles",
        }
    }
}

fn backend(error: impl std::fmt::Display) -> StorageError {
    StorageError::Backend(error.to_string())
}

fn encode<T: Serialize>(value: &T) -> Result<String, StorageError> {
    serde_json::to_string(value).map_err(backend)
}

fn decode<T: DeserializeOwned>(data: &str) -> Result<T, StorageError> {
    serde_json::from_str(data).map_err(backend)
}

fn not_found(kind: &'static str, id: &str) -> StorageError {
    StorageError::NotFound {
        kind,
        id: id.to_string(),
    }
}

pub struct SqlitePlacesRepository {
    connection: Mutex<rusqlite::Connection>,
}

impl std::fmt::Debug for SqlitePlacesRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SqlitePlacesRepository")
            .finish_non_exhaustive()
    }
}

impl SqlitePlacesRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqliteError> {
        let mut connection = rusqlite::Connection::open(path)?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    pub fn open_in_memory() -> Result<Self, SqliteError> {
        let mut connection = rusqlite::Connection::open_in_memory()?;
        migrate(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn conn(&self) -> MutexGuard<'_, rusqlite::Connection> {
        self.connection
            .lock()
            .expect("sqlite connection lock poisoned")
    }

    fn get<T: DeserializeOwned>(&self, table: Table, id: &str) -> Result<Option<T>, StorageError> {
        let connection = self.conn();
        let mut statement = connection
            .prepare(&format!("SELECT data FROM {} WHERE id = ?1", table.name()))
            .map_err(backend)?;
        let mut rows = statement.query([id]).map_err(backend)?;
        match rows.next().map_err(backend)? {
            Some(row) => {
                let data: String = row.get(0).map_err(backend)?;
                decode(&data).map(Some)
            }
            None => Ok(None),
        }
    }

    fn collect_folder_subtree(
        connection: &rusqlite::Connection,
        root: &str,
    ) -> Result<Vec<String>, StorageError> {
        let mut subtree = vec![root.to_string()];
        let mut index = 0;
        while index < subtree.len() {
            let parent = subtree[index].clone();
            index += 1;
            let mut statement = connection
                .prepare("SELECT id FROM folders WHERE parent_id = ?1")
                .map_err(backend)?;
            let children: Vec<String> = statement
                .query_map([parent], |row| row.get::<_, String>(0))
                .map_err(backend)?
                .collect::<Result<_, _>>()
                .map_err(backend)?;
            subtree.extend(children);
        }
        Ok(subtree)
    }
}

#[async_trait(?Send)]
impl PlacesRepository for SqlitePlacesRepository {
    async fn get_folder(&self, id: &str) -> Result<Option<Folder>, StorageError> {
        self.get(Table::Folders, id)
    }

    async fn list_folders(&self, parent_id: Option<&str>) -> Result<Vec<Folder>, StorageError> {
        let connection = self.conn();
        let mut statement = connection
            .prepare("SELECT data FROM folders WHERE parent_id IS NULL")
            .map_err(backend)?;
        let mut statement_with_filter = connection
            .prepare("SELECT data FROM folders WHERE parent_id = ?1")
            .map_err(backend)?;
        let mut rows = match parent_id {
            Some(id) => statement_with_filter.query([id]).map_err(backend)?,
            None => statement.query([]).map_err(backend)?,
        };
        let mut folders = Vec::new();
        while let Some(row) = rows.next().map_err(backend)? {
            let data: String = row.get(0).map_err(backend)?;
            folders.push(decode::<Folder>(&data)?);
        }
        folders.sort_by_key(|folder| folder.order);
        Ok(folders)
    }

    async fn create_folder(&self, input: NewFolder) -> Result<Folder, StorageError> {
        let folder = create_folder(input)?;
        let data = encode(&folder)?;
        self.conn()
            .execute(
                "INSERT INTO folders (id, parent_id, data) VALUES (?1, ?2, ?3)",
                params![folder.id, folder.parent_id, data],
            )
            .map_err(backend)?;
        Ok(folder)
    }

    async fn update_folder(&self, id: &str, patch: FolderPatch) -> Result<Folder, StorageError> {
        let mut folder = self
            .get_folder(id)
            .await?
            .ok_or_else(|| not_found("Folder", id))?;
        patch.apply(&mut folder);
        folder.updated_at = now_iso8601();
        let data = encode(&folder)?;
        self.conn()
            .execute(
                "UPDATE folders SET parent_id = ?1, data = ?2 WHERE id = ?3",
                params![folder.parent_id, data, id],
            )
            .map_err(backend)?;
        Ok(folder)
    }

    async fn delete_folder(&self, id: &str, recursive: bool) -> Result<(), StorageError> {
        // Existence is checked on its own lock scope: `std::sync::Mutex` is not
        // re-entrant, so calling another locking helper while holding the guard
        // would deadlock.
        let exists = {
            let connection = self.conn();
            connection
                .query_row("SELECT 1 FROM folders WHERE id = ?1", [id], |_| Ok(()))
                .optional()
                .map_err(backend)?
                .is_some()
        };
        if !exists {
            return Err(not_found("Folder", id));
        }

        let mut connection = self.conn();
        let subtree = Self::collect_folder_subtree(&connection, id)?;

        let mut has_children = subtree.len() > 1;
        if !has_children {
            let placemark_count: i64 = connection
                .query_row(
                    "SELECT COUNT(*) FROM placemarks WHERE folder_id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .map_err(backend)?;
            has_children = placemark_count > 0;
        }
        if has_children && !recursive {
            return Err(StorageError::FolderNotEmpty(id.to_string()));
        }

        let transaction = connection.transaction().map_err(backend)?;
        for folder_id in &subtree {
            transaction
                .execute("DELETE FROM placemarks WHERE folder_id = ?1", [folder_id])
                .map_err(backend)?;
            transaction
                .execute(
                    "DELETE FROM screen_overlays WHERE folder_id = ?1",
                    [folder_id],
                )
                .map_err(backend)?;
        }
        for folder_id in &subtree {
            transaction
                .execute("DELETE FROM folders WHERE id = ?1", [folder_id])
                .map_err(backend)?;
        }
        transaction.commit().map_err(backend)?;
        Ok(())
    }

    async fn get_placemark(&self, id: &str) -> Result<Option<Placemark>, StorageError> {
        self.get(Table::Placemarks, id)
    }

    async fn list_placemarks(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<Placemark>, StorageError> {
        let connection = self.conn();
        let mut statement = connection
            .prepare("SELECT data FROM placemarks WHERE folder_id IS NULL")
            .map_err(backend)?;
        let mut statement_with_filter = connection
            .prepare("SELECT data FROM placemarks WHERE folder_id = ?1")
            .map_err(backend)?;
        let mut rows = match folder_id {
            Some(id) => statement_with_filter.query([id]).map_err(backend)?,
            None => statement.query([]).map_err(backend)?,
        };
        let mut placemarks = Vec::new();
        while let Some(row) = rows.next().map_err(backend)? {
            let data: String = row.get(0).map_err(backend)?;
            placemarks.push(decode::<Placemark>(&data)?);
        }
        placemarks.sort_by_key(|placemark| placemark.order);
        Ok(placemarks)
    }

    async fn create_placemark(
        &self,
        input: NewPlacemark,
        geometry: PlacemarkGeometry,
    ) -> Result<Placemark, StorageError> {
        let placemark = create_placemark(input, geometry)?;
        let data = encode(&placemark)?;
        self.conn()
            .execute(
                "INSERT INTO placemarks (id, folder_id, data) VALUES (?1, ?2, ?3)",
                params![placemark.id, placemark.folder_id, data],
            )
            .map_err(backend)?;
        Ok(placemark)
    }

    async fn update_placemark(
        &self,
        id: &str,
        patch: PlacemarkPatch,
    ) -> Result<Placemark, StorageError> {
        let mut placemark = self
            .get_placemark(id)
            .await?
            .ok_or_else(|| not_found("Placemark", id))?;
        patch.apply(&mut placemark);
        placemark.updated_at = now_iso8601();
        let data = encode(&placemark)?;
        self.conn()
            .execute(
                "UPDATE placemarks SET folder_id = ?1, data = ?2 WHERE id = ?3",
                params![placemark.folder_id, data, id],
            )
            .map_err(backend)?;
        Ok(placemark)
    }

    async fn delete_placemark(&self, id: &str) -> Result<(), StorageError> {
        let affected = self
            .conn()
            .execute("DELETE FROM placemarks WHERE id = ?1", [id])
            .map_err(backend)?;
        if affected == 0 {
            return Err(not_found("Placemark", id));
        }
        Ok(())
    }

    async fn get_style(&self, id: &str) -> Result<Option<Style>, StorageError> {
        self.get(Table::Styles, id)
    }

    async fn list_styles(&self) -> Result<Vec<Style>, StorageError> {
        let connection = self.conn();
        let mut statement = connection
            .prepare("SELECT data FROM styles ORDER BY id")
            .map_err(backend)?;
        let mut rows = statement.query([]).map_err(backend)?;
        let mut styles = Vec::new();
        while let Some(row) = rows.next().map_err(backend)? {
            let data: String = row.get(0).map_err(backend)?;
            styles.push(decode::<Style>(&data)?);
        }
        Ok(styles)
    }

    async fn upsert_style(&self, style: Style) -> Result<Style, StorageError> {
        let data = encode(&style)?;
        self.conn()
            .execute(
                "INSERT OR REPLACE INTO styles (id, data) VALUES (?1, ?2)",
                params![style.id, data],
            )
            .map_err(backend)?;
        Ok(style)
    }

    async fn delete_style(&self, id: &str) -> Result<(), StorageError> {
        self.conn()
            .execute("DELETE FROM styles WHERE id = ?1", [id])
            .map_err(backend)?;
        Ok(())
    }

    async fn list_screen_overlays(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<ScreenOverlay>, StorageError> {
        let connection = self.conn();
        let mut statement = connection
            .prepare("SELECT data FROM screen_overlays WHERE folder_id IS NULL")
            .map_err(backend)?;
        let mut statement_with_filter = connection
            .prepare("SELECT data FROM screen_overlays WHERE folder_id = ?1")
            .map_err(backend)?;
        let mut rows = match folder_id {
            Some(id) => statement_with_filter.query([id]).map_err(backend)?,
            None => statement.query([]).map_err(backend)?,
        };
        let mut overlays = Vec::new();
        while let Some(row) = rows.next().map_err(backend)? {
            let data: String = row.get(0).map_err(backend)?;
            overlays.push(decode::<ScreenOverlay>(&data)?);
        }
        overlays.sort_by_key(|overlay| overlay.order);
        Ok(overlays)
    }

    async fn create_screen_overlay(
        &self,
        overlay: NewScreenOverlay,
    ) -> Result<ScreenOverlay, StorageError> {
        let overlay = create_screen_overlay(overlay)?;
        let data = encode(&overlay)?;
        self.conn()
            .execute(
                "INSERT INTO screen_overlays (id, folder_id, data) VALUES (?1, ?2, ?3)",
                params![overlay.id, overlay.folder_id, data],
            )
            .map_err(backend)?;
        Ok(overlay)
    }

    async fn delete_screen_overlay(&self, id: &str) -> Result<(), StorageError> {
        let affected = self
            .conn()
            .execute("DELETE FROM screen_overlays WHERE id = ?1", [id])
            .map_err(backend)?;
        if affected == 0 {
            return Err(not_found("ScreenOverlay", id));
        }
        Ok(())
    }

    async fn import_batch(&self, batch: ImportBatch) -> Result<(), StorageError> {
        let mut connection = self.conn();
        let transaction = connection.transaction().map_err(backend)?;
        for folder in &batch.folders {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO folders (id, parent_id, data) VALUES (?1, ?2, ?3)",
                    params![folder.id, folder.parent_id, encode(folder)?],
                )
                .map_err(backend)?;
        }
        for placemark in &batch.placemarks {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO placemarks (id, folder_id, data) VALUES (?1, ?2, ?3)",
                    params![placemark.id, placemark.folder_id, encode(placemark)?],
                )
                .map_err(backend)?;
        }
        for style in &batch.styles {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO styles (id, data) VALUES (?1, ?2)",
                    params![style.id, encode(style)?],
                )
                .map_err(backend)?;
        }
        for overlay in &batch.screen_overlays {
            transaction
                .execute(
                    "INSERT OR REPLACE INTO screen_overlays (id, folder_id, data) VALUES (?1, ?2, ?3)",
                    params![overlay.id, overlay.folder_id, encode(overlay)?],
                )
                .map_err(backend)?;
        }
        transaction.commit().map_err(backend)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_core::domain::geometry::GeoPoint;
    use terra_core::{NewPlacemark, PlacemarkGeometry};

    #[test]
    fn migrate_creates_the_expected_tables_and_version() {
        let connection = rusqlite::Connection::open_in_memory().expect("in-memory db");
        let mut connection = connection;
        migrate(&mut connection).expect("migrate");

        let tables: Vec<String> = connection
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .expect("prepare")
            .query_map([], |row| row.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");

        for expected in ["folders", "placemarks", "screen_overlays", "styles"] {
            assert!(
                tables.iter().any(|table| table == expected),
                "missing table {expected} in {tables:?}"
            );
        }

        let version: i64 = connection
            .query_row("SELECT version FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("version row");
        assert_eq!(version, SCHEMA_VERSION);
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut connection = rusqlite::Connection::open_in_memory().expect("in-memory db");
        migrate(&mut connection).expect("first migrate");
        migrate(&mut connection).expect("second migrate");
        let count: i64 = connection
            .query_row("SELECT COUNT(*) FROM schema_migrations", [], |row| {
                row.get(0)
            })
            .expect("count");
        assert_eq!(count, 1);
    }

    #[test]
    fn database_file_name_matches_the_tauri_app() {
        assert_eq!(
            database_path(Path::new("/tmp/data")),
            PathBuf::from("/tmp/data/terra-globe.db")
        );
    }

    #[test]
    fn sqlite_adapter_passes_the_repository_contract() {
        let repo = SqlitePlacesRepository::open_in_memory().expect("in-memory repo");
        pollster::block_on(terra_storage::contract::run(&repo));
    }

    #[test]
    fn rows_written_by_the_typescript_app_are_readable() {
        let repo = SqlitePlacesRepository::open_in_memory().expect("in-memory repo");
        // Shaped exactly like the web app's rows: camelCase keys, null folderId,
        // optional fields omitted rather than null, ISO timestamps.
        let json = r#"{
            "id": "11111111-1111-4111-8111-111111111111",
            "folderId": null,
            "name": "Berlin",
            "geometry": {"type": "Point", "coordinates": {"lon": 13.405, "lat": 52.52}},
            "styleId": null,
            "visibility": true,
            "order": 0,
            "createdAt": "2024-01-01T00:00:00.000Z",
            "updatedAt": "2024-01-01T00:00:00.000Z"
        }"#;
        repo.conn()
            .execute(
                "INSERT INTO placemarks (id, folder_id, data) VALUES (?1, NULL, ?2)",
                params!["11111111-1111-4111-8111-111111111111", json],
            )
            .expect("insert legacy row");

        let loaded = pollster::block_on(repo.get_placemark("11111111-1111-4111-8111-111111111111"))
            .expect("read")
            .expect("placemark exists");
        assert_eq!(loaded.name, "Berlin");
        assert_eq!(loaded.folder_id, None);
        assert_eq!(loaded.created_at, "2024-01-01T00:00:00.000Z");
    }

    #[test]
    fn rows_written_here_keep_the_typescript_json_shape() {
        let repo = SqlitePlacesRepository::open_in_memory().expect("in-memory repo");
        let geometry = PlacemarkGeometry::point(GeoPoint::new(13.405, 52.52).expect("valid"))
            .expect("geometry");
        let placemark = pollster::block_on(repo.create_placemark(
            NewPlacemark {
                name: "Berlin".into(),
                ..Default::default()
            },
            geometry,
        ))
        .expect("create");

        let data: String = repo
            .conn()
            .query_row(
                "SELECT data FROM placemarks WHERE id = ?1",
                [&placemark.id],
                |row| row.get(0),
            )
            .expect("row");
        let value: serde_json::Value = serde_json::from_str(&data).expect("json");
        let object = value.as_object().expect("object");
        assert!(object.contains_key("folderId"));
        assert!(object.contains_key("createdAt"));
        assert!(!object.contains_key("description"));
        assert_eq!(value["visibility"], serde_json::json!(true));
    }
}
