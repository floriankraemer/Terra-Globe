//! Layers and image overlays at the state level: settings restore, globe sync,
//! creation from the view, image loading and KML/KMZ round trips.

#[allow(dead_code)]
mod harness;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use harness::{App, make_app, make_app_with};
use std::sync::Arc;
use terra_app::platform::{FileSystem, PersistedState, SettingsStore};
use terra_app::{AppData, ImageState};
use terra_core::{OverlayLayerSetting, Patch, PlacemarkGeometry, PlacemarkPatch, Settings};
use terra_storage::{InMemoryPlacesRepository, PlacesRepository};

const PNG_1X1: &str = "data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR4nGP4z8DwHwAFAAH/iZk9HQAAAABJRU5ErkJggg==";

fn png() -> Vec<u8> {
    terra_kml::decode_data_url(PNG_1X1).expect("png").bytes
}

fn layer(id: &str, enabled: bool, opacity: f32) -> OverlayLayerSetting {
    OverlayLayerSetting {
        id: id.to_string(),
        enabled,
        opacity,
    }
}

fn persisted(layers: Vec<OverlayLayerSetting>) -> PersistedState {
    PersistedState {
        settings: Settings {
            overlay_layers: layers,
            ..Settings::default()
        },
        camera: None,
    }
}

fn app(persisted: PersistedState) -> Option<App> {
    let app = make_app(egui::vec2(800.0, 600.0), None, persisted);
    if app.is_none() {
        eprintln!("no GPU adapter; skipping overlay test");
    }
    app
}

/// Runs the job loop until every image load has finished.
fn pump(data: &mut AppData) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        data.handle_jobs();
        data.sync_images();
        if !data.images_loading() {
            break;
        }
        assert!(Instant::now() < deadline, "image loading timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
    data.sync_images();
}

fn pick_image(app: &App, path: &str, bytes: Vec<u8>) {
    app.files.write(Path::new(path), &bytes).expect("write");
    app.files.set_picked_image(Some(PathBuf::from(path)));
}

fn overlay_ids(data: &AppData) -> Vec<String> {
    data.image_overlays()
        .iter()
        .map(|overlay| overlay.id.clone())
        .collect()
}

fn first_overlay(data: &AppData) -> terra_core::Placemark {
    data.store
        .library()
        .placemarks
        .iter()
        .find(|placemark| matches!(placemark.geometry, PlacemarkGeometry::GroundOverlay { .. }))
        .expect("an overlay")
        .clone()
}

fn overlay_href(placemark: &terra_core::Placemark) -> String {
    let PlacemarkGeometry::GroundOverlay { image_url, .. } = &placemark.geometry else {
        panic!("not an overlay");
    };
    image_url.clone()
}

const RAILWAY_CREDIT: &str = "OpenRailwayMap";

fn credits(data: &AppData) -> String {
    data.globe.credits().join(" | ")
}

#[test]
fn enabled_layers_and_opacity_are_restored_on_startup() {
    let Some(app) = app(persisted(vec![
        layer("openseamap", true, 0.4),
        layer("openrailwaymap", false, 0.9),
    ])) else {
        return;
    };
    let data = app.data.borrow();
    assert_eq!(data.layer_state("openseamap"), (true, 0.4));
    assert_eq!(data.layer_state("openrailwaymap"), (false, 0.9));
    let active = terra_app::tile_overlays(&data.settings);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].source.id, "openseamap");
    assert!((active[0].opacity - 0.4).abs() < 1e-6);
    assert!(credits(&data).contains("OpenSeaMap"));
    assert!(!credits(&data).contains(RAILWAY_CREDIT));
}

