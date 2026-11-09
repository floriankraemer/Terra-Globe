//! Browser entry point for Terra Globe.
//!
//! The browser build is a supported target from M0 (it must keep compiling),
//! but it only becomes a shipped deliverable at M7. The desktop binary lives in
//! `terra-desktop`.

/// Starts the wasm application. On non-wasm targets this is a no-op kept so the
/// crate compiles everywhere (the desktop binary calls `terra_app::run_native`).
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .try_init();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|window| window.document())
            .expect("browser window with a document");
        let canvas = document
            .get_element_by_id("terra-canvas")
            .expect("index.html must contain <canvas id=\"terra-canvas\">");
        let canvas: web_sys::HtmlCanvasElement =
            wasm_bindgen::JsCast::dyn_into(canvas).expect("#terra-canvas must be a canvas element");

        let repository = match terra_storage_indexeddb::IndexedDbPlacesRepository::open().await {
            Ok(repository) => std::sync::Arc::new(repository)
                as std::sync::Arc<dyn terra_storage::PlacesRepository>,
            Err(error) => {
                tracing::error!("could not open IndexedDB: {error}");
                return;
            }
        };
        let deps = terra_app::AppDeps {
            repository,
            settings: std::sync::Arc::new(browser_adapters::BrowserSettingsStore),
            files: std::sync::Arc::new(browser_adapters::BrowserFiles),
            secrets: std::sync::Arc::new(browser_adapters::BrowserSecretStore),
            tile_cache: None,
        };

        eframe::WebRunner::new()
            .start(
                canvas,
                eframe::WebOptions::default(),
                Box::new(move |cc| {
                    terra_app::TerraApp::new(cc, deps)
                        .map(|app| Box::new(app) as Box<dyn eframe::App>)
                        .map_err(|error| {
                            Box::new(std::io::Error::other(error))
                                as Box<dyn std::error::Error + Send + Sync>
                        })
                }),
            )
            .await
            .expect("failed to start Terra Globe");
    });
}

/// Non-wasm builds use `terra-desktop`.
#[cfg(not(target_arch = "wasm32"))]
pub fn start() {}

#[cfg(target_arch = "wasm32")]
mod browser_adapters {
    use std::path::{Path, PathBuf};

    use terra_app::platform::{FileSystem, PersistedState, SettingsStore};
    use terra_providers::{SecretError, SecretStore};

    const SETTINGS_KEY: &str = "terra-globe:settings";
    const SECRET_PREFIX: &str = "terra-globe:secret:";

    fn storage() -> Option<web_sys::Storage> {
        web_sys::window()?.local_storage().ok()?
    }

    /// Settings persisted in `localStorage`, like the web app.
    pub struct BrowserSettingsStore;

    impl SettingsStore for BrowserSettingsStore {
        fn load(&self) -> PersistedState {
            storage()
                .and_then(|storage| storage.get_item(SETTINGS_KEY).ok().flatten())
                .and_then(|text| serde_json::from_str(&text).ok())
                .unwrap_or_default()
        }

        fn save(&self, state: &PersistedState) {
            if let (Some(storage), Ok(text)) = (storage(), serde_json::to_string(state)) {
                let _ = storage.set_item(SETTINGS_KEY, &text);
            }
        }
    }

    /// Provider keys in `localStorage`. Not a secure store; the desktop build
    /// uses the OS keychain and the web build matches the old app's behaviour.
    pub struct BrowserSecretStore;

    impl SecretStore for BrowserSecretStore {
        fn get(&self, key: &str) -> Result<Option<String>, SecretError> {
            Ok(storage().and_then(|storage| {
                storage
                    .get_item(&format!("{SECRET_PREFIX}{key}"))
                    .ok()
                    .flatten()
            }))
        }

        fn set(&self, key: &str, value: &str) -> Result<(), SecretError> {
            storage()
                .ok_or_else(|| SecretError::Backend("localStorage unavailable".into()))?
                .set_item(&format!("{SECRET_PREFIX}{key}"), value)
                .map_err(|_| SecretError::Backend("could not write to localStorage".into()))
        }

        fn remove(&self, key: &str) -> Result<(), SecretError> {
            if let Some(storage) = storage() {
                let _ = storage.remove_item(&format!("{SECRET_PREFIX}{key}"));
            }
            Ok(())
        }
    }

    /// The browser cannot read arbitrary paths; saving downloads the file.
    pub struct BrowserFiles;

    impl FileSystem for BrowserFiles {
        fn read(&self, _path: &Path) -> Result<Vec<u8>, String> {
            Err("the browser build cannot read files by path; use File > Open once the file picker lands".to_string())
        }

        fn write(&self, path: &Path, bytes: &[u8]) -> Result<(), String> {
            download(path, bytes)
        }

        fn pick_open(&self) -> Option<PathBuf> {
            None
        }

        fn pick_open_image(&self) -> Option<PathBuf> {
            None
        }

        fn pick_save(&self, suggested_name: &str) -> Option<PathBuf> {
            Some(PathBuf::from(suggested_name))
        }
    }

    fn download(path: &Path, bytes: &[u8]) -> Result<(), String> {
        use wasm_bindgen::JsCast;
        let window = web_sys::window().ok_or("no window")?;
        let document = window.document().ok_or("no document")?;
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("terra-globe.kml");
        let array = js_sys::Uint8Array::new_with_length(bytes.len() as u32);
        array.copy_from(bytes);
        let options = web_sys::BlobPropertyBag::new();
        options.set_type("application/octet-stream");
        let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&array, &options)
            .map_err(|_| "could not create the download blob".to_string())?;
        let url = web_sys::Url::create_object_url_with_blob(&blob)
            .map_err(|_| "could not create the download URL".to_string())?;
        let anchor = document
            .create_element("a")
            .map_err(|_| "could not create the download link".to_string())?
            .dyn_into::<web_sys::HtmlAnchorElement>()
            .map_err(|_| "download link has the wrong type".to_string())?;
        anchor.set_href(&url);
        anchor.set_download(name);
        anchor.click();
        let _ = web_sys::Url::revoke_object_url(&url);
        Ok(())
    }
}
