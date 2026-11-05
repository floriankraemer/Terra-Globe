#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Native Terra Globe binary: composes the SQLite repository, JSON settings,
//! native dialogs and the OS keychain, then hands them to the app layer.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use terra_app::AppDeps;
use terra_app::platform::{FileSystem, JsonSettingsStore};
use terra_providers::{InMemorySecretStore, SecretError, SecretStore};
use terra_storage::PlacesRepository;
use terra_storage_sqlite::SqlitePlacesRepository;
use tracing_subscriber::EnvFilter;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("warn,terra=info")),
        )
        .with_target(false)
        .init();

    if std::env::args().any(|argument| argument == "--headless-smoke") {
        return smoke_test();
    }

    let data_dir = data_dir();
    std::fs::create_dir_all(&data_dir)?;
    let database_path = terra_storage_sqlite::database_path(&data_dir);

    tracing::info!(path = %database_path.display(), "starting Terra Globe");
    let repository: Arc<dyn PlacesRepository> =
        Arc::new(SqlitePlacesRepository::open(&database_path)?);

    terra_app::run_native(AppDeps {
        repository,
        settings: Arc::new(JsonSettingsStore::new(data_dir.join("settings.json"))),
        files: Arc::new(NativeFileSystem),
        secrets: Arc::new(KeyringSecrets::default()),
    })?;
    Ok(())
}

fn data_dir() -> PathBuf {
    directories::ProjectDirs::from("app", "terraglobe", "Terra Globe")
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

struct NativeFileSystem;

impl FileSystem for NativeFileSystem {
    fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
        std::fs::write(path, bytes).map_err(|error| format!("{}: {error}", path.display()))
    }

    fn pick_open(&self) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .add_filter("KML / KMZ", &["kml", "kmz"])
            .pick_file()
    }

    fn pick_save(&self, suggested_name: &str) -> Option<PathBuf> {
        rfd::FileDialog::new()
            .set_file_name(suggested_name)
            .add_filter("KML", &["kml"])
            .add_filter("KMZ", &["kmz"])
            .save_file()
    }
}

/// OS keychain, with an in-memory copy so the app still works on machines
/// without a session keyring (headless CI, some Linux setups).
#[derive(Default)]
struct KeyringSecrets {
    fallback: InMemorySecretStore,
}

const KEYRING_SERVICE: &str = "terra-globe";

impl SecretStore for KeyringSecrets {
    fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
        let stored = keyring::Entry::new(KEYRING_SERVICE, key)
            .ok()
            .and_then(|entry| entry.get_password().ok());
        if stored.is_some() {
            return Ok(stored);
        }
        self.fallback.get(key)
    }

    fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, key) {
            if entry.set_password(value).is_ok() {
                return Ok(());
            }
        }
        self.fallback.set(key, value)
    }

    fn remove(&self, key: &str) -> Result<(), SecretError> {
        if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, key) {
            let _ = entry.delete_credential();
        }
        self.fallback.remove(key)
    }
}

/// Renders a few frames without a window, for CI and milestone checks.
fn smoke_test() -> Result<(), Box<dyn std::error::Error>> {
    use terra_globe::imagery::TileProvider;
    use terra_globe::{Camera, Globe};
    use terra_providers::TileSource;

    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;

    let mut source = TileSource::open_street_map();
    if std::env::var("TERRA_SMOKE_ONLINE").as_deref() != Ok("1") {
        source.url = "http://127.0.0.1:9/{z}/{x}/{y}.png".to_string();
    }
    let mut globe = Globe::new(device, queue, TileProvider::new(source), (800, 500));
    globe.camera = Camera::default();

    let mut last = globe.render(0.0);
    for frame in 1..4 {
        last = globe.render(f64::from(frame) * 0.1);
    }
    let (width, height, pixels) = globe.renderer.read_pixels()?;
    let checksum: u64 = pixels.iter().map(|byte| u64::from(*byte)).sum();

    println!(
        "headless smoke: {width}x{height}, {} tiles drawn, level {}, {} textures, checksum {checksum}",
        last.tiles_drawn, last.deepest_level, last.textures_ready
    );

    if last.tiles_drawn == 0 {
        return Err("renderer drew no tiles".into());
    }
    if pixels.iter().all(|byte| *byte == 0) {
        return Err("frame is entirely black".into());
    }
    Ok(())
}
