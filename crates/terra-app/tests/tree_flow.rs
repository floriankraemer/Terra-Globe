//! Headless automation of the Places tree: eye toggle, chevron, keyboard and drag-and-drop.

#[allow(dead_code)]
mod harness;

use std::cell::RefCell;
use std::rc::Rc;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use harness::make;
use terra_app::{AppData, ConfirmAction};

const TREE: &str = r#"<kml><Document>
<Folder id="trips"><name>Trips</name>
  <Placemark id="p1"><name>Berlin</name><Point><coordinates>13.405,52.52,0</coordinates></Point></Placemark>
  <Placemark id="p2"><name>Paris</name><Point><coordinates>2.352,48.857,0</coordinates></Point></Placemark>
</Folder>
<Folder id="work"><name>Work</name></Folder>
</Document></kml>"#;

const NESTED: &str = r#"<kml><Document>
<Folder id="outer"><name>Outer</name><Folder id="inner"><name>Inner</name>
  <Placemark id="p1"><name>Berlin</name><Point><coordinates>13.405,52.52,0</coordinates></Point></Placemark>
</Folder></Folder>
</Document></kml>"#;

type Fixture = (Harness<'static>, Rc<RefCell<AppData>>);

fn tree(document: &str) -> Option<Fixture> {
    let fixture = make(Some(document));
    if fixture.is_none() {
        eprintln!("no GPU adapter; skipping tree test");
    }
    fixture
}

fn center(harness: &Harness, label: &str) -> egui::Pos2 {
    harness.get_by_label(label).rect().center()
}

fn drag_row(harness: &mut Harness, from: egui::Pos2, to: egui::Pos2) {
    harness.hover_at(from);
    harness.run_steps(2);
    harness.drag_at(from);
    harness.run_steps(2);
    for step in 1..=6 {
        harness.hover_at(from + (to - from) * (step as f32 / 6.0));
        harness.run_steps(1);
    }
    harness.drop_at(to);
    harness.run_steps(3);
}

#[test]
fn eye_toggle_hides_and_shows_a_placemark() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Toggle visibility of Berlin").click();
    harness.run();
    assert!(
        !data
            .borrow()
            .store
            .library()
            .placemark("p1")
            .unwrap()
            .visibility
    );

    harness.get_by_label("Toggle visibility of Berlin").click();
    harness.run();
    assert!(
        data.borrow()
            .store
            .library()
            .placemark("p1")
            .unwrap()
            .visibility
    );
}

#[test]
fn eye_toggle_does_not_select_the_row() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Toggle visibility of Berlin").click();
    harness.run();
    assert_eq!(data.borrow().selected, None);
}

#[test]
fn chevron_collapses_and_expands_a_folder() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    assert!(harness.query_by_label("Berlin").is_some());

    harness.get_by_label("Collapse Trips").click();
    harness.run();
    assert!(harness.query_by_label("Berlin").is_none());
    assert_eq!(
        data.borrow().store.library().folder("trips").unwrap().open,
        Some(false)
    );

    harness.get_by_label("Expand Trips").click();
    harness.run();
    assert!(harness.query_by_label("Berlin").is_some());
}

#[test]
fn clicking_a_folder_selects_it_without_collapsing() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Berlin").click();
    harness.run();
    harness.get_by_label("Trips").click();
    harness.run();
    let data = data.borrow();
    assert_eq!(data.selected_folder.as_deref(), Some("trips"));
    assert_eq!(data.selected, None);
    drop(data);
    assert!(harness.query_by_label("Berlin").is_some());
}

#[test]
fn keyboard_down_then_enter_flies_to_the_next_placemark() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Berlin").click();
    harness.run();
    let before = data.borrow().globe.pose();

    harness.key_press(egui::Key::ArrowDown);
    harness.run();
    assert_eq!(data.borrow().selected.as_deref(), Some("p2"));

    harness.key_press(egui::Key::Enter);
    harness.run_steps(120);
    let after = data.borrow().globe.pose();
    assert_ne!(before.lon_deg, after.lon_deg);
    assert!((after.lon_deg - 2.352).abs() < 0.5, "lon {}", after.lon_deg);
    assert!(
        (after.lat_deg - 48.857).abs() < 0.5,
        "lat {}",
        after.lat_deg
    );
}

