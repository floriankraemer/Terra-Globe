//! Headless automation of the Layers panel and the image overlay workflow.

#[allow(dead_code)]
mod harness;

use std::path::{Path, PathBuf};

use egui_kittest::kittest::{NodeT, Queryable};
use harness::{App, make_app};
use terra_app::platform::{FileSystem, PersistedState, SettingsStore};
use terra_core::PlacemarkGeometry;

const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D', b'R',
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f, 0x15, 0xc4,
    0x89, 0x00, 0x00, 0x00, 0x0d, b'I', b'D', b'A', b'T', 0x78, 0x9c, 0x63, 0xf8, 0xcf, 0xc0, 0xf0,
    0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0xab, 0xce, 0x36, 0x89, 0x00, 0x00, 0x00, 0x00, b'I', b'E',
    b'N', b'D', 0xae, 0x42, 0x60, 0x82,
];

fn app() -> Option<App> {
    let app = make_app(egui::vec2(1000.0, 800.0), None, PersistedState::default());
    if app.is_none() {
        eprintln!("no GPU adapter; skipping layers test");
    }
    app
}

fn overlays(app: &App) -> Vec<terra_core::Placemark> {
    app.data
        .borrow()
        .store
        .library()
        .placemarks
        .iter()
        .filter(|placemark| matches!(placemark.geometry, PlacemarkGeometry::GroundOverlay { .. }))
        .cloned()
        .collect()
}

fn create_overlay_through_the_menu(app: &mut App) {
    app.files
        .write(Path::new("/tmp/pictures/harbour.png"), PNG)
        .expect("write");
    app.files
        .set_picked_image(Some(PathBuf::from("/tmp/pictures/harbour.png")));
    app.harness.get_by_label("Geometry").click();
    app.harness.run();
    app.harness.get_by_label("Image Overlay…").click();
    app.harness.run();
}

#[test]
fn the_layers_section_lists_base_maps_and_overlays() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness.get_by_label("Layers");
    app.harness.get_by_label("Base map");
    app.harness.get_by_label("Overlays");
    for name in [
        "OpenStreetMap",
        "OpenTopoMap",
        "OpenRailwayMap",
        "OpenSeaMap",
    ] {
        app.harness.get_by_label(name);
    }
}

#[test]
fn the_layers_header_collapses_and_expands() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness.get_by_label("Collapse Layers").click();
    app.harness.run();
    assert!(app.harness.query_by_label("OpenRailwayMap").is_none());
    app.harness.get_by_label("Layers");
    app.harness.get_by_label("Expand Layers").click();
    app.harness.run();
    app.harness.get_by_label("OpenRailwayMap");
}

#[test]
fn the_map_menu_is_gone_from_the_top_bar() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    assert!(app.harness.query_by_label("Map").is_none());
}

#[test]
fn the_eye_enables_an_overlay_and_persists_it() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness
        .get_by_label("Toggle layer OpenRailwayMap")
        .click();
    app.harness.run();
    let saved = app.settings.load().settings.overlay_layers;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, "openrailwaymap");
    assert!(saved[0].enabled);
    assert!(
        app.data
            .borrow()
            .globe
            .credits()
            .iter()
            .any(|credit| credit.contains("OpenRailwayMap"))
    );

    app.harness
        .get_by_label("Toggle layer OpenRailwayMap")
        .click();
    app.harness.run();
    assert!(!app.settings.load().settings.overlay_layers[0].enabled);
}

#[test]
fn the_opacity_slider_appears_for_the_selected_overlay_and_persists() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    assert!(app.harness.query_by_label_contains("Opacity").is_none());
    app.harness.get_by_label("OpenSeaMap").click();
    app.harness.run();
    app.harness
        .get_by_role_and_label(egui::accesskit::Role::SpinButton, "Opacity")
        .set_value_accesskit(40.0);
    app.harness.run();
    let saved = app.settings.load().settings.overlay_layers;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].id, "openseamap");
    assert!((saved[0].opacity - 0.4).abs() < 1e-6);
}

