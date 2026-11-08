//! IndexedDB storage adapter (browser).
//!
//! Matches the Dexie schema the web app used: database `terra-globe`, stores
//! `folders` / `placemarks` / `styles` / `screenOverlays`, inline key path `id`.
//! Values are the same camelCase JSON objects, so an existing browser profile
//! keeps its data after the rewrite.

#![cfg(target_arch = "wasm32")]

use indexed_db_futures::database::Database;
use indexed_db_futures::prelude::*;
use indexed_db_futures::transaction::TransactionMode;
use indexed_db_futures::typed_array::Uint8Array;
use serde::Serialize;
use serde::de::DeserializeOwned;
use terra_core::domain::folder::{Folder, FolderPatch, NewFolder, create_folder};
use terra_core::domain::geometry::PlacemarkGeometry;
use terra_core::domain::placemark::{NewPlacemark, Placemark, PlacemarkPatch, create_placemark};
use terra_core::domain::screen_overlay::{NewScreenOverlay, ScreenOverlay, create_screen_overlay};
use terra_core::domain::style::Style;
use terra_core::now_iso8601;
use terra_storage::{ImportBatch, PlacesRepository, StorageError};

const DB_NAME: &str = "terra-globe";
const STORE_FOLDERS: &str = "folders";
const STORE_PLACEMARKS: &str = "placemarks";
const STORE_STYLES: &str = "styles";
const STORE_OVERLAYS: &str = "screenOverlays";
/// Out-of-line keys: the normalised path; values are `Uint8Array`s.
const STORE_ASSETS: &str = "assets";

fn backend(error: impl std::fmt::Display) -> StorageError {
    StorageError::Backend(format!("indexeddb: {error}"))
}

fn not_found(kind: &'static str, id: &str) -> StorageError {
    StorageError::NotFound {
        kind,
        id: id.to_string(),
    }
}

#[derive(Debug)]
pub struct IndexedDbPlacesRepository {
    db: Database,
}

impl IndexedDbPlacesRepository {
    /// Opens (and upgrades) the database. Version 3 adds the `assets` store.
    pub async fn open() -> Result<Self, StorageError> {
        let db = Database::open(DB_NAME)
            .with_version(3u8)
            .with_on_upgrade_needed_fut(|event, db| async move {
                if event.old_version() < 1.0 {
                    for store in [STORE_FOLDERS, STORE_PLACEMARKS, STORE_STYLES] {
                        db.create_object_store(store)
                            .with_key_path("id".into())
                            .build()?;
                    }
                }
                if event.old_version() < 2.0 {
                    db.create_object_store(STORE_OVERLAYS)
                        .with_key_path("id".into())
                        .build()?;
                }
                if event.old_version() < 3.0 {
                    db.create_object_store(STORE_ASSETS).build()?;
                }
                Ok(())
            })
            .await
            .map_err(backend)?;
        Ok(Self { db })
    }

    async fn all<T: DeserializeOwned>(&self, store: &str) -> Result<Vec<T>, StorageError> {
        let transaction = self
            .db
            .transaction(store)
            .with_mode(TransactionMode::Readonly)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(store).map_err(backend)?;
        let values = object_store
            .get_all()
            .serde()
            .map_err(backend)?
            .await
            .map_err(backend)?
            .collect::<Result<Vec<T>, _>>()
            .map_err(backend)?;
        Ok(values)
    }

    async fn get_one<T: DeserializeOwned>(
        &self,
        store: &str,
        id: &str,
    ) -> Result<Option<T>, StorageError> {
        let transaction = self
            .db
            .transaction(store)
            .with_mode(TransactionMode::Readonly)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(store).map_err(backend)?;
        let value: Option<T> = object_store
            .get(id)
            .serde()
            .map_err(backend)?
            .await
            .map_err(backend)?;
        Ok(value)
    }

    async fn put<T: Serialize>(&self, store: &str, value: &T) -> Result<(), StorageError> {
        let transaction = self
            .db
            .transaction(store)
            .with_mode(TransactionMode::Readwrite)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(store).map_err(backend)?;
        object_store
            .put(value)
            .serde()
            .map_err(backend)?
            .await
            .map_err(backend)?;
        transaction.commit().await.map_err(backend)?;
        Ok(())
    }

