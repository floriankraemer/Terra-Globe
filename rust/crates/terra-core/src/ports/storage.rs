use crate::domain::{
    Folder, FolderPatch, NewFolder, NewPlacemark, NewScreenOverlay, Placemark, PlacemarkPatch,
    ScreenOverlay, Style,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityKind {
    Folder,
    Placemark,
    Style,
}

impl std::fmt::Display for EntityKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Folder => "Folder",
            Self::Placemark => "Placemark",
            Self::Style => "Style",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RepositoryError {
    #[error("{kind} not found: {id}")]
    NotFound { kind: EntityKind, id: String },
    #[error("folder is not empty: {0}")]
    NotEmpty(String),
    #[error("{0}")]
    Validation(String),
    #[error("storage backend error: {0}")]
    Backend(String),
}

pub type RepositoryResult<T> = Result<T, RepositoryError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DeleteFolderOptions {
    pub recursive: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ImportBatchPayload {
    pub folders: Vec<Folder>,
    pub placemarks: Vec<Placemark>,
    pub styles: Vec<Style>,
    pub screen_overlays: Vec<ScreenOverlay>,
}

/// Storage port (dependency-inversion boundary). No adapter-specific types
/// (SQLite, IndexedDB, ...) may leak through this interface. Synchronous:
/// callers that need it off the UI thread run it on a worker.
pub trait PlacesRepository: Send {
    fn get_folder(&self, id: &str) -> RepositoryResult<Option<Folder>>;
    /// Children of `parent_id` (root when `None`), sorted by `order`.
    fn list_folders(&self, parent_id: Option<&str>) -> RepositoryResult<Vec<Folder>>;
    fn create_folder(&mut self, folder: NewFolder) -> RepositoryResult<Folder>;
    fn update_folder(&mut self, id: &str, patch: FolderPatch) -> RepositoryResult<Folder>;
    fn delete_folder(&mut self, id: &str, opts: DeleteFolderOptions) -> RepositoryResult<()>;

    fn get_placemark(&self, id: &str) -> RepositoryResult<Option<Placemark>>;
    /// Placemarks in `folder_id` (root when `None`), sorted by `order`.
    fn list_placemarks(&self, folder_id: Option<&str>) -> RepositoryResult<Vec<Placemark>>;
    fn create_placemark(&mut self, placemark: NewPlacemark) -> RepositoryResult<Placemark>;
    fn update_placemark(&mut self, id: &str, patch: PlacemarkPatch) -> RepositoryResult<Placemark>;
    fn delete_placemark(&mut self, id: &str) -> RepositoryResult<()>;

    fn get_style(&self, id: &str) -> RepositoryResult<Option<Style>>;
    fn upsert_style(&mut self, style: Style) -> RepositoryResult<Style>;

    fn list_screen_overlays(&self, folder_id: Option<&str>)
    -> RepositoryResult<Vec<ScreenOverlay>>;
    fn create_screen_overlay(
        &mut self,
        overlay: NewScreenOverlay,
    ) -> RepositoryResult<ScreenOverlay>;
    /// Deleting a missing overlay is not an error.
    fn delete_screen_overlay(&mut self, id: &str) -> RepositoryResult<()>;

    /// Stores everything exactly as given (ids and timestamps preserved), upserting by id.
    fn import_batch(&mut self, payload: ImportBatchPayload) -> RepositoryResult<()>;
}