#[test]
fn unknown_layer_ids_are_ignored() {
    let Some(app) = app(persisted(vec![
        layer("does-not-exist", true, 1.0),
        layer("openseamap", true, 1.0),
    ])) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    let ids: Vec<String> = terra_app::tile_overlays(&data.settings)
        .into_iter()
        .map(|overlay| overlay.source.id)
        .collect();
    assert_eq!(ids, vec!["openseamap".to_string()]);
    data.set_layer_enabled("does-not-exist", true);
    assert_eq!(data.layer_state("does-not-exist"), (true, 1.0));
    assert_eq!(
        data.settings
            .overlay_layers
            .iter()
            .filter(|layer| layer.id == "does-not-exist")
            .count(),
        1,
        "unknown ids are neither created nor duplicated by the UI paths"
    );
}

#[test]
fn settings_are_normalized_on_load() {
    let Some(app) = app(persisted(vec![
        layer("openseamap", true, 5.0),
        layer("openseamap", false, 0.1),
        layer("openrailwaymap", true, f32::NAN),
    ])) else {
        return;
    };
    let data = app.data.borrow();
    assert_eq!(data.layer_state("openseamap"), (true, 1.0));
    assert_eq!(data.layer_state("openrailwaymap"), (true, 1.0));
    assert_eq!(data.settings.overlay_layers.len(), 2);
}

#[test]
fn toggling_a_layer_updates_settings_store_and_globe() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    assert!(!credits(&data).contains(RAILWAY_CREDIT));

    data.set_layer_enabled("openrailwaymap", true);
    assert!(credits(&data).contains(RAILWAY_CREDIT));
    let saved = app.settings.load().settings.overlay_layers;
    assert_eq!(saved, vec![layer("openrailwaymap", true, 1.0)]);

    data.set_layer_opacity("openrailwaymap", 0.25);
    assert_eq!(
        app.settings.load().settings.overlay_layers,
        vec![layer("openrailwaymap", true, 0.25)]
    );

    data.set_layer_enabled("openrailwaymap", false);
    assert!(!credits(&data).contains(RAILWAY_CREDIT));
    assert!(terra_app::tile_overlays(&data.settings).is_empty());
}

#[test]
fn opacity_is_clamped_when_set() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    data.set_layer_opacity("openseamap", 7.0);
    assert_eq!(data.layer_state("openseamap").1, 1.0);
    data.set_layer_opacity("openseamap", -3.0);
    assert_eq!(data.layer_state("openseamap").1, 0.0);
}

#[test]
fn overlays_follow_catalog_order_not_toggle_order() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    data.set_layer_enabled("waymarked-hiking", true);
    data.set_layer_enabled("openrailwaymap", true);
    let ids: Vec<String> = terra_app::tile_overlays(&data.settings)
        .into_iter()
        .map(|overlay| overlay.source.id)
        .collect();
    assert_eq!(ids, vec!["openrailwaymap", "waymarked-hiking"]);
}

#[test]
fn switching_the_basemap_keeps_overlays_and_persists() {
    let Some(app) = app(persisted(vec![layer("openseamap", true, 1.0)])) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    data.set_basemap("opentopomap");
    assert_eq!(data.settings.tile_source_id, "opentopomap");
    assert_eq!(app.settings.load().settings.tile_source_id, "opentopomap");
    assert_eq!(data.layer_state("openseamap"), (true, 1.0));
    let credits = credits(&data);
    assert!(credits.contains("OpenSeaMap") && credits.contains("OpenTopoMap"));

    data.set_basemap("not-a-basemap");
    assert_eq!(data.settings.tile_source_id, "opentopomap");
}

