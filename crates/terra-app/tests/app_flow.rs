//! End-to-end state-layer tests: import a document, edit it through the same
//! commands the UI calls, export it, and undo.
//!
//! These run with a headless wgpu device so the whole [`AppData`] is exercised,
//! not a mock of it. UI rendering itself needs a window and is covered by the
//! renderer snapshots plus a manual run.

use std::path::PathBuf;
use std::sync::Arc;

use terra_app::platform::{
    FileSystem, InMemorySettingsStore, MemoryFileSystem, PersistedState, SettingsStore,
};
use terra_app::state::AppError;
use terra_app::{AppData, AppDeps};
use terra_core::{Patch, PlacemarkPatch};
use terra_globe::Globe;
use terra_globe::imagery::TileProvider;
use terra_providers::InMemorySecretStore;
use terra_storage::InMemoryPlacesRepository;

const DOCUMENT: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<kml xmlns="http://www.opengis.net/kml/2.2"><Document>
  <Folder id="f1"><name>Places</name>
    <Placemark id="p1"><name>Berlin</name>
      <Point><coordinates>13.405,52.52,0</coordinates></Point></Placemark>
    <Placemark id="p2"><name>Ring</name>
      <Polygon><outerBoundaryIs><LinearRing><coordinates>13.3,52.4 13.5,52.4 13.5,52.6 13.3,52.4</coordinates></LinearRing></outerBoundaryIs></Polygon>
      <ExtendedData>
        <Data name="terra-globe:shape"><value>circle</value></Data>
        <Data name="terra-globe:centerLon"><value>13.4</value></Data>
        <Data name="terra-globe:centerLat"><value>52.5</value></Data>
        <Data name="terra-globe:radiusMeters"><value>1200</value></Data>
      </ExtendedData>
    </Placemark>
  </Folder>
</Document></kml>"#;

fn headless_globe() -> Option<Globe> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .ok()?;
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default())).ok()?;
    Some(Globe::new(
        device,
        queue,
        TileProvider::new(terra_providers::TileSource::open_street_map()),
        (800, 500),
    ))
}

struct Harness {
    data: AppData,
    files: Arc<MemoryFileSystem>,
}

fn harness(files: MemoryFileSystem) -> Option<Harness> {
    let globe = headless_globe()?;
    let files = Arc::new(files);
    let settings = Arc::new(InMemorySettingsStore::default()) as Arc<dyn SettingsStore>;
    let data = AppData::from_parts(
        AppDeps {
            repository: Arc::new(InMemoryPlacesRepository::new()),
            settings,
            files: files.clone(),
            secrets: Arc::new(InMemorySecretStore::new()),
        },
        globe,
        PersistedState::default(),
    )
    .expect("app data");
    Some(Harness { data, files })
}

#[test]
fn opening_a_document_imports_it_and_builds_a_scene() {
    let Some(mut harness) = harness(MemoryFileSystem::with_file(
        "/tmp/map.kml",
        DOCUMENT.as_bytes().to_vec(),
    )) else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    harness.data.open_path(PathBuf::from("/tmp/map.kml"));

    let library = harness.data.store.library();
    assert_eq!(library.folders.len(), 1);
    assert_eq!(library.placemarks.len(), 2);
    // The circle is reconstructed from ExtendedData, not left as a polygon.
    assert!(matches!(
        library.placemarks[1].geometry,
        terra_core::PlacemarkGeometry::Circle { .. }
    ));

    let (_, scene) = harness.data.build_scene();
    assert_eq!(scene.entries.len(), 2);
    assert!(scene.entries.iter().all(|entry| entry.visible));

    // Hiding the folder removes both placemarks from the scene.
    harness
        .data
        .store
        .set_folder_visibility("f1", false)
        .expect("hide");
    let (_, scene) = harness.data.build_scene();
    assert!(scene.entries.iter().all(|entry| !entry.visible));
}

