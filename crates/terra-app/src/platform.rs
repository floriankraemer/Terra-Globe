//! Ports the shells implement: settings persistence, files and secret storage.
//!
//! Keeping them as traits means the whole UI can be exercised in tests without
//! touching a real config directory or opening a dialog.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use terra_core::Settings;
use terra_globe::CameraPose;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PersistedState {
    #[serde(default)]
    pub settings: Settings,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub camera: Option<CameraPose>,
}

pub trait SettingsStore: Send + Sync {
    fn load(&self) -> PersistedState;
    fn save(&self, state: &PersistedState);
}

#[derive(Debug, Default)]
pub struct InMemorySettingsStore {
    state: Mutex<PersistedState>,
}

impl SettingsStore for InMemorySettingsStore {
    fn load(&self) -> PersistedState {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or_default()
    }

    fn save(&self, state: &PersistedState) {
        if let Ok(mut slot) = self.state.lock() {
            *slot = state.clone();
        }
    }
}

/// JSON file in the platform config directory (desktop).
#[derive(Debug)]
pub struct JsonSettingsStore {
    path: PathBuf,
}

impl JsonSettingsStore {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl SettingsStore for JsonSettingsStore {
    fn load(&self) -> PersistedState {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    fn save(&self, state: &PersistedState) {
        if let Some(parent) = self.path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(state) {
            // Write-then-rename so a crash cannot truncate the settings file.
            let temporary = self.path.with_extension("json.tmp");
            if std::fs::write(&temporary, text).is_ok() {
                let _ = std::fs::rename(&temporary, &self.path);
            }
        }
    }
}

pub trait FileSystem: Send + Sync {
    fn read(&self, path: &Path) -> Result<Vec<u8>, String>;
    fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String>;
    /// Blocking native dialog; shells without dialogs return `None`.
    fn pick_open(&self) -> Option<PathBuf>;
    /// Blocking native dialog filtered to raster images for ground overlays.
    fn pick_open_image(&self) -> Option<PathBuf>;
    fn pick_save(&self, suggested_name: &str) -> Option<PathBuf>;
}

/// In-memory file system for tests.
#[derive(Debug, Default)]
pub struct MemoryFileSystem {
    files: Mutex<std::collections::BTreeMap<PathBuf, Vec<u8>>>,
    open: Mutex<Option<PathBuf>>,
    image: Mutex<Option<PathBuf>>,
    save: Mutex<Option<PathBuf>>,
}

impl MemoryFileSystem {
    pub fn with_file(path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> Self {
        let files = MemoryFileSystem::default();
        files
            .files
            .lock()
            .expect("files lock")
            .insert(path.into(), bytes.into());
        files
    }

    pub fn set_picked_open(&self, path: Option<PathBuf>) {
        if let Ok(mut slot) = self.open.lock() {
            *slot = path;
        }
    }

    pub fn set_picked_image(&self, path: Option<PathBuf>) {
        if let Ok(mut slot) = self.image.lock() {
            *slot = path;
        }
    }

    pub fn set_picked_save(&self, path: Option<PathBuf>) {
        if let Ok(mut slot) = self.save.lock() {
            *slot = path;
        }
    }

    pub fn read_back(&self, path: &Path) -> Option<Vec<u8>> {
        self.files.lock().ok()?.get(path).cloned()
    }
}

impl FileSystem for MemoryFileSystem {
    fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        self.files
            .lock()
            .map_err(|_| "files lock poisoned".to_string())?
            .get(path)
            .cloned()
            .ok_or_else(|| format!("no such file: {}", path.display()))
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        self.files
            .lock()
            .map_err(|_| "files lock poisoned".to_string())?
            .insert(path.to_path_buf(), bytes.to_vec());
        Ok(())
    }

    fn pick_open(&self) -> Option<PathBuf> {
        self.open.lock().ok().and_then(|slot| slot.clone())
    }

    fn pick_open_image(&self) -> Option<PathBuf> {
        self.image.lock().ok().and_then(|slot| slot.clone())
    }

    fn pick_save(&self, _suggested_name: &str) -> Option<PathBuf> {
        self.save.lock().ok().and_then(|slot| slot.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_through_the_json_store() {
        let dir = std::env::temp_dir().join(format!("terra-settings-{}", std::process::id()));
        let path = dir.join("settings.json");
        let store = JsonSettingsStore::new(path.clone());
        let mut state = PersistedState::default();
        state.settings.language = "de".to_string();
        store.save(&state);

        let loaded = store.load();
        assert_eq!(loaded.settings.language, "de");
        assert_eq!(loaded.settings.routing_preset, "osrm");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_settings_fall_back_to_defaults() {
        let dir = std::env::temp_dir().join(format!("terra-bad-settings-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("settings.json");
        std::fs::write(&path, "{ not json").expect("write");
        let store = JsonSettingsStore::new(path);
        assert_eq!(store.load().settings.language, "en");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn memory_file_system_reads_and_writes() {
        let files = MemoryFileSystem::with_file("/tmp/a.kml", b"<kml/>".to_vec());
        assert_eq!(
            files.read(Path::new("/tmp/a.kml")).expect("read"),
            b"<kml/>"
        );
        files.write(Path::new("/tmp/b.kml"), b"x").expect("write");
        assert_eq!(
            files.read_back(Path::new("/tmp/b.kml")),
            Some(b"x".to_vec())
        );
        assert!(files.read(Path::new("/tmp/missing")).is_err());
    }
}
