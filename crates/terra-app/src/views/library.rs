//! Left panel: the folder and placemark tree, search, and drag-and-drop.

use eframe::egui;

use super::layers::{layers_height, layers_section, section_toggle};
use super::tree::{self, IconKind, NavAction, NavKey, RowControl, RowKind, RowSpec, VisibleRow};
use super::{DragPayload, tr};
use crate::i18n::I18n;
use crate::{AppData, ConfirmAction, theme};
use terra_core::{Patch, PlacemarkPatch};

const MIN_PLACES_HEIGHT: f32 = 120.0;

pub(super) fn left_panel(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let places = tr(i18n, "folders.placesHeader").to_string();
    let new_folder = tr(i18n, "folders.newFolder").to_string();
    let search_hint = tr(i18n, "folders.searchPlaceholder").to_string();
    let nothing = tr(i18n, "geocoding.noResults").to_string();

    egui::Panel::left("library")
        .resizable(true)
        .default_size(280.0)
        .show(ui, |ui| {
            let panel_height = ui.available_height();
            let places_height = (panel_height - layers_height(ui)).max(MIN_PLACES_HEIGHT);
            ui.scope(|ui| {
                ui.set_max_height(places_height);
                let mut open = true;
                ui.horizontal(|ui| {
                    open = section_toggle(ui, i18n, &places, "places");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(new_folder).clicked() {
                            let name = data.store.library().default_folder_name(None);
                            match data.store.create_folder(&name, None) {
                                Ok(folder) => {
                                    data.renaming = Some((folder.id.clone(), folder.name.clone()));
                                    data.renaming_new_folder = true;
                                    data.selected_folder = Some(folder.id);
                                    data.autosave();
                                }
                                Err(error) => data.notify_error(error.to_string()),
                            }
                        }
                    });
                });
                if open {
                    ui.add(
                        egui::TextEdit::singleline(&mut data.search)
                            .hint_text(search_hint)
                            .desired_width(f32::INFINITY),
                    );
                    ui.separator();
                    tree_body(data, i18n, ui, &nothing);
                }
            });
            layers_section(data, i18n, ui, panel_height);

            rename_window(data, i18n, ui.ctx());
        });
}

/// Name prompt for renaming a folder or placemark, and for a freshly created folder.
fn rename_window(data: &mut AppData, i18n: &I18n, ctx: &egui::Context) {
    let opened_for = egui::Id::unique("rename-opened-for");
    let Some((id, mut text)) = data.renaming.clone() else {
        ctx.data_mut(|d| d.remove_temp::<String>(opened_for));
        return;
    };
    let first_frame =
        ctx.data(|d| d.get_temp::<String>(opened_for)).as_deref() != Some(id.as_str());
    ctx.data_mut(|d| d.insert_temp(opened_for, id.clone()));

    let title = tr(i18n, "folders.rename");
    let confirm_label = tr(
        i18n,
        if data.renaming_new_folder {
            "folders.create"
        } else {
            "folders.rename"
        },
    );
    let field_id = egui::Id::unique("rename-field");
    let mut submit = false;
    let mut cancel = false;
    egui::Window::new(title)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::Vec2::ZERO)
        .show(ctx, |ui| {
            let mut output = egui::TextEdit::singleline(&mut text)
                .id(field_id)
                .hint_text(tr(i18n, "folders.folderNamePlaceholder"))
                .show(ui);
            ctx.accesskit_node_builder(output.response.id, |node| node.set_label(title));
            if first_frame {
                output.response.request_focus();
                output
                    .state
                    .cursor
                    .set_char_range(Some(egui::text::CCursorRange::two(
                        egui::text::CCursor::new(0),
                        egui::text::CCursor::new(text.chars().count()),
                    )));
                output.state.store(ctx, field_id);
            }
            let valid = !text.trim().is_empty();
            let enter = output.response.lost_focus()
                && ui.input(|input| input.key_pressed(egui::Key::Enter));
            ui.horizontal(|ui| {
                let clicked = ui
                    .add_enabled(valid, egui::Button::new(confirm_label))
                    .clicked();
                submit = valid && (clicked || enter);
                cancel = ui.button(tr(i18n, "common.cancel")).clicked()
                    || ui.input(|input| input.key_pressed(egui::Key::Escape));
            });
        });
    if submit {
        let result = if data.store.library().folder(&id).is_some() {
            data.store.rename_folder(&id, text.clone())
        } else if data.store.library().placemark(&id).is_some() {
            data.store.update_placemark(
                &id,
                PlacemarkPatch {
                    name: Patch::Set(text.clone()),
                    ..Default::default()
                },
            )
        } else {
            Ok(())
        };
        match result {
            Ok(()) => {
                data.sync_editor();
                data.renaming = None;
                data.renaming_new_folder = false;
                data.autosave();
            }
            Err(error) => data.notify_error(error.to_string()),
        }
    } else if cancel {
        data.renaming = None;
        data.renaming_new_folder = false;
    } else {
        data.renaming = Some((id, text));
    }
}

