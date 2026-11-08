//! Layout and interaction regressions found by running the real app at 1360x860.

#[allow(dead_code)]
mod harness;

use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::{NodeT, Queryable};
use harness::{DOCUMENT, make_sized};

const BIG: egui::Vec2 = egui::vec2(1360.0, 860.0);
const TOP_BAR: [&str; 10] = [
    "File", "View", "Undo", "Redo", "Search", "Geometry", "Ruler", "Route", "Settings", "About",
];

fn rect(harness: &Harness, label: &str) -> egui::Rect {
    harness.get_by_label_contains(label).rect()
}

// 1. top bar

#[test]
fn top_bar_is_one_row_when_everything_fits() {
    let Some((mut harness, _data)) = make_sized(BIG, None) else {
        return;
    };
    harness.run();
    let rects: Vec<_> = TOP_BAR.iter().map(|l| rect(&harness, l)).collect();
    let y = rects[0].center().y;
    for (label, r) in TOP_BAR.iter().zip(&rects) {
        assert!(
            (r.center().y - y).abs() < 1.0,
            "{label} is on another row: {r:?} vs y {y}"
        );
    }
    let top = rects.iter().map(|r| r.top()).fold(f32::MAX, f32::min);
    let bottom = rects.iter().map(|r| r.bottom()).fold(f32::MIN, f32::max);
    let panel_bottom = harness.get_by_label("Places").rect().top();
    assert!(
        panel_bottom - top < (bottom - top) + 24.0,
        "top panel is taller than one row: content {top}..{bottom}, next at {panel_bottom}"
    );
    assert!(
        rects.last().unwrap().right() > 1200.0,
        "right group is right-aligned"
    );
}

#[test]
fn top_bar_wraps_without_overlap_when_narrow() {
    let Some((mut harness, _data)) = make_sized(egui::vec2(640.0, 720.0), None) else {
        return;
    };
    harness.run();
    let rects: Vec<_> = TOP_BAR.iter().map(|l| rect(&harness, l)).collect();
    let y = rects[0].center().y;
    assert!(
        rects.iter().any(|r| (r.center().y - y).abs() > 10.0),
        "640 px cannot hold one row, so it must wrap"
    );
    for (i, a) in rects.iter().enumerate() {
        for b in &rects[i + 1..] {
            assert!(!a.intersects(*b), "{a:?} overlaps {b:?}");
        }
    }
}

// 2. left panel

fn many_placemarks(count: usize) -> String {
    let mut doc = String::from("<kml><Document>");
    for i in 0..count {
        doc.push_str(&format!(
            "<Placemark id=\"m{i}\"><name>Spot {i}</name><Point><coordinates>{},10,0</coordinates></Point></Placemark>",
            i % 170
        ));
    }
    doc.push_str("</Document></kml>");
    doc
}

#[test]
fn all_layer_rows_are_visible_at_1360x860() {
    for document in [None, Some(many_placemarks(60))] {
        let Some((mut harness, _data)) = make_sized(BIG, document.as_deref()) else {
            return;
        };
        harness.run();
        // Selecting the last overlay row opens its opacity slider below it, so
        // the section is as tall as it gets, with its final row at the bottom.
        let last_name = terra_providers::overlay_presets()
            .last()
            .expect("overlay presets")
            .name
            .clone();
        harness.get_by_label(&last_name).click();
        harness.run();
        harness.run();
        let panel_bottom = 860.0;
        let last = harness.get_by_role_and_label(Role::TreeItem, &last_name);
        assert!(
            last.rect().bottom() <= panel_bottom - 4.0,
            "{:?}",
            last.rect()
        );
        let opacity = harness.get_by_role_and_label(Role::Slider, "Opacity");
        assert!(
            opacity.rect().bottom() <= panel_bottom,
            "{:?}",
            opacity.rect()
        );
        let first = harness.get_by_role_and_label(Role::TreeItem, "Base map");
        let places = harness.get_by_label("Places").rect();
        assert!(first.rect().top() > places.bottom());
        // The section takes exactly its content height, flush with the panel
        // bottom, and Places keeps the space above it.
        assert!(
            opacity.rect().bottom() >= panel_bottom - 16.0,
            "{:?}",
            opacity.rect()
        );
        let layers_top = harness.get_by_label("Layers").rect().top();
        assert!(layers_top > 400.0 && layers_top < 640.0, "{layers_top}");
        if document.is_some() {
            let spot = harness
                .get_by_role_and_label(Role::TreeItem, "Spot 0")
                .rect();
            assert!(spot.bottom() < layers_top);
        }
    }
}