#[test]
fn keyboard_left_and_right_collapse_and_expand() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Trips").click();
    harness.run();
    harness.key_press(egui::Key::ArrowLeft);
    harness.run();
    assert_eq!(
        data.borrow().store.library().folder("trips").unwrap().open,
        Some(false)
    );
    harness.key_press(egui::Key::ArrowRight);
    harness.run();
    assert_eq!(
        data.borrow().store.library().folder("trips").unwrap().open,
        Some(true)
    );
}

#[test]
fn keys_do_nothing_until_the_tree_was_clicked() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.key_press(egui::Key::ArrowDown);
    harness.key_press(egui::Key::Delete);
    harness.run();
    let data = data.borrow();
    assert_eq!(data.selected, None);
    assert_eq!(data.selected_folder, None);
    assert_eq!(data.confirm, None);
}

#[test]
fn delete_and_f2_act_on_the_selection() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Berlin").click();
    harness.run();
    harness.key_press(egui::Key::F2);
    harness.run();
    assert_eq!(
        data.borrow().renaming,
        Some(("p1".to_owned(), "Berlin".to_owned()))
    );
    harness.get_by_label("Cancel").click();
    harness.run();

    harness.get_by_label("Berlin").click();
    harness.run();
    harness.key_press(egui::Key::Delete);
    harness.run();
    assert_eq!(
        data.borrow().confirm,
        Some(ConfirmAction::DeletePlacemark("p1".to_owned()))
    );
}

#[test]
fn double_click_flies_to_the_placemark() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    let before = data.borrow().globe.pose();
    let row = harness.get_by_label("Paris");
    row.click();
    row.click();
    harness.run_steps(120);
    let after = data.borrow().globe.pose();
    assert_ne!(before.lon_deg, after.lon_deg);
    assert!((after.lon_deg - 2.352).abs() < 0.5, "lon {}", after.lon_deg);
}

#[test]
fn search_with_no_match_shows_the_empty_text() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    data.borrow_mut().search = "zzz".into();
    harness.run();
    assert!(harness.query_by_label("Berlin").is_none());
    assert!(harness.query_by_label("No results").is_some());
}

#[test]
fn dragging_a_placemark_onto_a_folder_moves_it() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    let from = center(&harness, "Berlin");
    let to = center(&harness, "Work");
    drag_row(&mut harness, from, to);
    let data = data.borrow();
    let placemark = data.store.library().placemark("p1").unwrap();
    assert_eq!(placemark.folder_id.as_deref(), Some("work"));
}

#[test]
fn dropping_on_the_empty_area_moves_to_the_root() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    let from = center(&harness, "Berlin");
    let below_last_row = center(&harness, "Work") + egui::vec2(0.0, 40.0);
    drag_row(&mut harness, from, below_last_row);
    let data = data.borrow();
    assert_eq!(
        data.store.library().placemark("p1").unwrap().folder_id,
        None
    );
}

#[test]
fn a_plain_click_never_starts_a_drag() {
    let Some((mut harness, data)) = tree(TREE) else {
        return;
    };
    harness.run();
    harness.get_by_label("Berlin").click();
    harness.run();
    let data = data.borrow();
    assert_eq!(data.selected.as_deref(), Some("p1"));
    assert_eq!(
        data.store
            .library()
            .placemark("p1")
            .unwrap()
            .folder_id
            .as_deref(),
        Some("trips")
    );
}

#[test]
fn dropping_a_folder_onto_its_descendant_is_rejected() {
    let Some((mut harness, data)) = tree(NESTED) else {
        return;
    };
    harness.run();
    let before = data.borrow().store.library().folders.clone();
    let from = center(&harness, "Outer");
    let to = center(&harness, "Inner");
    drag_row(&mut harness, from, to);
    let data = data.borrow();
    assert_eq!(data.store.library().folders, before);
    let notice = data.notice.as_ref().expect("error notice");
    assert!(notice.error);
}