#[test]
fn creating_an_image_overlay_uses_the_view_and_undo_removes_it() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();

    let placemark = first_overlay(&data);
    assert_eq!(placemark.name, "harbour");
    assert_eq!(data.selected.as_deref(), Some(placemark.id.as_str()));
    let PlacemarkGeometry::GroundOverlay {
        bounds,
        image_url,
        rotation,
        opacity,
    } = placemark.geometry
    else {
        panic!("not an overlay");
    };
    assert!(
        image_url.starts_with("images/harbour-") && image_url.ends_with(".png"),
        "{image_url}"
    );
    assert_eq!(
        pollster::block_on(data.store.repository().get_asset(&image_url)).unwrap(),
        Some(png()),
        "the picture was copied into the asset store"
    );
    assert_eq!((rotation, opacity), (None, None));
    let (width, height) = data.globe.viewport();
    let middle = (f64::from(width) / 2.0, f64::from(height) / 2.0);
    let view = data.globe.pick(middle).expect("view centre on globe");
    assert!(bounds.south < view.lat_deg && view.lat_deg < bounds.north);
    assert!(bounds.west < view.lon_deg && view.lon_deg < bounds.east);
    let edge = data.globe.pick((0.0, 0.0));
    if let Some(edge) = edge {
        assert!(
            bounds.north < edge.lat_deg.max(view.lat_deg) + 1e-6
                && bounds.west > edge.lon_deg.min(view.lon_deg) - 1e-6,
            "the box is inside the visible area, not the whole of it"
        );
    }

    data.store.undo().expect("undo");
    assert!(data.store.library().placemarks.is_empty());
}

#[test]
fn a_cancelled_picker_creates_nothing() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    app.files.set_picked_image(None);
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    assert!(data.store.library().placemarks.is_empty());
    assert!(!data.store.can_undo());
    assert!(data.notice.is_none());
}

#[test]
fn an_unreadable_pick_reports_an_error_and_creates_nothing() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    app.files
        .set_picked_image(Some(PathBuf::from("/tmp/missing.png")));
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    assert!(data.store.library().placemarks.is_empty());
    assert!(data.notice.as_ref().is_some_and(|notice| notice.error));
}

#[test]
fn visible_overlays_load_and_reach_the_globe() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/a.png", png());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    assert!(data.image_overlays().is_empty(), "nothing before the load");
    pump(&mut data);
    let id = first_overlay(&data).id;
    assert_eq!(overlay_ids(&data), vec![id]);
    assert_eq!(data.image_overlays()[0].opacity, 1.0);
}

#[test]
fn hidden_placemarks_and_folders_are_excluded() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/a.png", png());
    let mut data = app.data.borrow_mut();
    let folder = data.store.create_folder("Maps", None).expect("folder");
    data.selected_folder = Some(folder.id.clone());
    data.add_image_overlay();
    pump(&mut data);
    let id = first_overlay(&data).id;
    assert_eq!(overlay_ids(&data), vec![id.clone()]);

    data.store
        .set_folder_visibility(&folder.id, false)
        .expect("hide folder");
    data.sync_images();
    assert!(data.image_overlays().is_empty());

    data.store
        .set_folder_visibility(&folder.id, true)
        .expect("show folder");
    data.store
        .set_placemark_visibility(&id, false)
        .expect("hide placemark");
    data.sync_images();
    assert!(data.image_overlays().is_empty());

    data.store
        .set_placemark_visibility(&id, true)
        .expect("show placemark");
    data.sync_images();
    assert_eq!(overlay_ids(&data), vec![id]);
}

#[test]
fn opacity_and_rotation_reach_the_globe() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/a.png", png());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    pump(&mut data);
    let placemark = first_overlay(&data);
    let PlacemarkGeometry::GroundOverlay {
        bounds, image_url, ..
    } = placemark.geometry
    else {
        panic!("not an overlay");
    };
    let geometry =
        PlacemarkGeometry::ground_overlay(bounds, image_url, Some(45.0), Some(0.5)).expect("ok");
    data.store
        .update_placemark(
            &placemark.id,
            PlacemarkPatch {
                geometry: Patch::Set(geometry),
                ..Default::default()
            },
        )
        .expect("update");
    data.sync_images();
    let applied = &data.image_overlays()[0];
    assert_eq!((applied.rotation_deg, applied.opacity), (45.0, 0.5));
}