#[test]
fn choosing_a_base_map_switches_it_and_keeps_overlays() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness.get_by_label("Toggle layer OpenSeaMap").click();
    app.harness.run();
    app.harness.get_by_label("OpenTopoMap").click();
    app.harness.run();
    assert_eq!(app.data.borrow().settings.tile_source_id, "opentopomap");
    let saved = app.settings.load().settings;
    assert_eq!(saved.tile_source_id, "opentopomap");
    assert!(saved.overlay_layers[0].enabled);

    app.harness
        .get_by_label("Use OpenStreetMap as base map")
        .click();
    app.harness.run();
    assert_eq!(app.data.borrow().settings.tile_source_id, "osm");
}

#[test]
fn exactly_one_base_map_is_chosen() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    let chosen = |app: &App| {
        ["OpenStreetMap", "OpenTopoMap"]
            .iter()
            .filter(|name| {
                app.harness
                    .get_by_label(&format!("Use {name} as base map"))
                    .accesskit_node()
                    .toggled()
                    == Some(egui::accesskit::Toggled::True)
            })
            .count()
    };
    assert_eq!(chosen(&app), 1);
    app.harness.get_by_label("OpenTopoMap").click();
    app.harness.run();
    assert_eq!(chosen(&app), 1);
}

#[test]
fn image_overlay_menu_entry_creates_an_overlay_and_opens_the_editor() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    create_overlay_through_the_menu(&mut app);

    let created = overlays(&app);
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].name, "harbour");
    app.harness.get_by_label("North");
    app.harness.get_by_label("South");
    app.harness.get_by_label("East");
    app.harness.get_by_label("West");
    app.harness.get_by_label("Rotation");
    app.harness.get_by_label("Change image…");
    let editor = app.data.borrow().editor.clone().expect("editor");
    let overlay = editor.overlay.expect("overlay buffers");
    let PlacemarkGeometry::GroundOverlay { bounds, .. } = created[0].geometry else {
        panic!("not an overlay");
    };
    assert_eq!(overlay.north, bounds.north.to_string());
    assert!(
        overlay.href.starts_with("images/harbour-") && overlay.href.ends_with(".png"),
        "{}",
        overlay.href
    );
}

#[test]
fn a_cancelled_image_dialog_creates_nothing() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.files.set_picked_image(None);
    app.harness.get_by_label("Geometry").click();
    app.harness.run();
    app.harness.get_by_label("Image Overlay…").click();
    app.harness.run();
    assert!(overlays(&app).is_empty());
    assert!(app.data.borrow().editor.is_none());
}

#[test]
fn the_editor_applies_valid_bounds() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    create_overlay_through_the_menu(&mut app);
    app.harness
        .get_by_label("North")
        .set_text_value_accesskit("60");
    app.harness
        .get_by_label("South")
        .set_text_value_accesskit("40");
    app.harness
        .get_by_label("Rotation")
        .set_text_value_accesskit("25");
    app.harness.run();
    app.harness.get_by_label("Save").click();
    app.harness.run();
    let PlacemarkGeometry::GroundOverlay {
        bounds, rotation, ..
    } = overlays(&app)[0].geometry.clone()
    else {
        panic!("not an overlay");
    };
    assert_eq!((bounds.north, bounds.south), (60.0, 40.0));
    assert_eq!(rotation, Some(25.0));
}

#[test]
fn the_editor_rejects_north_below_south() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    create_overlay_through_the_menu(&mut app);
    let before = overlays(&app)[0].geometry.clone();
    app.harness
        .get_by_label("North")
        .set_text_value_accesskit("-80");
    app.harness
        .get_by_label("South")
        .set_text_value_accesskit("10");
    app.harness.run();
    app.harness.get_by_label("Save").click();
    app.harness.run();
    assert_eq!(
        overlays(&app)[0].geometry,
        before,
        "invalid input is not applied"
    );
    app.harness.get_by_label_contains("Invalid image overlay");
}

#[test]
fn the_editor_rejects_latitude_beyond_the_poles_and_text() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    create_overlay_through_the_menu(&mut app);
    let before = overlays(&app)[0].geometry.clone();
    for bad in ["95", "north"] {
        app.harness
            .get_by_label("North")
            .set_text_value_accesskit(bad);
        app.harness.run();
        app.harness.get_by_label("Save").click();
        app.harness.run();
        assert_eq!(overlays(&app)[0].geometry, before, "{bad}");
        app.harness.get_by_label_contains("Invalid image overlay");
    }
}

