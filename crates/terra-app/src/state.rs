//! Application state: the in-memory library, mutations and undo.
//!
//! Every mutation goes through one command surface so undo/redo has a single
//! place to snapshot, and so the UI can never silently diverge from the
//! repository.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use terra_core::domain::folder::{Folder, FolderPatch, NewFolder};
use terra_core::domain::geometry::{GeoPoint, PlacemarkGeometry};
use terra_core::domain::placemark::{NewPlacemark, Placemark, PlacemarkPatch};
use terra_core::domain::screen_overlay::ScreenOverlay;
use terra_core::domain::style::{NewStyle, Style, create_style};
use terra_core::{Patch, is_folder_visible};
use terra_kml::ParseKmlResult;
use terra_storage::{ImportBatch, PlacesRepository};

pub const MAX_HISTORY: usize = 20;

#[derive(Debug, Clone, Default)]
pub struct Library {
    pub folders: Vec<Folder>,
    pub placemarks: Vec<Placemark>,
    pub styles: BTreeMap<String, Style>,
    pub overlays: Vec<ScreenOverlay>,
    pub revision: u64,
}

impl Library {
    pub fn from_batch(batch: ImportBatch) -> Self {
        Self {
            folders: batch.folders,
            placemarks: batch.placemarks,
            styles: batch
                .styles
                .into_iter()
                .map(|style| (style.id.clone(), style))
                .collect(),
            overlays: batch.screen_overlays,
            revision: 1,
        }
    }

    pub fn to_batch(&self) -> ImportBatch {
        ImportBatch {
            folders: self.folders.clone(),
            placemarks: self.placemarks.clone(),
            styles: self.styles.values().cloned().collect(),
            screen_overlays: self.overlays.clone(),
        }
    }