#[test]
fn a_failed_load_notifies_once_and_leaves_the_placemark_alone() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/broken.png", b"not an image".to_vec());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    let before = first_overlay(&data);
    pump(&mut data);

    let notice = data.notice.take().expect("failure notice");
    let href = overlay_href(&before);
    assert!(notice.error && notice.message.contains("broken-"));
    assert!(data.image_overlays().is_empty());
    assert!(matches!(data.images.get(&href), Some(ImageState::Failed)));
    assert_eq!(first_overlay(&data), before);

    data.sync_images();
    data.handle_jobs();
    assert!(data.notice.is_none(), "one toast per href");
}

#[test]
fn a_missing_file_fails_without_panicking() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    let bounds = terra_core::RectangleBounds::new(10.0, 0.0, 10.0, 0.0).expect("bounds");
    let geometry = PlacemarkGeometry::ground_overlay(bounds, "/nowhere/map.png".into(), None, None)
        .expect("geometry");
    data.store.create_placemark(geometry, None).expect("create");
    pump(&mut data);
    assert!(data.notice.as_ref().is_some_and(|notice| notice.error));
    assert_eq!(data.store.library().placemarks.len(), 1);
}

fn kmz_with_overlay() -> Vec<u8> {
    let kml = r#"<kml><Document><GroundOverlay id="g1"><name>Embedded</name><color>80ffffff</color><Icon><href>images/pic.png</href></Icon><LatLonBox><north>10</north><south>0</south><east>10</east><west>0</west><rotation>15</rotation></LatLonBox></GroundOverlay></Document></kml>"#;
    terra_kml::write_kmz(
        kml,
        &[terra_kml::KmzEntry {
            name: "images/pic.png".into(),
            bytes: png(),
        }],
    )
    .expect("kmz")
}

#[test]
fn kmz_import_resolves_embedded_images() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    app.files
        .write(Path::new("/tmp/embedded.kmz"), &kmz_with_overlay())
        .expect("write");
    let mut data = app.data.borrow_mut();
    data.open_path(PathBuf::from("/tmp/embedded.kmz"));
    pump(&mut data);
    assert_eq!(overlay_ids(&data), vec!["g1".to_string()]);
    let applied = &data.image_overlays()[0];
    assert_eq!(applied.rotation_deg, 15.0);
    assert!((applied.opacity - 128.0 / 255.0).abs() < 1e-3);
}

#[test]
fn a_kmz_without_its_image_reports_a_failure_but_imports_the_overlay() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let kml = r#"<kml><Document><GroundOverlay id="g1"><name>Lost</name><Icon><href>images/gone.png</href></Icon><LatLonBox><north>10</north><south>0</south><east>10</east><west>0</west></LatLonBox></GroundOverlay></Document></kml>"#;
    app.files
        .write(
            Path::new("/tmp/lost.kmz"),
            &terra_kml::write_kmz(kml, &[]).expect("kmz"),
        )
        .expect("write");
    let mut data = app.data.borrow_mut();
    data.open_path(PathBuf::from("/tmp/lost.kmz"));
    pump(&mut data);
    assert!(data.store.library().placemark("g1").is_some());
    assert!(data.image_overlays().is_empty());
    assert!(data.notice.as_ref().is_some_and(|notice| notice.error));
}

#[test]
fn relative_hrefs_resolve_next_to_the_open_kml() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let kml = r#"<kml><Document><GroundOverlay id="g1"><name>Local</name><Icon><href>pic.png</href></Icon><LatLonBox><north>10</north><south>0</south><east>10</east><west>0</west></LatLonBox></GroundOverlay></Document></kml>"#;
    app.files
        .write(Path::new("/maps/doc.kml"), kml.as_bytes())
        .expect("write");
    app.files
        .write(Path::new("/maps/pic.png"), &png())
        .expect("write");
    let mut data = app.data.borrow_mut();
    data.open_path(PathBuf::from("/maps/doc.kml"));
    pump(&mut data);
    assert_eq!(overlay_ids(&data), vec!["g1".to_string()]);
}

