//! Headless UI automation with `egui_kittest`.
//!
//! These drive the real widget tree through AccessKit (clicks by label) and
//! assert on the application state, which is the closest thing to the Playwright
//! suite the web app had.

mod harness;

use egui_kittest::kittest::Queryable;
use harness::{DOCUMENT, make, make_sized};
use terra_app::ModalKind;
use terra_globe::{CameraPose, DrawTool, Geodetic};

#[test]
fn new_folder_button_creates_a_folder() {
    let Some((mut harness, data)) = make(None) else {
        eprintln!("no GPU adapter; skipping UI test");
        return;
    };
    harness.run();
    harness.get_by_label("New Folder").click();
    harness.run();

    let data = data.borrow();
    assert_eq!(data.store.library().folders.len(), 1);
    assert!(
        data.renaming.is_some(),
        "creating a folder opens the rename prompt"
    );
    assert!(data.store.can_undo());
}

#[test]
fn drawing_tools_update_the_session() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    for (label, tool) in [
        ("Marker", DrawTool::Marker),
        ("Line", DrawTool::Line),
        ("Polygon", DrawTool::Polygon),
        ("Circle", DrawTool::Circle),
        ("Rectangle", DrawTool::Rectangle),
    ] {
        harness.get_by_label_contains("Geometry").click();
        harness.run();
        harness.get_by_label(label).click();
        harness.run();
        assert_eq!(data.borrow().drawing.tool(), Some(tool));
        harness.get_by_label(&format!("Geometry: {label}"));
    }
}

#[test]
fn finish_and_cancel_show_only_while_drawing() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    assert!(harness.query_by_label("Finish").is_none());
    assert!(harness.query_by_label("Cancel").is_none());

    harness.get_by_label("Geometry").click();
    harness.run();
    harness.get_by_label("Polygon").click();
    harness.run();
    harness.get_by_label("Finish");
    harness.get_by_label("Cancel").click();
    harness.run();
    assert_eq!(data.borrow().drawing.tool(), None);
    assert!(harness.query_by_label("Finish").is_none());
}

#[test]
fn finishing_with_too_few_points_creates_nothing() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    harness.get_by_label("Geometry").click();
    harness.run();
    harness.get_by_label("Line").click();
    harness.run();
    harness.get_by_label("Finish").click();
    harness.run();
    assert!(data.borrow().store.library().placemarks.is_empty());
}

#[test]
fn picking_a_tool_turns_the_ruler_off_and_ruler_toggles() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    harness.get_by_label("Ruler").click();
    harness.run();
    assert!(data.borrow().ruler_adding);

    harness.get_by_label("Geometry").click();
    harness.run();
    harness.get_by_label("Circle").click();
    harness.run();
    assert!(!data.borrow().ruler_adding);
    assert_eq!(data.borrow().drawing.tool(), Some(DrawTool::Circle));

    harness.get_by_label("Ruler").click();
    harness.run();
    assert!(data.borrow().ruler_adding);
    assert_eq!(data.borrow().drawing.tool(), None);
}

#[test]
fn escape_closes_the_geometry_menu_without_starting_a_tool() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    harness.get_by_label("Geometry").click();
    harness.run();
    harness.get_by_label("Circle");
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(harness.query_by_label("Circle").is_none());
    assert_eq!(data.borrow().drawing.tool(), None);
}

#[test]
fn top_bar_fits_at_1280_and_has_no_title() {
    let Some((mut harness, _data)) = make_sized(egui::vec2(1280.0, 720.0), None) else {
        return;
    };
    harness.run();
    assert!(harness.query_by_label("Terra Globe").is_none());
    let about = harness.get_by_label("About").rect();
    assert!(about.right() <= 1280.0, "About overflows: {about:?}");
    assert!(about.left() >= 0.0);
}

#[test]
fn clicking_a_placemark_opens_the_editor() {
    let Some((mut harness, data)) = make(Some(DOCUMENT)) else {
        return;
    };
    harness.run();
    assert_eq!(data.borrow().store.library().placemarks.len(), 1);

    harness.get_by_label("Berlin").click();
    harness.run();
    let editor = data.borrow().editor.clone().expect("editor opened");
    assert_eq!(editor.name, "Berlin");
}