    pub fn touch(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub fn folder(&self, id: &str) -> Option<&Folder> {
        self.folders.iter().find(|folder| folder.id == id)
    }

    pub fn placemark(&self, id: &str) -> Option<&Placemark> {
        self.placemarks.iter().find(|placemark| placemark.id == id)
    }

    pub fn child_folders(&self, parent: Option<&str>) -> Vec<&Folder> {
        let mut folders: Vec<&Folder> = self
            .folders
            .iter()
            .filter(|folder| folder.parent_id.as_deref() == parent)
            .collect();
        folders.sort_by_key(|folder| folder.order);
        folders
    }

    pub fn placemarks_in(&self, folder: Option<&str>) -> Vec<&Placemark> {
        let mut placemarks: Vec<&Placemark> = self
            .placemarks
            .iter()
            .filter(|placemark| placemark.folder_id.as_deref() == folder)
            .collect();
        placemarks.sort_by_key(|placemark| placemark.order);
        placemarks
    }

    /// Style used for rendering, falling back to the default marker/line style.
    pub fn style_for(&self, placemark: &Placemark) -> Style {
        placemark
            .style_id
            .as_deref()
            .and_then(|id| self.styles.get(id))
            .cloned()
            .unwrap_or_else(|| Style {
                id: String::new(),
                marker_icon: Some(terra_core::DEFAULT_MARKER_ICON),
                ..Style::default()
            })
    }

    /// A placemark is visible only when it and every ancestor folder is.
    pub fn is_placemark_visible(&self, placemark: &Placemark) -> bool {
        placemark.visibility && is_folder_visible(&self.folders, placemark.folder_id.as_deref())
    }

    pub fn is_folder_visible(&self, folder: &Folder) -> bool {
        is_folder_visible(&self.folders, Some(&folder.id))
    }

    pub fn visible_placemarks(&self) -> Vec<&Placemark> {
        self.placemarks
            .iter()
            .filter(|placemark| self.is_placemark_visible(placemark))
            .collect()
    }

    /// Unique folder name for the "new folder" action.
    pub fn default_folder_name(&self, parent: Option<&str>) -> String {
        let existing: BTreeSet<&str> = self
            .folders
            .iter()
            .filter(|folder| folder.parent_id.as_deref() == parent)
            .map(|folder| folder.name.as_str())
            .collect();
        let mut index = 1;
        loop {
            let candidate = if index == 1 {
                "New folder".to_string()
            } else {
                format!("New folder {index}")
            };
            if !existing.contains(candidate.as_str()) {
                return candidate;
            }
            index += 1;
        }
    }

    pub fn default_placemark_name(&self, geometry: &PlacemarkGeometry) -> String {
        let base = match geometry.type_name() {
            "Point" => "Marker",
            "LineString" => "Line",
            "Polygon" => "Polygon",
            "Circle" => "Circle",
            "Rectangle" => "Rectangle",
            other => other,
        };
        format!("{base} {}", self.placemarks.len() + 1)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error(transparent)]
    Storage(#[from] terra_storage::StorageError),
    #[error(transparent)]
    Domain(#[from] terra_core::DomainError),
    #[error(transparent)]
    Kml(#[from] terra_kml::KmlError),
    #[error("{0}")]
    Message(String),
}

pub type AppResult<T> = Result<T, AppError>;

/// Owns the repository and the in-memory projection of it.
pub struct StateStore {
    repository: Arc<dyn PlacesRepository>,
    library: Library,
    undo: Vec<Library>,
    redo: Vec<Library>,
}

impl StateStore {
    pub fn load(repository: Arc<dyn PlacesRepository>) -> AppResult<Self> {
        let batch = pollster::block_on(terra_storage::collect_all(repository.as_ref()))?;
        Ok(Self {
            repository,
            library: Library::from_batch(batch),
            undo: Vec::new(),
            redo: Vec::new(),
        })
    }

    pub fn library(&self) -> &Library {
        &self.library
    }

    pub fn repository(&self) -> &Arc<dyn PlacesRepository> {
        &self.repository
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Runs a mutation with an undo snapshot. A failed mutation restores the
    /// snapshot so the UI never shows a change the repository rejected.
    pub fn mutate<T>(&mut self, action: impl FnOnce(&mut Self) -> AppResult<T>) -> AppResult<T> {
        let snapshot = self.library.clone();
        match action(self) {
            Ok(value) => {
                push_capped(&mut self.undo, snapshot);
                self.redo.clear();
                self.library.touch();
                Ok(value)
            }
            Err(error) => {
                self.library = snapshot;
                Err(error)
            }
        }
    }

    pub fn undo(&mut self) -> AppResult<()> {
        let Some(snapshot) = self.undo.pop() else {
            return Ok(());
        };
        let current = self.library.clone();
        self.restore(snapshot)?;
        push_capped(&mut self.redo, current);
        Ok(())
    }

    pub fn redo(&mut self) -> AppResult<()> {
        let Some(snapshot) = self.redo.pop() else {
            return Ok(());
        };
        let current = self.library.clone();
        self.restore(snapshot)?;
        push_capped(&mut self.undo, current);
        Ok(())
    }

    /// Writes a snapshot back to the repository, deleting anything it no longer
    /// contains. Used by undo/redo.
    fn restore(&mut self, snapshot: Library) -> AppResult<()> {
        let current = pollster::block_on(terra_storage::collect_all(self.repository.as_ref()))?;
        let keep_placemarks: BTreeSet<&str> =
            snapshot.placemarks.iter().map(|p| p.id.as_str()).collect();
        let keep_folders: BTreeSet<&str> = snapshot.folders.iter().map(|f| f.id.as_str()).collect();
        let keep_overlays: BTreeSet<&str> =
            snapshot.overlays.iter().map(|o| o.id.as_str()).collect();
        let keep_styles: BTreeSet<&str> = snapshot.styles.keys().map(String::as_str).collect();

        for placemark in &current.placemarks {
            if !keep_placemarks.contains(placemark.id.as_str()) {
                pollster::block_on(self.repository.delete_placemark(&placemark.id))?;
            }
        }
        for overlay in &current.screen_overlays {
            if !keep_overlays.contains(overlay.id.as_str()) {
                pollster::block_on(self.repository.delete_screen_overlay(&overlay.id))?;
            }
        }
        for style in &current.styles {
            if !keep_styles.contains(style.id.as_str()) {
                pollster::block_on(self.repository.delete_style(&style.id))?;
            }
        }
        // Children before parents so a recursive delete never takes a folder the
        // snapshot still needs.
        let mut missing_folders: Vec<&Folder> = current
            .folders
            .iter()
            .filter(|folder| !keep_folders.contains(folder.id.as_str()))
            .collect();
        missing_folders
            .sort_by_key(|folder| std::cmp::Reverse(folder_depth(&current.folders, &folder.id)));
        for folder in missing_folders {
            pollster::block_on(self.repository.delete_folder(&folder.id, true))?;
        }

        pollster::block_on(self.repository.import_batch(snapshot.to_batch()))?;
        self.library = snapshot;
        self.library.touch();
        Ok(())
    }

    pub fn create_folder(&mut self, name: &str, parent: Option<String>) -> AppResult<Folder> {
        self.mutate(|state| {
            let order = state.library.child_folders(parent.as_deref()).len() as i64;
            let input = NewFolder {
                name: name.to_string(),
                parent_id: parent,
                order,
                ..Default::default()
            };
            let folder = pollster::block_on(state.repository.create_folder(input))?;
            state.library.folders.push(folder.clone());
            Ok(folder)
        })
    }

    pub fn rename_folder(&mut self, id: &str, name: String) -> AppResult<()> {
        self.mutate(|state| {
            if name.trim().is_empty() {
                return Err(AppError::Domain(terra_core::DomainError::EmptyName));
            }
            let patch = FolderPatch {
                name: Patch::Set(name),
                ..Default::default()
            };
            let updated = pollster::block_on(state.repository.update_folder(id, patch))?;
            replace_folder(&mut state.library, updated);
            Ok(())
        })
    }

    pub fn set_folder_visibility(&mut self, id: &str, visible: bool) -> AppResult<()> {
        self.mutate(|state| {
            let patch = FolderPatch {
                visibility: Patch::Set(visible),
                ..Default::default()
            };
            let updated = pollster::block_on(state.repository.update_folder(id, patch))?;
            replace_folder(&mut state.library, updated);
            Ok(())
        })
    }

    pub fn set_folder_open(&mut self, id: &str, open: bool) -> AppResult<()> {
        self.mutate(|state| {
            let patch = FolderPatch {
                open: Patch::Set(Some(open)),
                ..Default::default()
            };
            let updated = pollster::block_on(state.repository.update_folder(id, patch))?;
            replace_folder(&mut state.library, updated);
            Ok(())
        })
    }

    pub fn move_folder(&mut self, id: &str, parent: Option<String>) -> AppResult<()> {
        self.mutate(|state| {
            if parent.as_deref() == Some(id)
                || is_descendant(&state.library.folders, id, parent.as_deref())
            {
                return Err(AppError::Message("a folder cannot move into itself".into()));
            }
            let order = state.library.child_folders(parent.as_deref()).len() as i64;
            let patch = FolderPatch {
                parent_id: Patch::Set(parent),
                order: Patch::Set(order),
                ..Default::default()
            };
            let updated = pollster::block_on(state.repository.update_folder(id, patch))?;
            replace_folder(&mut state.library, updated);
            Ok(())
        })
    }

    pub fn delete_folder(&mut self, id: &str) -> AppResult<()> {
        self.mutate(|state| {
            pollster::block_on(state.repository.delete_folder(id, true))?;
            let doomed = folder_subtree(&state.library.folders, id);
            state.library.placemarks.retain(|placemark| {
                !placemark
                    .folder_id
                    .as_ref()
                    .is_some_and(|f| doomed.contains(f))
            });
            state
                .library
                .folders
                .retain(|folder| !doomed.contains(&folder.id));
            state.library.overlays.retain(|overlay| {
                !overlay
                    .folder_id
                    .as_ref()
                    .is_some_and(|f| doomed.contains(f))
            });
            Ok(())
        })
    }

    pub fn create_placemark(
        &mut self,
        geometry: PlacemarkGeometry,
        folder: Option<String>,
    ) -> AppResult<Placemark> {
        self.mutate(|state| {
            let name = state.library.default_placemark_name(&geometry);
            let style = create_style(NewStyle {
                outline_color: "#3388ff".into(),
                outline_width: 2.0,
                fill_color: "#3388ff".into(),
                fill_opacity: 0.35,
                outline_enabled: None,
                filled: Some(!matches!(geometry, PlacemarkGeometry::Point { .. })),
                icon_url: None,
                icon_scale: None,
                marker_icon: None,
                label_color: None,
            })
            .map_err(AppError::Domain)?;
            let style = pollster::block_on(state.repository.upsert_style(style))?;
            let input = NewPlacemark {
                name,
                folder_id: folder,
                style_id: Some(style.id.clone()),
                ..Default::default()
            };
            let placemark = pollster::block_on(state.repository.create_placemark(input, geometry))?;
            state.library.styles.insert(style.id.clone(), style);
            state.library.placemarks.push(placemark.clone());
            Ok(placemark)
        })
    }

    pub fn update_placemark(&mut self, id: &str, patch: PlacemarkPatch) -> AppResult<()> {
        self.mutate(|state| {
            let updated = pollster::block_on(state.repository.update_placemark(id, patch))?;
            if let Some(existing) = state
                .library
                .placemarks
                .iter_mut()
                .find(|placemark| placemark.id == id)
            {
                *existing = updated;
            }
            Ok(())
        })
    }

    pub fn set_placemark_visibility(&mut self, id: &str, visible: bool) -> AppResult<()> {
        self.update_placemark(
            id,
            PlacemarkPatch {
                visibility: Patch::Set(visible),
                ..Default::default()
            },
        )
    }

    pub fn move_placemark(&mut self, id: &str, folder: Option<String>) -> AppResult<()> {
        self.mutate(|state| {
            let order = state.library.placemarks_in(folder.as_deref()).len() as i64;
            let patch = PlacemarkPatch {
                folder_id: Patch::Set(folder),
                order: Patch::Set(order),
                ..Default::default()
            };
            let updated = pollster::block_on(state.repository.update_placemark(id, patch))?;
            if let Some(existing) = state
                .library
                .placemarks
                .iter_mut()
                .find(|placemark| placemark.id == id)
            {
                *existing = updated;
            }
            Ok(())
        })
    }

    pub fn delete_placemark(&mut self, id: &str) -> AppResult<()> {
        self.mutate(|state| {
            pollster::block_on(state.repository.delete_placemark(id))?;
            state
                .library
                .placemarks
                .retain(|placemark| placemark.id != id);
            Ok(())
        })
    }

    pub fn update_style(&mut self, style: Style) -> AppResult<()> {
        self.mutate(|state| {
            let saved = pollster::block_on(state.repository.upsert_style(style))?;
            state.library.styles.insert(saved.id.clone(), saved);
            Ok(())
        })
    }

    /// Adds parsed KML to the library, preserving ids.
    pub fn import(&mut self, parsed: ParseKmlResult) -> AppResult<usize> {
        self.mutate(|state| {
            let count = parsed.placemarks.len();
            pollster::block_on(state.repository.import_batch(ImportBatch {
                folders: parsed.folders.clone(),
                placemarks: parsed.placemarks.clone(),
                styles: parsed.styles.clone(),
                screen_overlays: parsed.screen_overlays.clone(),
            }))?;
            for folder in parsed.folders {
                replace_folder(&mut state.library, folder);
            }
            for style in parsed.styles {
                state.library.styles.insert(style.id.clone(), style);
            }
            for placemark in parsed.placemarks {
                if let Some(existing) = state
                    .library
                    .placemarks
                    .iter_mut()
                    .find(|candidate| candidate.id == placemark.id)
                {
                    *existing = placemark;
                } else {
                    state.library.placemarks.push(placemark);
                }
            }
            for overlay in parsed.screen_overlays {
                state.library.overlays.push(overlay);
            }
            Ok(count)
        })
    }

    pub fn export_batch(&self) -> ImportBatch {
        self.library.to_batch()
    }

    /// Creates a placemark, or a folder plus a placemark, from a geocoding hit.
    pub fn add_geocoded_placemark(
        &mut self,
        label: String,
        point: GeoPoint,
    ) -> AppResult<Placemark> {
        let folder = self.create_folder("Search results", None)?;
        self.mutate(|state| {
            let geometry = PlacemarkGeometry::point(point).map_err(AppError::Domain)?;
            let style = create_style(NewStyle {
                outline_color: "#ff8a3d".into(),
                outline_width: 2.0,
                fill_color: "#ff8a3d".into(),
                fill_opacity: 0.4,
                outline_enabled: None,
                filled: None,
                icon_url: None,
                icon_scale: None,
                marker_icon: None,
                label_color: None,
            })
            .map_err(AppError::Domain)?;
            let style = pollster::block_on(state.repository.upsert_style(style))?;
            state.library.styles.insert(style.id.clone(), style.clone());
            let input = NewPlacemark {
                name: label,
                folder_id: Some(folder.id.clone()),
                style_id: Some(style.id),
                ..Default::default()
            };
            let placemark = pollster::block_on(state.repository.create_placemark(input, geometry))?;
            state.library.placemarks.push(placemark.clone());
            Ok(placemark)
        })
    }
}

fn push_capped(stack: &mut Vec<Library>, entry: Library) {
    stack.push(entry);
    if stack.len() > MAX_HISTORY {
        stack.remove(0);
    }
}

fn replace_folder(library: &mut Library, folder: Folder) {
    if let Some(existing) = library
        .folders
        .iter_mut()
        .find(|candidate| candidate.id == folder.id)
    {
        *existing = folder;
    } else {
        library.folders.push(folder);
    }
}

fn folder_depth(folders: &[Folder], id: &str) -> usize {
    let mut depth = 0;
    let mut current = folders.iter().find(|folder| folder.id == id);
    while let Some(folder) = current {
        depth += 1;
        current = folder
            .parent_id
            .as_deref()
            .and_then(|parent| folders.iter().find(|candidate| candidate.id == parent));
        if depth > folders.len() {
            break;
        }
    }
    depth
}

fn is_descendant(folders: &[Folder], ancestor: &str, candidate: Option<&str>) -> bool {
    let mut current = candidate;
    let mut guard = 0;
    while let Some(id) = current {
        if id == ancestor {
            return true;
        }
        guard += 1;
        if guard > folders.len() {
            break;
        }
        current = folders
            .iter()
            .find(|folder| folder.id == id)
            .and_then(|folder| folder.parent_id.as_deref());
    }
    false
}

fn folder_subtree(folders: &[Folder], root: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let mut queue = vec![root.to_string()];
    while let Some(id) = queue.pop() {
        out.insert(id.clone());
        for folder in folders {
            if folder.parent_id.as_deref() == Some(id.as_str()) {
                queue.push(folder.id.clone());
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use terra_storage::InMemoryPlacesRepository;

    fn store() -> StateStore {
        StateStore::load(Arc::new(InMemoryPlacesRepository::new())).expect("store")
    }

    #[test]
    fn mutations_are_persisted_and_undoable() {
        let mut store = store();
        let folder = store.create_folder("Trips", None).expect("folder");
        assert_eq!(store.library().folders.len(), 1);
        assert!(store.can_undo());

        store
            .rename_folder(&folder.id, "Holidays".into())
            .expect("rename");
        assert_eq!(store.library().folders[0].name, "Holidays");

        store.undo().expect("undo");
        assert_eq!(store.library().folders[0].name, "Trips");
        store.redo().expect("redo");
        assert_eq!(store.library().folders[0].name, "Holidays");
    }

    #[test]
    fn deleting_a_folder_removes_its_content_and_undo_restores_it() {
        let mut store = store();
        let folder = store.create_folder("Trips", None).expect("folder");
        let geometry =
            PlacemarkGeometry::point(GeoPoint::new(1.0, 2.0).expect("point")).expect("geometry");
        let placemark = store
            .create_placemark(geometry, Some(folder.id.clone()))
            .expect("placemark");
        assert_eq!(store.library().placemarks.len(), 1);

        store.delete_folder(&folder.id).expect("delete");
        assert!(store.library().folders.is_empty());
        assert!(store.library().placemarks.is_empty());

        store.undo().expect("undo");
        assert_eq!(store.library().folders.len(), 1);
        assert_eq!(store.library().placemarks.len(), 1);
        assert_eq!(store.library().placemarks[0].id, placemark.id);
    }

    #[test]
    fn visibility_is_inherited_from_folders() {
        let mut store = store();
        let folder = store.create_folder("Hidden", None).expect("folder");
        let geometry =
            PlacemarkGeometry::point(GeoPoint::new(1.0, 2.0).expect("point")).expect("geometry");
        store
            .create_placemark(geometry, Some(folder.id.clone()))
            .expect("placemark");
        assert_eq!(store.library().visible_placemarks().len(), 1);
        store
            .set_folder_visibility(&folder.id, false)
            .expect("hide");
        assert!(store.library().visible_placemarks().is_empty());
    }

    #[test]
    fn folders_cannot_move_into_their_own_descendant() {
        let mut store = store();
        let parent = store.create_folder("Parent", None).expect("folder");
        let child = store
            .create_folder("Child", Some(parent.id.clone()))
            .expect("folder");
        let error = store
            .move_folder(&parent.id, Some(child.id.clone()))
            .expect_err("must fail");
        assert!(error.to_string().contains("cannot move into itself"));
        assert!(store.library().folder(&parent.id).is_some());
    }

    #[test]
    fn imports_merge_instead_of_duplicating() {
        let mut store = store();
        let kml = r#"<kml><Document><Placemark id="p1"><name>A</name><Point><coordinates>1,2</coordinates></Point></Placemark></Document></kml>"#;
        let parsed = terra_kml::parse_kml(kml).expect("parse");
        store.import(parsed.clone()).expect("import");
        store.import(parsed).expect("import again");
        assert_eq!(store.library().placemarks.len(), 1);
    }

    #[test]
    fn history_is_capped() {
        let mut store = store();
        for index in 0..(MAX_HISTORY + 5) {
            store
                .create_folder(&format!("F{index}"), None)
                .expect("folder");
        }
        for _ in 0..(MAX_HISTORY + 10) {
            let _ = store.undo();
        }
        assert_eq!(store.library().folders.len(), 5);
    }
}