#[test]
fn kml_export_keeps_href_rotation_and_opacity() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    let bounds = terra_core::RectangleBounds::new(10.0, 0.0, 10.0, 0.0).expect("bounds");
    let geometry = PlacemarkGeometry::ground_overlay(
        bounds,
        "https://example.test/a.png".into(),
        Some(30.0),
        Some(0.5),
    )
    .expect("geometry");
    data.store
        .create_placemark(geometry.clone(), None)
        .expect("create");
    data.export_to(PathBuf::from("/tmp/out.kml"), false);
    let kml = String::from_utf8(
        app.files
            .read_back(Path::new("/tmp/out.kml"))
            .expect("file"),
    )
    .expect("utf8");
    let parsed = terra_kml::parse_kml(&kml).expect("parses");
    let PlacemarkGeometry::GroundOverlay {
        image_url,
        rotation,
        opacity,
        ..
    } = &parsed.placemarks[0].geometry
    else {
        panic!("not an overlay");
    };
    assert_eq!(image_url, "https://example.test/a.png");
    assert_eq!(*rotation, Some(30.0));
    assert!((opacity.expect("opacity") - 0.5).abs() < 0.01);
}

#[test]
fn kmz_export_embeds_local_images() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    data.add_image_overlay();
    data.export_to(PathBuf::from("/tmp/out.kmz"), true);
    assert!(data.notice.as_ref().is_some_and(|notice| !notice.error));

    let bytes = app
        .files
        .read_back(Path::new("/tmp/out.kmz"))
        .expect("file");
    let entries = terra_kml::read_kmz(&bytes).expect("kmz");
    let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
    let href = overlay_href(&first_overlay(&data));
    assert_eq!(names, vec!["doc.kml", href.as_str()]);
    assert_eq!(entries[1].bytes, png());
    let kml = String::from_utf8(entries[0].bytes.clone()).expect("utf8");
    assert!(kml.contains(&format!("<href>{href}</href>")));
    assert!(!kml.contains("/tmp/pictures"));
}

#[test]
fn kmz_export_round_trips_through_import() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    {
        let mut data = app.data.borrow_mut();
        data.add_image_overlay();
        data.export_to(PathBuf::from("/tmp/out.kmz"), true);
    }
    let Some(second) = app_with_files_from(&app) else {
        return;
    };
    let mut data = second.data.borrow_mut();
    data.open_path(PathBuf::from("/tmp/out.kmz"));
    pump(&mut data);
    assert_eq!(data.image_overlays().len(), 1);
}

fn app_with_files_from(source: &App) -> Option<App> {
    let second = app(PersistedState::default())?;
    let bytes = source.files.read_back(Path::new("/tmp/out.kmz"))?;
    second.files.write(Path::new("/tmp/out.kmz"), &bytes).ok()?;
    Some(second)
}

#[test]
fn kmz_export_keeps_unreadable_hrefs_and_reports_them() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    let bounds = terra_core::RectangleBounds::new(10.0, 0.0, 10.0, 0.0).expect("bounds");
    let geometry =
        PlacemarkGeometry::ground_overlay(bounds, "/gone/map.png".into(), None, None).expect("ok");
    data.store.create_placemark(geometry, None).expect("create");
    data.export_to(PathBuf::from("/tmp/out.kmz"), true);
    let bytes = app
        .files
        .read_back(Path::new("/tmp/out.kmz"))
        .expect("file");
    let entries = terra_kml::read_kmz(&bytes).expect("kmz");
    assert_eq!(entries.len(), 1);
    assert!(String::from_utf8_lossy(&entries[0].bytes).contains("/gone/map.png"));
    assert!(data.notice.is_some());
}