#[test]
fn layers_never_take_more_than_half_the_panel() {
    let Some((mut harness, _data)) = make_sized(egui::vec2(1000.0, 420.0), None) else {
        return;
    };
    harness.run();
    harness.run();
    let top = harness.get_by_label("Layers").rect().top();
    assert!(
        top >= 420.0 * 0.45,
        "layers start at {top}, above half of 420"
    );
}

#[test]
fn collapsing_layers_gives_the_space_back_to_places() {
    let Some((mut harness, _data)) = make_sized(BIG, Some(&many_placemarks(60))) else {
        return;
    };
    harness.run();
    harness.run();
    let before = harness.get_by_label("Layers").rect().top();
    harness.get_by_label("Collapse Layers").click();
    harness.run();
    harness.run();
    let after = harness.get_by_label("Layers").rect().top();
    assert!(after > before + 100.0, "layers header {before} -> {after}");
}

// 4. selection

#[test]
fn only_the_selected_row_is_marked_selected() {
    let Some((mut harness, _data)) = make_sized(BIG, None) else {
        return;
    };
    harness.run();
    let selected = |h: &Harness, name: &str| {
        h.get_by_role_and_label(Role::TreeItem, name)
            .accesskit_node()
            .toggled()
            .map(|t| t == egui::accesskit::Toggled::True)
    };
    harness.get_by_label("Waymarked Trails: Cycling").click();
    harness.run();
    assert_eq!(selected(&harness, "Waymarked Trails: Cycling"), Some(true));
    assert_ne!(selected(&harness, "OpenStreetMap"), Some(true));
    harness.get_by_label("OpenTopoMap").click();
    harness.run();
    assert_eq!(selected(&harness, "OpenTopoMap"), Some(true));
    assert_ne!(selected(&harness, "Waymarked Trails: Cycling"), Some(true));
    assert_ne!(selected(&harness, "OpenStreetMap"), Some(true));
}

// 5. opacity row

#[test]
fn opacity_slider_fills_the_row_with_label_first() {
    let Some((mut harness, _data)) = make_sized(BIG, None) else {
        return;
    };
    harness.run();
    harness.get_by_label("Waymarked Trails: Cycling").click();
    harness.run();
    harness.run();
    let slider = harness.get_by_role_and_label(Role::Slider, "Opacity");
    let label = harness.get_by_role_and_label(Role::Label, "Opacity");
    let item = harness
        .get_by_role_and_label(Role::TreeItem, "Waymarked Trails: Cycling")
        .rect();
    assert!(
        label.rect().right() <= slider.rect().left(),
        "label before slider"
    );
    let value = harness
        .get_by_role_and_label(Role::SpinButton, "Opacity")
        .rect();
    assert!(
        value.left() >= slider.rect().right() && value.right() <= item.right(),
        "{value:?}"
    );
    assert!(label.rect().left() > item.left() + 16.0, "indented");
    assert!(
        slider.rect().left() - label.rect().right() <= 8.0
            && item.right() - value.right() <= 12.0
            && slider.rect().width() >= item.width() * 0.4,
        "slider {:?} must fill the row {:?} between label and value",
        slider.rect(),
        item
    );
}

// 3. attribution

// (unit tests live next to merge_credits in views/globe_view.rs)

// 6. rename

fn open_rename(harness: &mut Harness, name: &str) {
    harness.get_by_role_and_label(Role::TreeItem, name).click();
    harness.run();
    harness.key_press(egui::Key::F2);
    harness.run();
    harness.run();
}

fn name_field<'a>(harness: &'a Harness) -> egui_kittest::Node<'a> {
    harness.get_by_role_and_label(Role::TextInput, "Rename")
}

#[test]
fn rename_dialog_is_centred_focused_and_selected() {
    let Some((mut harness, data)) = make_sized(egui::vec2(800.0, 600.0), Some(DOCUMENT)) else {
        return;
    };
    harness.run();
    open_rename(&mut harness, "Berlin");
    let field = name_field(&harness);
    let r = field.rect();
    assert!((r.center().x - 400.0).abs() < 120.0, "not centred: {r:?}");
    assert!(
        r.top() > rect(&harness, "Undo").bottom() + 20.0,
        "over the toolbar: {r:?}"
    );
    field.type_text("Potsdam");
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Rename")
        .click();
    harness.run();
    let data = data.borrow();
    assert_eq!(
        data.store.library().placemark("p1").unwrap().name,
        "Potsdam"
    );
    assert!(data.renaming.is_none());
}

