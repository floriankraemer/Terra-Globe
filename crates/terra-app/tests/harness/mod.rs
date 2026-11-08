use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use egui_kittest::Harness;
use terra_app::platform::{
    FileSystem, InMemorySettingsStore, MemoryFileSystem, PersistedState, SettingsStore,
};
use terra_app::{AppData, AppDeps};
use terra_globe::Globe;
use terra_globe::imagery::TileProvider;
use terra_providers::InMemorySecretStore;
use terra_storage::{InMemoryPlacesRepository, PlacesRepository};

pub const DOCUMENT: &str = r#"<kml><Document><Folder id="f1"><name>Places</name><Placemark id="p1"><name>Berlin</name><Point><coordinates>13.405,52.52,0</coordinates></Point></Placemark></Folder></Document></kml>"#;

pub fn make(document: Option<&str>) -> Option<(Harness<'static>, Rc<RefCell<AppData>>)> {
    make_sized(egui::vec2(800.0, 600.0), document)
}

/// A harness with an explicit window size, for tests that need a large globe
/// viewport to send pointer and keyboard input to.
pub fn make_sized(
    size: egui::Vec2,
    document: Option<&str>,
) -> Option<(Harness<'static>, Rc<RefCell<AppData>>)> {
    let app = make_app(size, document, PersistedState::default())?;
    Some((app.harness, app.data))
}

/// A running app plus the injected ports, for tests that inspect what was persisted.
#[allow(dead_code)]
pub struct App {
    pub harness: Harness<'static>,
    pub data: Rc<RefCell<AppData>>,
    pub files: Arc<MemoryFileSystem>,
    pub settings: Arc<InMemorySettingsStore>,
    pub repository: Arc<dyn PlacesRepository>,
}

pub fn make_app(
    size: egui::Vec2,
    document: Option<&str>,
    persisted: PersistedState,
) -> Option<App> {
    make_app_with(
        size,
        document,
        persisted,
        Arc::new(InMemoryPlacesRepository::new()),
        None,
    )
}

/// Like [`make_app`] over a given repository and, optionally, file system, so a
/// test can "restart" the app: a new `AppData` over the same stored data.
pub fn make_app_with(
    size: egui::Vec2,
    document: Option<&str>,
    persisted: PersistedState,
    repository: Arc<dyn PlacesRepository>,
    files: Option<Arc<MemoryFileSystem>>,
) -> Option<App> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    let globe = Globe::new(
        device,
        queue,
        TileProvider::new(terra_providers::TileSource::open_street_map()),
        (800, 500),
    );
    let files = files.unwrap_or_default();
    if let Some(doc) = document {
        files
            .write(PathBuf::from("/tmp/ui.kml").as_path(), doc.as_bytes())
            .ok()?;
    }
    let settings = Arc::new(InMemorySettingsStore::default());
    settings.save(&persisted);
    let mut data = AppData::from_parts(
        AppDeps {
            repository: repository.clone(),
            settings: settings.clone() as Arc<dyn SettingsStore>,
            files: files.clone(),
            secrets: Arc::new(InMemorySecretStore::new()),
            tile_cache: None,
        },
        globe,
        persisted,
    )
    .ok()?;
    if document.is_some() {
        data.open_path(PathBuf::from("/tmp/ui.kml"));
    }
    let data = Rc::new(RefCell::new(data));
    let closure = data.clone();
    let i18n = Rc::new(RefCell::new(terra_app::i18n::I18n::new("en")));
    let closure_i18n = i18n.clone();
    let mut harness = Harness::builder()
        .with_size(size)
        // One harness step is one frame at 60 Hz, so the navigation tests move
        // the way they would on screen.
        .with_step_dt(1.0 / 60.0)
        .build_ui(move |ui| {
            let mut data = closure.borrow_mut();
            let mut i18n = closure_i18n.borrow_mut();
            terra_app::views::app_ui(&mut data, &mut i18n, ui, None);
        });
    harness.run_steps(2);
    Some(App {
        harness,
        data,
        files,
        settings,
        repository,
    })
}