#[test]
fn bounds_fall_back_to_the_camera_target_when_the_view_misses_the_globe() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    data.globe.camera.distance = 5.0e7;
    let (width, height) = data.globe.viewport();
    assert!(
        data.globe
            .pick((f64::from(width) * 0.25, f64::from(height) * 0.25))
            .is_none(),
        "precondition: the view corner is in space"
    );
    let bounds = data.view_bounds().expect("fallback bounds");
    let pose = data.globe.pose();
    assert!(bounds.south < pose.lat_deg && pose.lat_deg < bounds.north);
    assert!(bounds.west < pose.lon_deg && pose.lon_deg < bounds.east);
}

// --- persisted assets -------------------------------------------------------

fn restart(source: &App, keep_files: bool) -> Option<App> {
    make_app_with(
        egui::vec2(800.0, 600.0),
        None,
        PersistedState::default(),
        source.repository.clone(),
        keep_files.then(|| source.files.clone()),
    )
}

fn ground_overlay(href: &str) -> PlacemarkGeometry {
    let bounds = terra_core::RectangleBounds::new(10.0, 0.0, 10.0, 0.0).expect("bounds");
    PlacemarkGeometry::ground_overlay(bounds, href.into(), None, None).expect("geometry")
}

fn stored(app: &App, path: &str) -> Option<Vec<u8>> {
    pollster::block_on(app.repository.get_asset(path)).expect("get_asset")
}

fn import_kmz(app: &App) {
    app.files
        .write(Path::new("/tmp/embedded.kmz"), &kmz_with_overlay())
        .expect("write");
    app.data
        .borrow_mut()
        .open_path(PathBuf::from("/tmp/embedded.kmz"));
}

#[test]
fn kmz_import_stores_only_referenced_images_and_survives_restart() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let kml = r#"<kml><Document><GroundOverlay id="g1"><name>E</name><Icon><href>./images/pic.png</href></Icon><LatLonBox><north>10</north><south>0</south><east>10</east><west>0</west></LatLonBox></GroundOverlay></Document></kml>"#;
    let kmz = terra_kml::write_kmz(
        kml,
        &[
            terra_kml::KmzEntry {
                name: "images/pic.png".into(),
                bytes: png(),
            },
            terra_kml::KmzEntry {
                name: "files/junk.bin".into(),
                bytes: vec![1, 2, 3],
            },
        ],
    )
    .expect("kmz");
    app.files
        .write(Path::new("/tmp/two.kmz"), &kmz)
        .expect("write");
    app.data
        .borrow_mut()
        .open_path(PathBuf::from("/tmp/two.kmz"));
    assert_eq!(
        pollster::block_on(app.repository.list_assets()).unwrap(),
        vec!["images/pic.png".to_string()],
        "unreferenced entries are not stored"
    );

    let Some(second) = restart(&app, false) else {
        return;
    };
    let mut data = second.data.borrow_mut();
    pump(&mut data);
    assert_eq!(overlay_ids(&data), vec!["g1".to_string()]);
}

#[test]
fn picked_image_survives_restart_after_the_original_is_gone() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    app.data.borrow_mut().add_image_overlay();
    // The restarted app gets an empty file system: the original is gone.
    let Some(second) = restart(&app, false) else {
        return;
    };
    let mut data = second.data.borrow_mut();
    pump(&mut data);
    assert_eq!(data.image_overlays().len(), 1);
    assert!(data.notice.is_none());
}

#[test]
fn kmz_export_after_restart_embeds_the_stored_image() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    import_kmz(&app);
    let Some(second) = restart(&app, false) else {
        return;
    };
    let mut data = second.data.borrow_mut();
    data.export_to(PathBuf::from("/tmp/again.kmz"), true);
    assert!(data.notice.as_ref().is_some_and(|notice| !notice.error));
    let entries =
        terra_kml::read_kmz(&second.files.read_back(Path::new("/tmp/again.kmz")).unwrap())
            .expect("kmz");
    let image = entries
        .iter()
        .find(|entry| entry.name == "images/pic.png")
        .expect("image embedded");
    assert_eq!(image.bytes, png());
}