#[test]
fn rename_folder_with_enter() {
    let Some((mut harness, data)) = make_sized(egui::vec2(800.0, 600.0), Some(DOCUMENT)) else {
        return;
    };
    harness.run();
    open_rename(&mut harness, "Places");
    name_field(&harness).type_text("Trips");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let data = data.borrow();
    assert_eq!(data.store.library().folder("f1").unwrap().name, "Trips");
    assert!(data.renaming.is_none());
}

#[test]
fn rename_cancel_and_escape_keep_the_old_name() {
    let Some((mut harness, data)) = make_sized(egui::vec2(800.0, 600.0), Some(DOCUMENT)) else {
        return;
    };
    harness.run();
    open_rename(&mut harness, "Berlin");
    name_field(&harness).type_text("Nope");
    harness.run();
    harness
        .get_by_role_and_label(Role::Button, "Cancel")
        .click();
    harness.run();
    assert_eq!(
        data.borrow().store.library().placemark("p1").unwrap().name,
        "Berlin"
    );
    assert!(data.borrow().renaming.is_none());

    open_rename(&mut harness, "Berlin");
    name_field(&harness).type_text("Nope");
    harness.key_press(egui::Key::Escape);
    harness.run();
    harness.run();
    assert_eq!(
        data.borrow().store.library().placemark("p1").unwrap().name,
        "Berlin"
    );
    assert!(data.borrow().renaming.is_none());
}

#[test]
fn empty_name_is_rejected_and_keeps_the_dialog_open() {
    let Some((mut harness, data)) = make_sized(egui::vec2(800.0, 600.0), Some(DOCUMENT)) else {
        return;
    };
    harness.run();
    open_rename(&mut harness, "Berlin");
    harness.key_press(egui::Key::Backspace);
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();
    harness.run();
    let data = data.borrow();
    assert_eq!(data.store.library().placemark("p1").unwrap().name, "Berlin");
    assert!(
        data.renaming.is_some(),
        "dialog stays open on a rejected name"
    );
}

#[test]
fn new_folder_dialog_keeps_the_create_button() {
    let Some((mut harness, _data)) = make_sized(egui::vec2(800.0, 600.0), None) else {
        return;
    };
    harness.run();
    harness.get_by_label("New Folder").click();
    harness.run();
    harness.get_by_role_and_label(Role::Button, "Create");
}

// 7. toast

#[test]
fn notice_toast_sits_over_the_globe_not_the_side_panel() {
    let Some((mut harness, data)) = make_sized(BIG, None) else {
        return;
    };
    harness.run();
    data.borrow_mut().notify("Toast message");
    harness.run();
    harness.run();
    let toast = harness
        .get_by_label("Toast message")
        .rect()
        .union(harness.get_by_label("Dismiss").rect());
    let panel_right = harness
        .get_by_role_and_label(Role::TreeItem, "Base map")
        .rect()
        .right()
        + 8.0;
    assert!(
        toast.left() > panel_right,
        "toast {toast:?} over left panel"
    );
    assert!(
        (toast.center().x - (panel_right + 1360.0) / 2.0).abs() < 12.0,
        "{toast:?}"
    );
    assert!(
        toast.bottom() < 860.0 - 20.0,
        "{toast:?} covers the attribution line"
    );
}

// 8. editor

#[test]
fn editor_style_rows_read_label_first() {
    let Some((mut harness, _data)) = make_sized(BIG, Some(DOCUMENT)) else {
        return;
    };
    harness.run();
    harness.get_by_label("Berlin").click();
    harness.run();
    harness.run();
    for (check, color, slider) in [
        ("Outline", "Outline Color", "Outline Width"),
        ("Filled", "Fill Color", "Opacity"),
    ] {
        let check = harness.get_by_role_and_label(Role::CheckBox, check).rect();
        let color = harness.get_by_label(color).rect();
        let slider = harness.get_by_role_and_label(Role::Slider, slider).rect();
        assert!((check.center().y - color.center().y).abs() < 2.0);
        assert!((check.center().y - slider.center().y).abs() < 2.0);
        assert!(check.right() <= color.left() + 60.0 && check.left() < color.left());
        assert!(color.right() <= slider.left() + 1.0, "{color:?} {slider:?}");
    }
}