/// Frames the globe on Berlin and returns where its marker is drawn. In a
/// 1600x1000 window the globe rect starts at (288, 33), past the left panel and
/// top bar.
fn centre_globe_on_berlin(
    harness: &mut egui_kittest::Harness,
    data: &std::rc::Rc<std::cell::RefCell<terra_app::AppData>>,
) -> egui::Pos2 {
    const GLOBE_ORIGIN: egui::Pos2 = egui::pos2(288.0, 33.0);
    data.borrow_mut().globe.apply_pose(CameraPose {
        lon_deg: 13.405,
        lat_deg: 52.52,
        distance: 3_000_000.0,
        heading: 0.0,
        pitch: 0.0,
    });
    harness.run_steps(3);
    let screen = data
        .borrow()
        .globe
        .project(Geodetic::from_degrees(13.405, 52.52, 0.0))
        .expect("Berlin is visible");
    egui::pos2(
        GLOBE_ORIGIN.x + screen.x as f32,
        GLOBE_ORIGIN.y + screen.y as f32,
    )
}

fn click_globe(harness: &mut egui_kittest::Harness, at: egui::Pos2) {
    harness.hover_at(at);
    harness.run_steps(2);
    harness.drag_at(at);
    harness.run_steps(2);
    harness.drop_at(at);
    harness.run_steps(2);
}

/// Clicks `offset` pixels to the right of Berlin's marker in a fresh app and
/// returns the selected placemark id. A fresh app per click, because selecting
/// opens the editor, which narrows the globe and moves the marker.
fn select_by_clicking_beside_berlin(offset: f32) -> Option<String> {
    let (mut harness, data) = make_sized(egui::vec2(1600.0, 1000.0), Some(DOCUMENT))?;
    harness.run();
    let marker = centre_globe_on_berlin(&mut harness, &data);
    click_globe(&mut harness, marker + egui::vec2(offset, 0.0));
    data.borrow().selected.clone()
}

#[test]
fn clicking_30_px_from_a_marker_does_not_select_it() {
    assert_eq!(select_by_clicking_beside_berlin(30.0), None);
}

#[test]
fn clicking_12_px_from_a_marker_selects_it() {
    assert_eq!(
        select_by_clicking_beside_berlin(12.0).as_deref(),
        Some("p1")
    );
}

#[test]
fn clicking_20_px_from_a_marker_selects_it() {
    // Past the old 14 px hit radius, inside the 1.8x one.
    assert_eq!(
        select_by_clicking_beside_berlin(20.0).as_deref(),
        Some("p1")
    );
}

#[test]
fn settings_modal_opens_and_closes() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    harness.get_by_label("Settings").click();
    harness.run();
    assert_eq!(data.borrow().modal, Some(ModalKind::Settings));

    harness.get_by_label("Close").click();
    harness.run();
    assert_eq!(data.borrow().modal, None);
}

#[test]
fn undo_button_reflects_history() {
    let Some((mut harness, data)) = make(None) else {
        return;
    };
    harness.run();
    assert!(!data.borrow().store.can_undo());

    harness.get_by_label("New Folder").click();
    harness.run();
    assert!(data.borrow().store.can_undo());

    // The rename window opens over the toolbar; dismiss it before clicking Undo.
    harness.get_by_label("Cancel").click();
    harness.run();
    harness.get_by_label("Undo").click();
    harness.run();
    assert!(data.borrow().store.library().folders.is_empty());
    assert!(data.borrow().store.can_redo());
}

#[test]
fn visibility_toggle_hides_the_placemark() {
    let Some((mut harness, data)) = make(Some(DOCUMENT)) else {
        return;
    };
    harness.run();

    harness.get_by_label("Toggle visibility of Berlin").click();
    harness.run();

    let data = data.borrow();
    let placemark = data.store.library().placemark("p1").expect("placemark");
    assert!(!placemark.visibility);
    assert!(data.store.library().visible_placemarks().is_empty());
}

#[test]
fn top_bar_buttons_never_overlap() {
    for width in [640.0, 800.0, 1280.0] {
        let Some((mut harness, _data)) = make_sized(egui::vec2(width, 720.0), None) else {
            return;
        };
        harness.run();
        let window = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width, 720.0));
        let rects: Vec<(String, egui::Rect)> = [
            "File", "View", "Undo", "Redo", "Search", "Geometry", "Ruler", "Route", "Settings",
            "About",
        ]
        .iter()
        .map(|label| {
            (
                label.to_string(),
                harness.get_by_label_contains(label).rect(),
            )
        })
        .collect();
        for (i, (name, rect)) in rects.iter().enumerate() {
            assert!(
                window.contains_rect(*rect),
                "{name} outside window at {width}: {rect:?}"
            );
            for (other, other_rect) in &rects[i + 1..] {
                assert!(
                    !rect.intersects(*other_rect),
                    "{name} overlaps {other} at {width}"
                );
            }
        }
    }
}