fn tree_focus_id() -> egui::Id {
    egui::Id::unique("places-tree-focus")
}

fn set_tree_focus(ui: &egui::Ui, focused: bool) {
    ui.data_mut(|d| d.insert_temp(tree_focus_id(), focused));
    if focused {
        // The tree is not focusable, so a text field would otherwise keep the keyboard.
        ui.memory_mut(|memory| {
            if let Some(id) = memory.focused() {
                memory.surrender_focus(id);
            }
        });
    }
}

fn tree_body(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui, nothing: &str) {
    let query = data.search.to_lowercase();
    let rows = tree::visible_rows(data.store.library(), &query);

    let keyboard_free = data.confirm.is_none() && data.modal.is_none() && data.renaming.is_none();
    let focused = ui
        .data(|d| d.get_temp::<bool>(tree_focus_id()))
        .unwrap_or(false)
        && !ui.ctx().egui_wants_keyboard_input()
        && keyboard_free;
    if focused {
        handle_keys(data, ui, &rows);
    }

    let panel_rect = ui.max_rect();
    let pressed_outside = ui.input(|input| {
        input.pointer.any_pressed()
            && input
                .pointer
                .interact_pos()
                .is_some_and(|pos| !panel_rect.contains(pos))
    });
    if pressed_outside {
        set_tree_focus(ui, false);
    }

    egui::ScrollArea::vertical()
        .id_salt("places-scroll")
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            if rows.is_empty() && !query.is_empty() {
                ui.label(egui::RichText::new(nothing).color(theme::TEXT_MUTED));
            }
            for row in &rows {
                match row.kind {
                    RowKind::Folder => folder_row(data, i18n, ui, row),
                    RowKind::Placemark => placemark_row(data, i18n, ui, row),
                }
            }

            // Dropping on the empty area moves to the root. A response-based
            // target: wrapping the rows would swallow their clicks.
            let remaining = (ui.clip_rect().bottom() - ui.cursor().top()).max(24.0);
            let rest = ui.allocate_response(
                egui::vec2(ui.available_width(), remaining),
                egui::Sense::hover(),
            );
            if rest.dnd_hover_payload::<DragPayload>().is_some() {
                ui.painter().rect_stroke(
                    rest.rect.shrink(2.0),
                    3.0,
                    egui::Stroke::new(1.0, theme::ACCENT),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(payload) = rest.dnd_release_payload::<DragPayload>() {
                apply_drop(data, &payload, None);
            }
        });
}

fn current_id(data: &AppData) -> Option<String> {
    data.selected.clone().or(data.selected_folder.clone())
}

fn handle_keys(data: &mut AppData, ui: &egui::Ui, rows: &[VisibleRow]) {
    use egui::{Key, Modifiers};
    let current = current_id(data);
    let navigation = [
        (Key::ArrowUp, NavKey::Up),
        (Key::ArrowDown, NavKey::Down),
        (Key::ArrowLeft, NavKey::Left),
        (Key::ArrowRight, NavKey::Right),
        (Key::Enter, NavKey::Enter),
    ];
    for (key, nav) in navigation {
        if ui.input_mut(|input| input.consume_key(Modifiers::NONE, key)) {
            let action = tree::next_selection(rows, current.as_deref(), nav);
            apply_nav(data, action);
            return;
        }
    }
    let Some(id) = current else {
        return;
    };
    if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::F2)) {
        if let Some(name) = row_name(data, &id) {
            data.renaming = Some((id, name));
        }
    } else if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Delete)) {
        if data.store.library().folder(&id).is_some() {
            data.confirm = Some(ConfirmAction::DeleteFolder(id));
        } else if data.store.library().placemark(&id).is_some() {
            data.confirm = Some(ConfirmAction::DeletePlacemark(id));
        }
    }
}

fn row_name(data: &AppData, id: &str) -> Option<String> {
    let library = data.store.library();
    library
        .folder(id)
        .map(|folder| folder.name.clone())
        .or_else(|| {
            library
                .placemark(id)
                .map(|placemark| placemark.name.clone())
        })
}

