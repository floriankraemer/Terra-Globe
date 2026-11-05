//! Headless UI automation with `egui_kittest`.
//!
//! These drive the real widget tree through AccessKit (clicks by label) and
//! assert on the application state, which is the closest thing to the Playwright
//! suite the web app had.

mod harness;

use egui_kittest::kittest::Queryable;
use harness::{DOCUMENT, make};
use terra_app::ModalKind;
use terra_globe::DrawTool;

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
    harness.get_by_label("Circle").click();
    harness.run();
    assert_eq!(data.borrow().drawing.tool(), Some(DrawTool::Circle));

    harness.get_by_label("Polygon").click();
    harness.run();
    assert_eq!(data.borrow().drawing.tool(), Some(DrawTool::Polygon));
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

    // The tree shows the folder toggle and then the placemark toggle; hide the
    // placemark (the second one).
    let count = harness.get_all_by_label("ON").count();
    assert!(count >= 2, "expected folder and placemark toggles");
    {
        let mut toggles = harness.get_all_by_label("ON");
        toggles.nth(1).expect("placemark toggle").click();
    }
    harness.run();

    let data = data.borrow();
    let placemark = data.store.library().placemark("p1").expect("placemark");
    assert!(!placemark.visibility);
    assert!(data.store.library().visible_placemarks().is_empty());
}
