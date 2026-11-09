//! `PlacesRepository` port, a shared contract suite, and the in-memory adapter.
//!
//! Every storage backend must pass [`contract::run`]; it is the Rust port of
//! `packages/core/src/storage/repositoryContractTests.ts`.

use std::collections::HashMap;
use std::sync::RwLock;

use async_trait::async_trait;
use terra_core::domain::folder::{Folder, FolderPatch, NewFolder, create_folder};
use terra_core::domain::geometry::PlacemarkGeometry;
use terra_core::domain::placemark::{NewPlacemark, Placemark, PlacemarkPatch, create_placemark};
use terra_core::domain::screen_overlay::{NewScreenOverlay, ScreenOverlay, create_screen_overlay};
use terra_core::domain::style::Style;
use terra_core::{DomainError, now_iso8601};

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("{kind} not found: {id}")]
    NotFound { kind: &'static str, id: String },
    #[error("folder {0} is not empty; pass recursive = true to delete it")]
    FolderNotEmpty(String),
    #[error(transparent)]
    Invalid(#[from] DomainError),
    #[error("storage backend error: {0}")]
    Backend(String),
}

/// Batch payload produced by a KML import.
#[derive(Debug, Clone, Default)]
pub struct ImportBatch {
    pub folders: Vec<Folder>,
    pub placemarks: Vec<Placemark>,
    pub styles: Vec<Style>,
    pub screen_overlays: Vec<ScreenOverlay>,
}

#[async_trait(?Send)]
pub trait PlacesRepository {
    async fn get_folder(&self, id: &str) -> Result<Option<Folder>, StorageError>;
    async fn list_folders(&self, parent_id: Option<&str>) -> Result<Vec<Folder>, StorageError>;
    async fn create_folder(&self, input: NewFolder) -> Result<Folder, StorageError>;
    async fn update_folder(&self, id: &str, patch: FolderPatch) -> Result<Folder, StorageError>;
    async fn delete_folder(&self, id: &str, recursive: bool) -> Result<(), StorageError>;

    async fn get_placemark(&self, id: &str) -> Result<Option<Placemark>, StorageError>;
    async fn list_placemarks(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<Placemark>, StorageError>;
    async fn create_placemark(
        &self,
        input: NewPlacemark,
        geometry: PlacemarkGeometry,
    ) -> Result<Placemark, StorageError>;
    async fn update_placemark(
        &self,
        id: &str,
        patch: PlacemarkPatch,
    ) -> Result<Placemark, StorageError>;
    async fn delete_placemark(&self, id: &str) -> Result<(), StorageError>;

    async fn get_style(&self, id: &str) -> Result<Option<Style>, StorageError>;
    async fn list_styles(&self) -> Result<Vec<Style>, StorageError>;
    async fn upsert_style(&self, style: Style) -> Result<Style, StorageError>;
    async fn delete_style(&self, id: &str) -> Result<(), StorageError>;

    async fn list_screen_overlays(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<ScreenOverlay>, StorageError>;
    async fn create_screen_overlay(
        &self,
        overlay: NewScreenOverlay,
    ) -> Result<ScreenOverlay, StorageError>;
    async fn delete_screen_overlay(&self, id: &str) -> Result<(), StorageError>;

    async fn import_batch(&self, batch: ImportBatch) -> Result<(), StorageError>;

    /// Binary assets (overlay images) keyed by a document-relative path such as
    /// `images/map-1a2b.png`. Callers pass paths already normalised (no leading
    /// `./` or `/`); adapters store them verbatim. Empty blobs are allowed.
    async fn put_asset(&self, path: &str, bytes: Vec<u8>) -> Result<(), StorageError>;
    async fn get_asset(&self, path: &str) -> Result<Option<Vec<u8>>, StorageError>;
    /// Deleting a missing asset is a no-op.
    async fn delete_asset(&self, path: &str) -> Result<(), StorageError>;
    /// Sorted ascending.
    async fn list_assets(&self) -> Result<Vec<String>, StorageError>;
}

#[derive(Debug, Default)]
struct MemoryState {
    folders: HashMap<String, Folder>,
    placemarks: HashMap<String, Placemark>,
    styles: HashMap<String, Style>,
    overlays: HashMap<String, ScreenOverlay>,
    assets: HashMap<String, Vec<u8>>,
}

/// Test double and the reference behaviour for the contract suite.
#[derive(Debug, Default)]
pub struct InMemoryPlacesRepository {
    state: RwLock<MemoryState>,
}

impl InMemoryPlacesRepository {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> std::sync::RwLockReadGuard<'_, MemoryState> {
        self.state.read().expect("storage lock poisoned")
    }

    fn write(&self) -> std::sync::RwLockWriteGuard<'_, MemoryState> {
        self.state.write().expect("storage lock poisoned")
    }

    fn collect_folder_subtree(state: &MemoryState, id: &str, out: &mut Vec<String>) {
        out.push(id.to_string());
        for folder in state.folders.values() {
            if folder.parent_id.as_deref() == Some(id) {
                Self::collect_folder_subtree(state, &folder.id, out);
            }
        }
    }
}

#[async_trait(?Send)]
impl PlacesRepository for InMemoryPlacesRepository {
    async fn get_folder(&self, id: &str) -> Result<Option<Folder>, StorageError> {
        Ok(self.read().folders.get(id).cloned())
    }

    async fn list_folders(&self, parent_id: Option<&str>) -> Result<Vec<Folder>, StorageError> {
        let mut folders: Vec<Folder> = self
            .read()
            .folders
            .values()
            .filter(|folder| folder.parent_id.as_deref() == parent_id)
            .cloned()
            .collect();
        folders.sort_by_key(|folder| folder.order);
        Ok(folders)
    }

    async fn create_folder(&self, input: NewFolder) -> Result<Folder, StorageError> {
        let folder = create_folder(input)?;
        self.write()
            .folders
            .insert(folder.id.clone(), folder.clone());
        Ok(folder)
    }

    async fn update_folder(&self, id: &str, patch: FolderPatch) -> Result<Folder, StorageError> {
        let mut state = self.write();
        let folder = state
            .folders
            .get_mut(id)
            .ok_or_else(|| StorageError::NotFound {
                kind: "Folder",
                id: id.to_string(),
            })?;
        patch.apply(folder);
        folder.updated_at = now_iso8601();
        Ok(folder.clone())
    }

    async fn delete_folder(&self, id: &str, recursive: bool) -> Result<(), StorageError> {
        let mut state = self.write();
        if !state.folders.contains_key(id) {
            return Err(StorageError::NotFound {
                kind: "Folder",
                id: id.to_string(),
            });
        }
        let mut subtree = Vec::new();
        Self::collect_folder_subtree(&state, id, &mut subtree);

        let has_children = subtree.len() > 1
            || state
                .placemarks
                .values()
                .any(|p| p.folder_id.as_deref() == Some(id));
        if has_children && !recursive {
            return Err(StorageError::FolderNotEmpty(id.to_string()));
        }

        let doomed: std::collections::HashSet<&String> = subtree.iter().collect();
        state
            .placemarks
            .retain(|_, p| !p.folder_id.as_ref().is_some_and(|f| doomed.contains(f)));
        state
            .overlays
            .retain(|_, o| !o.folder_id.as_ref().is_some_and(|f| doomed.contains(f)));
        for folder_id in subtree {
            state.folders.remove(&folder_id);
        }
        Ok(())
    }

    async fn get_placemark(&self, id: &str) -> Result<Option<Placemark>, StorageError> {
        Ok(self.read().placemarks.get(id).cloned())
    }

    async fn list_placemarks(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<Placemark>, StorageError> {
        let mut placemarks: Vec<Placemark> = self
            .read()
            .placemarks
            .values()
            .filter(|p| p.folder_id.as_deref() == folder_id)
            .cloned()
            .collect();
        placemarks.sort_by_key(|p| p.order);
        Ok(placemarks)
    }

    async fn create_placemark(
        &self,
        input: NewPlacemark,
        geometry: PlacemarkGeometry,
    ) -> Result<Placemark, StorageError> {
        let placemark = create_placemark(input, geometry)?;
        self.write()
            .placemarks
            .insert(placemark.id.clone(), placemark.clone());
        Ok(placemark)
    }

    async fn update_placemark(
        &self,
        id: &str,
        patch: PlacemarkPatch,
    ) -> Result<Placemark, StorageError> {
        let mut state = self.write();
        let placemark = state
            .placemarks
            .get_mut(id)
            .ok_or_else(|| StorageError::NotFound {
                kind: "Placemark",
                id: id.to_string(),
            })?;
        patch.apply(placemark);
        placemark.updated_at = now_iso8601();
        Ok(placemark.clone())
    }

    async fn delete_placemark(&self, id: &str) -> Result<(), StorageError> {
        let removed = self.write().placemarks.remove(id);
        if removed.is_none() {
            return Err(StorageError::NotFound {
                kind: "Placemark",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    async fn get_style(&self, id: &str) -> Result<Option<Style>, StorageError> {
        Ok(self.read().styles.get(id).cloned())
    }

    async fn list_styles(&self) -> Result<Vec<Style>, StorageError> {
        let mut styles: Vec<Style> = self.read().styles.values().cloned().collect();
        styles.sort_by(|left, right| left.id.cmp(&right.id));
        Ok(styles)
    }

    async fn upsert_style(&self, style: Style) -> Result<Style, StorageError> {
        self.write().styles.insert(style.id.clone(), style.clone());
        Ok(style)
    }

    async fn delete_style(&self, id: &str) -> Result<(), StorageError> {
        self.write().styles.remove(id);
        Ok(())
    }

    async fn list_screen_overlays(
        &self,
        folder_id: Option<&str>,
    ) -> Result<Vec<ScreenOverlay>, StorageError> {
        let mut overlays: Vec<ScreenOverlay> = self
            .read()
            .overlays
            .values()
            .filter(|o| o.folder_id.as_deref() == folder_id)
            .cloned()
            .collect();
        overlays.sort_by_key(|o| o.order);
        Ok(overlays)
    }

    async fn create_screen_overlay(
        &self,
        overlay: NewScreenOverlay,
    ) -> Result<ScreenOverlay, StorageError> {
        let overlay = create_screen_overlay(overlay)?;
        self.write()
            .overlays
            .insert(overlay.id.clone(), overlay.clone());
        Ok(overlay)
    }

    async fn delete_screen_overlay(&self, id: &str) -> Result<(), StorageError> {
        let removed = self.write().overlays.remove(id);
        if removed.is_none() {
            return Err(StorageError::NotFound {
                kind: "ScreenOverlay",
                id: id.to_string(),
            });
        }
        Ok(())
    }

    async fn import_batch(&self, batch: ImportBatch) -> Result<(), StorageError> {
        let mut state = self.write();
        for folder in batch.folders {
            state.folders.insert(folder.id.clone(), folder);
        }
        for placemark in batch.placemarks {
            state.placemarks.insert(placemark.id.clone(), placemark);
        }
        for style in batch.styles {
            state.styles.insert(style.id.clone(), style);
        }
        for overlay in batch.screen_overlays {
            state.overlays.insert(overlay.id.clone(), overlay);
        }
        Ok(())
    }

    async fn put_asset(&self, path: &str, bytes: Vec<u8>) -> Result<(), StorageError> {
        self.write().assets.insert(path.to_string(), bytes);
        Ok(())
    }

    async fn get_asset(&self, path: &str) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(self.read().assets.get(path).cloned())
    }

    async fn delete_asset(&self, path: &str) -> Result<(), StorageError> {
        self.write().assets.remove(path);
        Ok(())
    }

    async fn list_assets(&self) -> Result<Vec<String>, StorageError> {
        let mut paths: Vec<String> = self.read().assets.keys().cloned().collect();
        paths.sort();
        Ok(paths)
    }
}

/// Reads the whole library: the export path, the undo/redo snapshot source and
/// the CLI all need a consistent "everything" view.
pub async fn collect_all(repo: &dyn PlacesRepository) -> Result<ImportBatch, StorageError> {
    let mut batch = ImportBatch {
        styles: repo.list_styles().await?,
        ..Default::default()
    };

    let mut queue: Vec<Option<String>> = vec![None];
    while let Some(parent) = queue.pop() {
        let folders = repo.list_folders(parent.as_deref()).await?;
        for folder in folders {
            batch
                .placemarks
                .extend(repo.list_placemarks(Some(&folder.id)).await?);
            batch
                .screen_overlays
                .extend(repo.list_screen_overlays(Some(&folder.id)).await?);
            queue.push(Some(folder.id.clone()));
            batch.folders.push(folder);
        }
        if parent.is_none() {
            batch.placemarks.extend(repo.list_placemarks(None).await?);
            batch
                .screen_overlays
                .extend(repo.list_screen_overlays(None).await?);
        }
    }
    Ok(batch)
}

/// Behaviour every adapter must reproduce. Called from each adapter's tests.
pub mod contract {
    use super::*;
    use terra_core::domain::geometry::GeoPoint;
    use terra_core::domain::style::{NewStyle, create_style};
    use terra_core::{NewScreenOverlay, Patch};

    fn point(lon: f64, lat: f64) -> PlacemarkGeometry {
        PlacemarkGeometry::point(GeoPoint::new(lon, lat).expect("valid point")).expect("valid")
    }

    pub async fn run(repo: &dyn PlacesRepository) {
        folder_and_placemark_lifecycle(repo).await;
        recursive_folder_delete(repo).await;
        update_semantics(repo).await;
        styles_and_overlays(repo).await;
        batch_import(repo).await;
        assets(repo).await;
    }

    async fn assets(repo: &dyn PlacesRepository) {
        assert_eq!(repo.get_asset("images/missing.png").await.unwrap(), None);
        repo.delete_asset("images/missing.png")
            .await
            .expect("deleting a missing asset is a no-op");

        repo.put_asset("images/a.png", vec![1, 2, 3]).await.unwrap();
        repo.put_asset("files/b.jpg", vec![9]).await.unwrap();
        assert_eq!(
            repo.get_asset("images/a.png").await.unwrap(),
            Some(vec![1, 2, 3])
        );

        repo.put_asset("images/a.png", vec![4, 5]).await.unwrap();
        assert_eq!(
            repo.get_asset("images/a.png").await.unwrap(),
            Some(vec![4, 5])
        );

        assert_eq!(
            repo.list_assets().await.unwrap(),
            vec!["files/b.jpg".to_string(), "images/a.png".to_string()]
        );

        repo.put_asset("images/empty.bin", Vec::new())
            .await
            .unwrap();
        assert_eq!(
            repo.get_asset("images/empty.bin").await.unwrap(),
            Some(Vec::new())
        );

        let big: Vec<u8> = (0..5 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        repo.put_asset("images/big.bin", big.clone()).await.unwrap();
        assert!(repo.get_asset("images/big.bin").await.unwrap() == Some(big));

        repo.delete_asset("images/a.png").await.unwrap();
        assert_eq!(repo.get_asset("images/a.png").await.unwrap(), None);
        assert_eq!(
            repo.get_asset("files/b.jpg").await.unwrap(),
            Some(vec![9]),
            "deleting one asset leaves the others"
        );
        for path in repo.list_assets().await.unwrap() {
            repo.delete_asset(&path).await.unwrap();
        }
        assert!(repo.list_assets().await.unwrap().is_empty());
    }

    async fn folder_and_placemark_lifecycle(repo: &dyn PlacesRepository) {
        let root = repo
            .create_folder(NewFolder {
                name: "Root".into(),
                parent_id: None,
                order: 0,
                ..Default::default()
            })
            .await
            .expect("create folder");

        let child = repo
            .create_folder(NewFolder {
                name: "Child".into(),
                parent_id: Some(root.id.clone()),
                order: 0,
                ..Default::default()
            })
            .await
            .expect("create child folder");

        assert_eq!(repo.list_folders(None).await.unwrap().len(), 1);
        assert_eq!(repo.list_folders(Some(&root.id)).await.unwrap().len(), 1);

        let placemark = repo
            .create_placemark(
                NewPlacemark {
                    name: "Berlin".into(),
                    folder_id: Some(child.id.clone()),
                    ..Default::default()
                },
                point(13.405, 52.52),
            )
            .await
            .expect("create placemark");

        assert_eq!(
            repo.list_placemarks(Some(&child.id)).await.unwrap().len(),
            1
        );
        assert!(repo.get_placemark(&placemark.id).await.unwrap().is_some());

        repo.delete_placemark(&placemark.id).await.unwrap();
        assert!(repo.get_placemark(&placemark.id).await.unwrap().is_none());
        assert!(matches!(
            repo.delete_placemark(&placemark.id).await,
            Err(StorageError::NotFound { .. })
        ));
    }

    async fn recursive_folder_delete(repo: &dyn PlacesRepository) {
        let parent = repo
            .create_folder(NewFolder {
                name: "Parent".into(),
                parent_id: None,
                order: 0,
                ..Default::default()
            })
            .await
            .unwrap();
        let child = repo
            .create_folder(NewFolder {
                name: "Child".into(),
                parent_id: Some(parent.id.clone()),
                order: 0,
                ..Default::default()
            })
            .await
            .unwrap();
        repo.create_placemark(
            NewPlacemark {
                name: "Nested".into(),
                folder_id: Some(child.id.clone()),
                ..Default::default()
            },
            point(1.0, 2.0),
        )
        .await
        .unwrap();

        assert!(matches!(
            repo.delete_folder(&parent.id, false).await,
            Err(StorageError::FolderNotEmpty(_))
        ));
        repo.delete_folder(&parent.id, true).await.unwrap();
        assert!(repo.get_folder(&parent.id).await.unwrap().is_none());
        assert!(repo.get_folder(&child.id).await.unwrap().is_none());
        assert!(
            repo.list_placemarks(Some(&child.id))
                .await
                .unwrap()
                .is_empty()
        );
    }

    async fn update_semantics(repo: &dyn PlacesRepository) {
        let folder = repo
            .create_folder(NewFolder {
                name: "Old".into(),
                parent_id: None,
                order: 3,
                description: Some("keep me".into()),
                ..Default::default()
            })
            .await
            .unwrap();

        let renamed = repo
            .update_folder(
                &folder.id,
                FolderPatch {
                    name: Patch::Set("New".into()),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(renamed.name, "New");
        assert_eq!(renamed.order, 3);
        assert_eq!(renamed.description.as_deref(), Some("keep me"));

        let cleared = repo
            .update_folder(
                &folder.id,
                FolderPatch {
                    description: Patch::Set(None),
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        assert_eq!(cleared.description, None);
        assert_eq!(cleared.name, "New");
    }

    async fn styles_and_overlays(repo: &dyn PlacesRepository) {
        let style = create_style(NewStyle {
            outline_color: "#3366cc".into(),
            outline_width: 2.0,
            fill_color: "#cc6633".into(),
            fill_opacity: 0.6,
            outline_enabled: None,
            filled: None,
            icon_url: None,
            icon_scale: None,
            marker_icon: None,
            label_color: None,
        })
        .unwrap();
        repo.upsert_style(style.clone()).await.unwrap();
        assert_eq!(
            repo.get_style(&style.id).await.unwrap(),
            Some(style.clone())
        );
        assert_eq!(repo.list_styles().await.unwrap(), vec![style]);

        let overlay = repo
            .create_screen_overlay(NewScreenOverlay {
                name: "Legend".into(),
                image_url: "legend.png".into(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(repo.list_screen_overlays(None).await.unwrap().len(), 1);
        repo.delete_screen_overlay(&overlay.id).await.unwrap();
        assert!(repo.list_screen_overlays(None).await.unwrap().is_empty());
    }

    async fn batch_import(repo: &dyn PlacesRepository) {
        let root = create_folder(NewFolder {
            name: "Imported".into(),
            parent_id: None,
            order: 0,
            ..Default::default()
        })
        .unwrap();
        let placemark = create_placemark(
            NewPlacemark {
                name: "Imported pin".into(),
                folder_id: Some(root.id.clone()),
                ..Default::default()
            },
            point(10.0, 20.0),
        )
        .unwrap();

        repo.import_batch(ImportBatch {
            folders: vec![root.clone()],
            placemarks: vec![placemark.clone()],
            ..Default::default()
        })
        .await
        .unwrap();

        assert!(repo.get_folder(&root.id).await.unwrap().is_some());
        assert!(repo.get_placemark(&placemark.id).await.unwrap().is_some());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn in_memory_adapter_passes_the_contract() {
        let repo = super::InMemoryPlacesRepository::new();
        pollster::block_on(super::contract::run(&repo));
    }
}