fn apply_nav(data: &mut AppData, action: NavAction) {
    match action {
        NavAction::None => {}
        NavAction::Select(id) => select_row(data, &id),
        NavAction::Collapse(id) => set_open(data, &id, false),
        NavAction::Expand(id) => set_open(data, &id, true),
        NavAction::FlyTo(id) => fly_to_row(data, &id),
    }
}

fn set_open(data: &mut AppData, id: &str, open: bool) {
    if let Err(error) = data.store.set_folder_open(id, open) {
        data.notify_error(error.to_string());
    }
}

fn select_row(data: &mut AppData, id: &str) {
    if data.store.library().folder(id).is_some() {
        data.select(None);
        data.selected_folder = Some(id.to_owned());
    } else if let Some(folder_id) = data
        .store
        .library()
        .placemark(id)
        .map(|placemark| placemark.folder_id.clone())
    {
        data.select(Some(id.to_owned()));
        data.selected_folder = folder_id;
    }
}

fn fly_to_row(data: &mut AppData, id: &str) {
    let library = data.store.library();
    let geometry = library
        .placemark(id)
        .or_else(|| library.placemarks_in(Some(id)).first().copied())
        .map(|placemark| placemark.geometry.clone());
    if let Some(geometry) = geometry {
        let now = data.now_seconds();
        data.globe.fly_to_geometry(&geometry, now);
    }
}

/// Scrolls a newly selected row into view once, so manual scrolling is not undone every frame.
fn scroll_if_newly_selected(ui: &egui::Ui, row: &egui::Response, id: &str, selected: bool) {
    let memory_id = egui::Id::unique("places-tree-scrolled-to");
    if !selected {
        return;
    }
    let last = ui.data(|d| d.get_temp::<String>(memory_id));
    if last.as_deref() != Some(id) {
        row.scroll_to_me(None);
        ui.data_mut(|d| d.insert_temp(memory_id, id.to_owned()));
    }
}

fn folder_row(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui, row: &VisibleRow) {
    let Some(folder) = data.store.library().folder(&row.id).cloned() else {
        return;
    };
    let inherited_visible = crate::folder_is_visible(data.store.library(), &folder);
    let eye_label = i18n.t_args("folders.toggleFolderVisibility", &[("name", &folder.name)]);
    let chevron_label = i18n.t_args(
        if row.open {
            "folders.collapse"
        } else {
            "folders.expand"
        },
        &[("name", &folder.name)],
    );
    let selected = data.selected.is_none() && data.selected_folder.as_deref() == Some(&folder.id);
    let spec = RowSpec {
        id: egui::Id::unique(("places-row", &folder.id)),
        depth: row.depth,
        ancestors_have_sibling_below: &row.ancestors_have_sibling_below,
        has_sibling_below: row.has_sibling_below,
        has_children: row.has_children,
        open: row.open,
        icon: if row.open && row.has_children {
            IconKind::FolderOpen
        } else {
            IconKind::FolderClosed
        },
        tint: None,
        label: &folder.name,
        selected,
        dimmed: !inherited_visible,
        visible: folder.visibility,
        control: RowControl::Eye,
        eye_label: &eye_label,
        chevron_label: &chevron_label,
        accepts_drop: true,
    };
    let out = tree::tree_row(ui, &spec);
    scroll_if_newly_selected(ui, &out.row, &folder.id, selected);

    if out.eye_clicked {
        set_tree_focus(ui, true);
        if let Err(error) = data
            .store
            .set_folder_visibility(&folder.id, !folder.visibility)
        {
            data.notify_error(error.to_string());
        }
    } else if out.chevron_clicked {
        set_tree_focus(ui, true);
        set_open(data, &folder.id, !row.open);
    } else if out.row.clicked() {
        set_tree_focus(ui, true);
        select_row(data, &folder.id);
    }
    if out.row.double_clicked() && !out.eye_clicked && !out.chevron_clicked {
        fly_to_row(data, &folder.id);
    }
    out.row
        .dnd_set_drag_payload(DragPayload::Folder(folder.id.clone()));
    if let Some(payload) = out.row.dnd_release_payload::<DragPayload>() {
        apply_drop(data, &payload, Some(folder.id.clone()));
    }
    out.row.context_menu(|ui| {
        if ui.button(tr(i18n, "folders.rename")).clicked() {
            data.renaming = Some((folder.id.clone(), folder.name.clone()));
            ui.close();
        }
        if ui.button(tr(i18n, "folders.newFolder")).clicked() {
            let name = data.store.library().default_folder_name(Some(&folder.id));
            match data.store.create_folder(&name, Some(folder.id.clone())) {
                Ok(child) => {
                    data.renaming = Some((child.id.clone(), child.name.clone()));
                    data.renaming_new_folder = true;
                    data.autosave();
                }
                Err(error) => data.notify_error(error.to_string()),
            }
            ui.close();
        }
        if ui.button(tr(i18n, "folders.delete")).clicked() {
            data.confirm = Some(ConfirmAction::DeleteFolder(folder.id.clone()));
            ui.close();
        }
        move_menu(
            data,
            i18n,
            ui,
            &DragPayload::Folder(folder.id.clone()),
            true,
        );
    });
}