#[test]
fn change_image_replaces_the_href_on_save() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    create_overlay_through_the_menu(&mut app);
    app.files
        .write(Path::new("/tmp/pictures/other.png"), PNG)
        .expect("write");
    app.files
        .set_picked_image(Some(PathBuf::from("/tmp/pictures/other.png")));
    app.harness.get_by_label("Change image…").click();
    app.harness.run();
    app.harness.get_by_label("Save").click();
    app.harness.run();
    let PlacemarkGeometry::GroundOverlay { image_url, .. } = overlays(&app)[0].geometry.clone()
    else {
        panic!("not an overlay");
    };
    assert!(
        image_url.starts_with("images/other-") && image_url.ends_with(".png"),
        "{image_url}"
    );
    let picked = image_url;

    app.files.set_picked_image(None);
    app.harness.get_by_label("Change image…").click();
    app.harness.run();
    app.harness.get_by_label("Save").click();
    app.harness.run();
    let PlacemarkGeometry::GroundOverlay { image_url, .. } = overlays(&app)[0].geometry.clone()
    else {
        panic!("not an overlay");
    };
    assert_eq!(image_url, picked, "cancel keeps the image");
}

// --- terrain ----------------------------------------------------------------

#[test]
fn the_terrain_row_is_the_first_layer_and_starts_on() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    let terrain = app
        .harness
        .get_by_role_and_label(egui::accesskit::Role::TreeItem, "Terrain (3D)")
        .rect();
    let base_map = app
        .harness
        .get_by_role_and_label(egui::accesskit::Role::TreeItem, "Base map")
        .rect();
    assert!(terrain.bottom() <= base_map.top() + 1.0, "{terrain:?}");
    assert!(app.data.borrow().settings.terrain_enabled);
    assert!(app.data.borrow().globe.terrain_enabled());
    assert!(
        app.data
            .borrow()
            .globe
            .credits()
            .iter()
            .any(|credit| credit.contains("Mapzen Terrain Tiles"))
    );
}

#[test]
fn the_terrain_eye_toggles_the_globe_and_persists() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness
        .get_by_label("Toggle layer Terrain (3D)")
        .click();
    app.harness.run();
    assert!(!app.data.borrow().settings.terrain_enabled);
    assert!(!app.data.borrow().globe.terrain_enabled());
    assert!(!app.settings.load().settings.terrain_enabled);
    assert!(
        !app.data
            .borrow()
            .globe
            .credits()
            .iter()
            .any(|credit| credit.contains("Mapzen Terrain Tiles")),
        "no credit for data that is not drawn"
    );

    app.harness
        .get_by_label("Toggle layer Terrain (3D)")
        .click();
    app.harness.run();
    assert!(app.data.borrow().globe.terrain_enabled());
    assert!(app.settings.load().settings.terrain_enabled);
}

#[test]
fn the_terrain_choice_survives_a_restart() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness
        .get_by_label("Toggle layer Terrain (3D)")
        .click();
    app.harness.run();
    let saved = app.settings.load();
    assert!(!saved.settings.terrain_enabled);

    let Some(mut reopened) = make_app(egui::vec2(1000.0, 800.0), None, saved) else {
        return;
    };
    reopened.harness.run();
    assert!(!reopened.data.borrow().globe.terrain_enabled());
    assert!(!reopened.data.borrow().settings.terrain_enabled);
}

#[test]
fn terrain_does_not_touch_the_overlay_layers() {
    let Some(mut app) = app() else {
        return;
    };
    app.harness.run();
    app.harness.get_by_label("Toggle layer OpenSeaMap").click();
    app.harness.run();
    app.harness
        .get_by_label("Toggle layer Terrain (3D)")
        .click();
    app.harness.run();
    let saved = app.settings.load().settings;
    assert_eq!(saved.overlay_layers.len(), 1);
    assert!(saved.overlay_layers[0].enabled);
    assert!(!saved.terrain_enabled);
}