#[test]
fn editor_updates_persist_and_can_be_undone() {
    let Some(mut harness) = harness(MemoryFileSystem::with_file(
        "/tmp/map.kml",
        DOCUMENT.as_bytes().to_vec(),
    )) else {
        return;
    };
    harness.data.open_path(PathBuf::from("/tmp/map.kml"));
    harness.data.select(Some("p1".to_string()));
    assert!(harness.data.editor.is_some());

    harness
        .data
        .store
        .update_placemark(
            "p1",
            PlacemarkPatch {
                name: Patch::Set("Berlin Centre".into()),
                description: Patch::Set(Some("edited".into())),
                ..Default::default()
            },
        )
        .expect("update");
    harness.data.select(Some("p1".to_string()));
    let editor = harness.data.editor.as_ref().expect("editor");
    assert_eq!(editor.name, "Berlin Centre");
    assert_eq!(editor.description, "edited");

    harness.data.store.undo().expect("undo");
    let placemark = harness
        .data
        .store
        .library()
        .placemark("p1")
        .expect("placemark");
    assert_eq!(placemark.name, "Berlin");
}

#[test]
fn drawing_creates_a_placemark_in_the_selected_folder() {
    let Some(mut harness) = harness(MemoryFileSystem::default()) else {
        return;
    };
    harness
        .data
        .store
        .create_folder("Drawings", None)
        .expect("folder");
    let folder = harness.data.store.library().folders[0].id.clone();
    harness.data.selected_folder = Some(folder.clone());

    let geometry = terra_core::PlacemarkGeometry::circle(
        terra_core::GeoPoint::new(1.0, 2.0).expect("point"),
        500.0,
    )
    .expect("circle");
    let placemark = harness
        .data
        .store
        .create_placemark(geometry, Some(folder.clone()))
        .expect("placemark");
    assert_eq!(placemark.folder_id.as_deref(), Some(folder.as_str()));

    // It gets a style, so the renderer can draw it.
    let library = harness.data.store.library();
    let style = library.style_for(library.placemark(&placemark.id).expect("exists"));
    assert!(style.filled);
}

#[test]
fn export_round_trips_and_autosave_writes_the_file() {
    let Some(mut harness) = harness(MemoryFileSystem::with_file(
        "/tmp/map.kml",
        DOCUMENT.as_bytes().to_vec(),
    )) else {
        return;
    };
    harness.data.open_path(PathBuf::from("/tmp/map.kml"));
    harness.data.file_path = Some(PathBuf::from("/tmp/map.kml"));
    harness.data.settings.auto_save = true;

    harness
        .data
        .store
        .rename_folder("f1", "Renamed".to_string())
        .expect("rename");
    harness.data.autosave();

    let written = harness
        .files
        .read(PathBuf::from("/tmp/map.kml").as_path())
        .expect("written");
    let text = String::from_utf8(written).expect("utf8");
    assert!(text.contains("Renamed"));
    let reparsed = terra_kml::parse_kml(&text).expect("reparses");
    assert_eq!(reparsed.placemarks.len(), 2);
    assert!(matches!(
        reparsed.placemarks[1].geometry,
        terra_core::PlacemarkGeometry::Circle { .. }
    ));
}

#[test]
fn settings_and_camera_are_persisted() {
    let Some(mut harness) = harness(MemoryFileSystem::default()) else {
        return;
    };
    harness.data.settings.language = "de".to_string();
    harness.data.settings.unit_system = terra_core::UnitSystem::Imperial;
    harness.data.globe.camera.distance = 123_456.0;
    harness.data.persist_settings();

    let persisted = harness.data.deps.settings.load();
    assert_eq!(persisted.settings.language, "de");
    assert_eq!(
        persisted.settings.unit_system,
        terra_core::UnitSystem::Imperial
    );
    let camera = persisted.camera.expect("camera stored");
    assert!((camera.distance - 123_456.0).abs() < 1e-6);
}

#[test]
fn invalid_documents_report_an_error_instead_of_importing_nothing_silently() {
    let Some(mut harness) = harness(MemoryFileSystem::with_file(
        "/tmp/bad.kml",
        b"this is not kml".to_vec(),
    )) else {
        return;
    };
    harness.data.open_path(PathBuf::from("/tmp/bad.kml"));
    let notice = harness.data.notice.as_ref().expect("notice");
    assert!(notice.error);
    assert!(harness.data.store.library().placemarks.is_empty());
}

#[test]
fn errors_are_typed_not_stringly() {
    // Guards the AppError surface the UI relies on for messages.
    let error = AppError::Message("boom".to_string());
    assert_eq!(error.to_string(), "boom");
}