#[test]
fn undoing_an_overlay_delete_keeps_its_image() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    let placemark = first_overlay(&data);
    let href = overlay_href(&placemark);
    data.store.delete_placemark(&placemark.id).expect("delete");
    assert_eq!(
        pollster::block_on(data.store.repository().get_asset(&href)).unwrap(),
        Some(png()),
        "deleting does not remove the asset"
    );
    data.store.undo().expect("undo");
    data.assets.clear();
    pump(&mut data);
    assert_eq!(overlay_ids(&data), vec![placemark.id]);
}

#[test]
fn startup_collects_unreferenced_assets_but_keeps_referenced_ones() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    import_kmz(&app);
    pollster::block_on(async {
        app.repository
            .put_asset("images/orphan-1.png", png())
            .await
            .unwrap();
        app.repository
            .put_asset("other/orphan.bin", vec![0])
            .await
            .unwrap();
    });
    let Some(second) = restart(&app, false) else {
        return;
    };
    assert_eq!(
        pollster::block_on(second.repository.list_assets()).unwrap(),
        vec!["images/pic.png".to_string()]
    );
    let mut data = second.data.borrow_mut();
    pump(&mut data);
    assert_eq!(overlay_ids(&data), vec!["g1".to_string()]);
}

#[test]
fn collecting_never_touches_assets_of_deleted_but_undoable_overlays_in_a_session() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    app.data.borrow_mut().add_image_overlay();
    let href = overlay_href(&first_overlay(&app.data.borrow()));
    let id = first_overlay(&app.data.borrow()).id;
    app.data.borrow_mut().store.delete_placemark(&id).unwrap();
    assert!(stored(&app, &href).is_some(), "only a restart collects");
    let Some(second) = restart(&app, false) else {
        return;
    };
    assert!(
        stored(&second, &href).is_none(),
        "unreferenced after restart"
    );
}

#[test]
fn a_missing_stored_asset_fails_with_one_notice_and_keeps_the_placemark() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    let mut data = app.data.borrow_mut();
    data.store
        .create_placemark(ground_overlay("images/nothing.png"), None)
        .expect("create");
    pump(&mut data);
    let notice = data.notice.take().expect("notice");
    assert!(notice.error && notice.message.contains("images/nothing.png"));
    data.sync_images();
    data.handle_jobs();
    assert!(data.notice.is_none(), "one toast per href");
    assert_eq!(data.store.library().placemarks.len(), 1);
}

#[test]
fn corrupt_asset_bytes_fail_with_a_notice() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    pollster::block_on(
        app.repository
            .put_asset("images/bad.png", b"garbage".to_vec()),
    )
    .unwrap();
    let mut data = app.data.borrow_mut();
    data.store
        .create_placemark(ground_overlay("images/bad.png"), None)
        .expect("create");
    pump(&mut data);
    assert!(data.notice.as_ref().is_some_and(|notice| notice.error));
    assert!(data.image_overlays().is_empty());
    assert!(matches!(
        data.images.get("images/bad.png"),
        Some(ImageState::Failed)
    ));
}

/// Delegates everything to an in-memory repository except `put_asset`, which fails.
struct RejectingAssets(InMemoryPlacesRepository);

macro_rules! delegating_repo {
    ($($name:ident($($arg:ident: $ty:ty),*) -> $ret:ty;)*) => {
        #[async_trait::async_trait(?Send)]
        impl PlacesRepository for RejectingAssets {
            $(async fn $name(&self, $($arg: $ty),*) -> $ret { self.0.$name($($arg),*).await })*

            async fn put_asset(
                &self,
                _path: &str,
                _bytes: Vec<u8>,
            ) -> Result<(), terra_storage::StorageError> {
                Err(terra_storage::StorageError::Backend("disk full".into()))
            }
        }
    };
}

