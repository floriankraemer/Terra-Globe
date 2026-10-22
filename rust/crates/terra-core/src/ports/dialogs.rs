use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileFilter {
    pub name: String,
    pub extensions: Vec<String>,
}

impl FileFilter {
    pub fn kml_kmz() -> Self {
        Self {
            name: "KML/KMZ".into(),
            extensions: vec!["kml".into(), "kmz".into()],
        }
    }
}

/// Native file dialogs. Returns `None` when the user cancels.
pub trait FileDialogs: Send + Sync {
    fn pick_open(&self, filters: &[FileFilter]) -> Option<PathBuf>;
    fn pick_save(&self, filters: &[FileFilter], default_name: &str) -> Option<PathBuf>;
}