fn placemark_row(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui, row: &VisibleRow) {
    let Some(placemark) = data.store.library().placemark(&row.id).cloned() else {
        return;
    };
    let library = data.store.library();
    let inherited_visible = library.is_placemark_visible(&placemark);
    let icon = tree::icon_for_geometry(&placemark.geometry);
    let tint = placemark
        .style_id
        .as_deref()
        .and_then(|id| library.styles.get(id))
        .map(|style| {
            let hex = if style.filled && icon != IconKind::Line {
                &style.fill_color
            } else {
                &style.outline_color
            };
            theme::color32_from_hex(hex)
        });
    let eye_label = i18n.t_args(
        "folders.togglePlacemarkVisibility",
        &[("name", &placemark.name)],
    );
    let selected = data.selected.as_deref() == Some(placemark.id.as_str());
    let spec = RowSpec {
        id: egui::Id::unique(("places-row", &placemark.id)),
        depth: row.depth,
        ancestors_have_sibling_below: &row.ancestors_have_sibling_below,
        has_sibling_below: row.has_sibling_below,
        has_children: false,
        open: false,
        icon,
        tint,
        label: &placemark.name,
        selected,
        dimmed: !inherited_visible,
        visible: placemark.visibility,
        control: RowControl::Eye,
        eye_label: &eye_label,
        chevron_label: "",
        accepts_drop: true,
    };
    let out = tree::tree_row(ui, &spec);
    scroll_if_newly_selected(ui, &out.row, &placemark.id, selected);

    if out.eye_clicked {
        set_tree_focus(ui, true);
        if let Err(error) = data
            .store
            .set_placemark_visibility(&placemark.id, !placemark.visibility)
        {
            data.notify_error(error.to_string());
        }
    } else if out.row.clicked() {
        set_tree_focus(ui, true);
        select_row(data, &placemark.id);
    }
    if out.row.double_clicked() && !out.eye_clicked {
        fly_to_row(data, &placemark.id);
    }
    out.row
        .dnd_set_drag_payload(DragPayload::Placemark(placemark.id.clone()));
    if let Some(payload) = out.row.dnd_release_payload::<DragPayload>() {
        apply_drop(data, &payload, placemark.folder_id.clone());
    }
    out.row.context_menu(|ui| {
        if ui.button(tr(i18n, "folders.rename")).clicked() {
            data.renaming = Some((placemark.id.clone(), placemark.name.clone()));
            ui.close();
        }
        if ui.button(tr(i18n, "folders.delete")).clicked() {
            data.confirm = Some(ConfirmAction::DeletePlacemark(placemark.id.clone()));
            ui.close();
        }
        move_menu(
            data,
            i18n,
            ui,
            &DragPayload::Placemark(placemark.id.clone()),
            false,
        );
    });
}

fn move_menu(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    payload: &DragPayload,
    hide_self: bool,
) {
    ui.menu_button("Move to", |ui| {
        if ui.button("Root").clicked() {
            apply_drop(data, payload, None);
            ui.close();
        }
        let folders: Vec<(String, String)> = data
            .store
            .library()
            .folders
            .iter()
            .filter(|folder| !(hide_self && payload_id(payload) == folder.id))
            .map(|folder| (folder.id.clone(), folder.name.clone()))
            .collect();
        for (id, name) in folders {
            if ui.button(name).clicked() {
                apply_drop(data, payload, Some(id));
                ui.close();
            }
        }
    });
    let _ = i18n;
}

fn payload_id(payload: &DragPayload) -> &str {
    match payload {
        DragPayload::Placemark(id) | DragPayload::Folder(id) => id,
    }
}

fn apply_drop(data: &mut AppData, payload: &DragPayload, folder: Option<String>) {
    let result = match payload {
        DragPayload::Placemark(id) => data.store.move_placemark(id, folder),
        DragPayload::Folder(id) => data.store.move_folder(id, folder),
    };
    match result {
        Ok(()) => data.autosave(),
        Err(error) => data.notify_error(error.to_string()),
    }
}