    async fn put_many<T: Serialize>(&self, store: &str, values: &[T]) -> Result<(), StorageError> {
        if values.is_empty() {
            return Ok(());
        }
        let transaction = self
            .db
            .transaction(store)
            .with_mode(TransactionMode::Readwrite)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(store).map_err(backend)?;
        for value in values {
            object_store
                .put(value)
                .serde()
                .map_err(backend)?
                .await
                .map_err(backend)?;
        }
        transaction.commit().await.map_err(backend)?;
        Ok(())
    }

    async fn delete(&self, store: &str, id: &str) -> Result<(), StorageError> {
        let transaction = self
            .db
            .transaction(store)
            .with_mode(TransactionMode::Readwrite)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(store).map_err(backend)?;
        object_store.delete(id).await.map_err(backend)?;
        transaction.commit().await.map_err(backend)?;
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl PlacesRepository for IndexedDbPlacesRepository {
    async fn get_folder(&self, id: &str) -> Result<Option<Folder>, StorageError> {
        self.get_one(STORE_FOLDERS, id).await
    }

    async fn list_folders(&self, parent_id: Option<&str>) -> Result<Vec<Folder>, StorageError> {
        let mut folders: Vec<Folder> = self
            .all::<Folder>(STORE_FOLDERS)
            .await?
            .into_iter()
            .filter(|folder| folder.parent_id.as_deref() == parent_id)
            .collect();
        folders.sort_by_key(|folder| folder.order);
        Ok(folders)
    }

    async fn create_folder(&self, input: NewFolder) -> Result<Folder, StorageError> {
        let folder = create_folder(input)?;
        self.put(STORE_FOLDERS, &folder).await?;
        Ok(folder)
    }

    async fn update_folder(&self, id: &str, patch: FolderPatch) -> Result<Folder, StorageError> {
        let mut folder = self
            .get_folder(id)
            .await?
            .ok_or_else(|| not_found("Folder", id))?;
        patch.apply(&mut folder);
        folder.updated_at = now_iso8601();
        self.put(STORE_FOLDERS, &folder).await?;
        Ok(folder)
    }

    async fn delete_folder(&self, id: &str, recursive: bool) -> Result<(), StorageError> {
        if self.get_folder(id).await?.is_none() {
            return Err(not_found("Folder", id));
        }
        let folders = self.all::<Folder>(STORE_FOLDERS).await?;
        let mut subtree = vec![id.to_string()];
        let mut index = 0;
        while index < subtree.len() {
            let parent = subtree[index].clone();
            index += 1;
            subtree.extend(
                folders
                    .iter()
                    .filter(|folder| folder.parent_id.as_deref() == Some(parent.as_str()))
                    .map(|folder| folder.id.clone()),
            );
        }

        let placemarks = self.all::<Placemark>(STORE_PLACEMARKS).await?;
        let overlays = self.all::<ScreenOverlay>(STORE_OVERLAYS).await?;
        let has_children = subtree.len() > 1
            || placemarks
                .iter()
                .any(|placemark| placemark.folder_id.as_deref() == Some(id));
        if has_children && !recursive {
            return Err(StorageError::FolderNotEmpty(id.to_string()));
        }

        for placemark in &placemarks {
            if placemark
                .folder_id
                .as_ref()
                .is_some_and(|folder| subtree.contains(folder))
            {
                self.delete(STORE_PLACEMARKS, &placemark.id).await?;
            }
        }
        for overlay in &overlays {
            if overlay
                .folder_id
                .as_ref()
                .is_some_and(|folder| subtree.contains(folder))
            {
                self.delete(STORE_OVERLAYS, &overlay.id).await?;
            }
        }
        for folder_id in &subtree {
            self.delete(STORE_FOLDERS, folder_id).await?;
        }
        Ok(())
    }

    async fn get_placemark(&self, id: &str) -> Result<Option<Placemark>, StorageError> {
        self.get_one(STORE_PLACEMARKS, id).await
    }

    async fn list_placemarks(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<Placemark>, StorageError> {
        let mut placemarks: Vec<Placemark> = self
            .all::<Placemark>(STORE_PLACEMARKS)
            .await?
            .into_iter()
            .filter(|placemark| placemark.folder_id.as_deref() == folder_id)
            .collect();
        placemarks.sort_by_key(|placemark| placemark.order);
        Ok(placemarks)
    }

    async fn create_placemark(
        &self,
        input: NewPlacemark,
        geometry: PlacemarkGeometry,
    ) -> Result<Placemark, StorageError> {
        let placemark = create_placemark(input, geometry)?;
        self.put(STORE_PLACEMARKS, &placemark).await?;
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
        self.put(STORE_PLACEMARKS, &placemark).await?;
        Ok(placemark)
    }

    async fn delete_placemark(&self, id: &str) -> Result<(), StorageError> {
        if self.get_placemark(id).await?.is_none() {
            return Err(not_found("Placemark", id));
        }
        self.delete(STORE_PLACEMARKS, id).await
    }

    async fn get_style(&self, id: &str) -> Result<Option<Style>, StorageError> {
        self.get_one(STORE_STYLES, id).await
    }

    async fn list_styles(&self) -> Result<Vec<Style>, StorageError> {
        let mut styles = self.all::<Style>(STORE_STYLES).await?;
        styles.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(styles)
    }

    async fn upsert_style(&self, style: Style) -> Result<Style, StorageError> {
        self.put(STORE_STYLES, &style).await?;
        Ok(style)
    }

    async fn delete_style(&self, id: &str) -> Result<(), StorageError> {
        self.delete(STORE_STYLES, id).await
    }

    async fn list_screen_overlays(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<ScreenOverlay>, StorageError> {
        let mut overlays: Vec<ScreenOverlay> = self
            .all::<ScreenOverlay>(STORE_OVERLAYS)
            .await?
            .into_iter()
            .filter(|overlay| overlay.folder_id.as_deref() == folder_id)
            .collect();
        overlays.sort_by_key(|overlay| overlay.order);
        Ok(overlays)
    }

    async fn create_screen_overlay(
        &self,
        overlay: NewScreenOverlay,
    ) -> Result<ScreenOverlay, StorageError> {
        let overlay = create_screen_overlay(overlay)?;
        self.put(STORE_OVERLAYS, &overlay).await?;
        Ok(overlay)
    }

    async fn delete_screen_overlay(&self, id: &str) -> Result<(), StorageError> {
        self.delete(STORE_OVERLAYS, id).await
    }

    async fn import_batch(&self, batch: ImportBatch) -> Result<(), StorageError> {
        self.put_many(STORE_FOLDERS, &batch.folders).await?;
        self.put_many(STORE_PLACEMARKS, &batch.placemarks).await?;
        self.put_many(STORE_STYLES, &batch.styles).await?;
        self.put_many(STORE_OVERLAYS, &batch.screen_overlays)
            .await?;
        Ok(())
    }

    async fn put_asset(&self, path: &str, bytes: Vec<u8>) -> Result<(), StorageError> {
        let transaction = self
            .db
            .transaction(STORE_ASSETS)
            .with_mode(TransactionMode::Readwrite)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(STORE_ASSETS).map_err(backend)?;
        object_store
            .put(Uint8Array::from(bytes))
            .with_key(path.to_string())
            .await
            .map_err(backend)?;
        transaction.commit().await.map_err(backend)?;
        Ok(())
    }

    async fn get_asset(&self, path: &str) -> Result<Option<Vec<u8>>, StorageError> {
        let transaction = self
            .db
            .transaction(STORE_ASSETS)
            .with_mode(TransactionMode::Readonly)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(STORE_ASSETS).map_err(backend)?;
        let value: Option<Uint8Array> = object_store
            .get(path.to_string())
            .primitive()
            .map_err(backend)?
            .await
            .map_err(backend)?;
        Ok(value.map(|array| array.as_ref().to_vec()))
    }

    async fn delete_asset(&self, path: &str) -> Result<(), StorageError> {
        self.delete(STORE_ASSETS, path).await
    }

    async fn list_assets(&self) -> Result<Vec<String>, StorageError> {
        let transaction = self
            .db
            .transaction(STORE_ASSETS)
            .with_mode(TransactionMode::Readonly)
            .build()
            .map_err(backend)?;
        let object_store = transaction.object_store(STORE_ASSETS).map_err(backend)?;
        let mut paths = object_store
            .get_all_keys::<String>()
            .await
            .map_err(backend)?
            .collect::<Result<Vec<String>, _>>()
            .map_err(backend)?;
        paths.sort();
        Ok(paths)
    }
}