delegating_repo! {
        get_folder(id: &str) -> Result<Option<terra_core::Folder>, terra_storage::StorageError>;
        list_folders(p: Option<&str>) -> Result<Vec<terra_core::Folder>, terra_storage::StorageError>;
        create_folder(i: terra_core::NewFolder) -> Result<terra_core::Folder, terra_storage::StorageError>;
        update_folder(id: &str, p: terra_core::domain::folder::FolderPatch) -> Result<terra_core::Folder, terra_storage::StorageError>;
        delete_folder(id: &str, r: bool) -> Result<(), terra_storage::StorageError>;
        get_placemark(id: &str) -> Result<Option<terra_core::Placemark>, terra_storage::StorageError>;
        list_placemarks(f: Option<&str>) -> Result<Vec<terra_core::Placemark>, terra_storage::StorageError>;
        create_placemark(i: terra_core::NewPlacemark, g: PlacemarkGeometry) -> Result<terra_core::Placemark, terra_storage::StorageError>;
        update_placemark(id: &str, p: PlacemarkPatch) -> Result<terra_core::Placemark, terra_storage::StorageError>;
        delete_placemark(id: &str) -> Result<(), terra_storage::StorageError>;
        get_style(id: &str) -> Result<Option<terra_core::Style>, terra_storage::StorageError>;
        list_styles() -> Result<Vec<terra_core::Style>, terra_storage::StorageError>;
        upsert_style(s: terra_core::Style) -> Result<terra_core::Style, terra_storage::StorageError>;
        delete_style(id: &str) -> Result<(), terra_storage::StorageError>;
        list_screen_overlays(f: Option<&str>) -> Result<Vec<terra_core::ScreenOverlay>, terra_storage::StorageError>;
        create_screen_overlay(o: terra_core::NewScreenOverlay) -> Result<terra_core::ScreenOverlay, terra_storage::StorageError>;
        delete_screen_overlay(id: &str) -> Result<(), terra_storage::StorageError>;
        import_batch(b: terra_storage::ImportBatch) -> Result<(), terra_storage::StorageError>;
        get_asset(p: &str) -> Result<Option<Vec<u8>>, terra_storage::StorageError>;
        delete_asset(p: &str) -> Result<(), terra_storage::StorageError>;
        list_assets() -> Result<Vec<String>, terra_storage::StorageError>;
}

fn rejecting_app() -> Option<App> {
    make_app_with(
        egui::vec2(800.0, 600.0),
        None,
        PersistedState::default(),
        Arc::new(RejectingAssets(InMemoryPlacesRepository::new())),
        None,
    )
}

#[test]
fn a_failing_asset_write_creates_no_overlay_when_picking() {
    let Some(app) = rejecting_app() else {
        return;
    };
    pick_image(&app, "/tmp/pictures/harbour.png", png());
    let mut data = app.data.borrow_mut();
    data.add_image_overlay();
    let notice = data.notice.as_ref().expect("notice");
    assert!(notice.error && notice.message.contains("disk full"));
    assert!(data.store.library().placemarks.is_empty());
    assert!(!data.store.can_undo());
}

#[test]
fn a_failing_asset_write_imports_nothing_from_a_kmz() {
    let Some(app) = rejecting_app() else {
        return;
    };
    import_kmz(&app);
    let data = app.data.borrow();
    assert!(data.notice.as_ref().is_some_and(|notice| notice.error));
    assert!(data.store.library().placemarks.is_empty());
    assert!(data.file_path.is_none());
}

#[test]
fn legacy_absolute_hrefs_still_load() {
    let Some(app) = app(PersistedState::default()) else {
        return;
    };
    app.files
        .write(Path::new("/old/place/map.png"), &png())
        .expect("write");
    let mut data = app.data.borrow_mut();
    data.store
        .create_placemark(ground_overlay("/old/place/map.png"), None)
        .expect("create");
    pump(&mut data);
    assert_eq!(data.image_overlays().len(), 1);
}
