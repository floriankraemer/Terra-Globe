//! All UI views. Kept in one module because they share one mutable state object.

use eframe::egui;
use terra_core::{
    CoordinateFormat, GeoPoint, Patch, PlacemarkGeometry, PlacemarkPatch, UnitSystem,
    format_coordinate, format_distance,
};
use terra_globe::{Camera, DrawTool, SceneMode};

use crate::i18n::I18n;
use crate::tools::{self, RouteMode};
use crate::{AppData, ConfirmAction, ModalKind, Notice, theme};
use web_time::Instant;

fn tr<'a>(i18n: &'a I18n, key: &'a str) -> &'a str {
    i18n.t(key)
}

#[derive(Debug, Clone)]
enum DragPayload {
    Placemark(String),
    Folder(String),
}

pub fn app_ui(
    data: &mut AppData,
    i18n: &mut I18n,
    ui: &mut egui::Ui,
    render_state: Option<&egui_wgpu::RenderState>,
) {
    data.handle_jobs();
    shortcuts(data, ui.ctx());

    top_bar(data, i18n, ui);
    left_panel(data, i18n, ui);
    right_panel(data, i18n, ui);
    central_globe(data, i18n, ui, render_state);
    floating_panels(data, i18n, ui.ctx());
    modals(data, i18n, ui.ctx());
    if let Some(notice) = data.notice.clone() {
        notice_toast(data, i18n, ui.ctx(), &notice);
    }
}

fn shortcuts(data: &mut AppData, ctx: &egui::Context) {
    let (undo, redo, escape) = ctx.input(|input| {
        (
            input.modifiers.command && input.key_pressed(egui::Key::Z) && !input.modifiers.shift,
            input.modifiers.command
                && ((input.key_pressed(egui::Key::Z) && input.modifiers.shift)
                    || input.key_pressed(egui::Key::Y)),
            input.key_pressed(egui::Key::Escape),
        )
    });
    if undo {
        apply_undo(data, true);
    }
    if redo {
        apply_undo(data, false);
    }
    if escape {
        data.drawing.cancel();
        data.ruler_adding = false;
        data.route_adding = false;
    }
}

fn apply_undo(data: &mut AppData, undo: bool) {
    let result = if undo {
        data.store.undo()
    } else {
        data.store.redo()
    };
    match result {
        Ok(()) => {
            data.sync_editor();
            data.autosave();
        }
        Err(error) => data.notify_error(error.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Top bar
// ---------------------------------------------------------------------------

fn top_bar(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let file_label = tr(i18n, "fileMenu.trigger").to_string();
    let view_label = tr(i18n, "viewMenu.trigger").to_string();
    let undo_label = tr(i18n, "undoRedo.undo").to_string();
    let redo_label = tr(i18n, "undoRedo.redo").to_string();
    let open_label = tr(i18n, "importExport.loadButton").to_string();
    let save_label = tr(i18n, "importExport.saveButton").to_string();
    let export_kml = tr(i18n, "importExport.exportKml").to_string();
    let export_kmz = tr(i18n, "importExport.exportKmz").to_string();
    let settings_label = tr(i18n, "app.settings").to_string();
    let about_label = tr(i18n, "app.about").to_string();
    let map_label = tr(i18n, "app.basemap").to_string();
    let search_hint = tr(i18n, "geocoding.placeholder").to_string();
    let search_label = tr(i18n, "geocoding.search").to_string();
    let searching = tr(i18n, "geocoding.searching").to_string();
    let no_results = tr(i18n, "geocoding.noResults").to_string();
    let labels_label = tr(i18n, "placemarkEditor.showDistanceRings").to_string();
    let reset_label = "Reset view".to_string();

    egui::Panel::top("topbar").show(ui, |ui| {
        ui.horizontal(|ui| {
            ui.heading("Terra Globe");
            ui.separator();
            ui.menu_button(file_label, |ui| {
                if ui.button(open_label).clicked() {
                    data.open_file();
                    ui.close();
                }
                if ui.button(save_label).clicked() {
                    data.save(false);
                    ui.close();
                }
                if ui.button(format!("{export_kml}...")).clicked() {
                    if let Some(path) = data.deps.files.pick_save("terra-globe.kml") {
                        data.export_to(path, false);
                    }
                    ui.close();
                }
                if ui.button(format!("{export_kmz}...")).clicked() {
                    if let Some(path) = data.deps.files.pick_save("terra-globe.kmz") {
                        data.export_to(path, true);
                    }
                    ui.close();
                }
            });
            ui.menu_button(view_label, |ui| {
                for mode in [SceneMode::Scene3D, SceneMode::Scene2D, SceneMode::Columbus] {
                    if ui
                        .selectable_label(data.globe.mode() == mode, tr(i18n, mode.label_key()))
                        .clicked()
                    {
                        data.set_scene_mode(mode);
                        ui.close();
                    }
                }
                ui.separator();
                let labels = data.settings.show_labels;
                if ui
                    .checkbox(&mut data.settings.show_labels, labels_label.clone())
                    .changed()
                    && labels != data.settings.show_labels
                {
                    data.persist_settings();
                }
                ui.checkbox(&mut data.show_stats, "Stats");
                if ui.button(reset_label.clone()).clicked() {
                    data.globe.camera = Camera::default();
                    ui.close();
                }
            });
            ui.separator();
            if ui
                .add_enabled(data.store.can_undo(), egui::Button::new(undo_label))
                .clicked()
            {
                apply_undo(data, true);
            }
            if ui
                .add_enabled(data.store.can_redo(), egui::Button::new(redo_label))
                .clicked()
            {
                apply_undo(data, false);
            }
            ui.separator();
            let response = ui.name_inputs(tr(i18n, "geocoding.search"), |ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut data.geocode.query)
                        .hint_text(search_hint)
                        .desired_width(180.0),
                )
            });
            let submitted =
                response.lost_focus() && ui.input(|input| input.key_pressed(egui::Key::Enter));
            let clicked = ui
                .add_enabled(!data.geocode.pending, egui::Button::new(search_label))
                .clicked();
            if (submitted || clicked) && !data.geocode.pending {
                data.start_geocode_search();
            }
            if data.geocode.pending {
                ui.spinner();
                ui.label(searching);
            }
            if let Some(error) = data.geocode.error.clone() {
                if data.geocode.results.is_empty() {
                    ui.label(egui::RichText::new(no_results).color(theme::TEXT_MUTED));
                    let _ = error;
                }
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(about_label).clicked() {
                    data.modal = Some(ModalKind::About);
                }
                if ui.button(settings_label).clicked() {
                    data.modal = Some(ModalKind::Settings);
                }
                ui.menu_button(map_label, |ui| {
                    let current = data.settings.tile_source_id.clone();
                    for (id, name) in [("osm", "OpenStreetMap"), ("opentopomap", "OpenTopoMap")] {
                        if ui.selectable_label(current == id, name).clicked() && current != id {
                            data.settings.tile_source_id = id.to_string();
                            let provider = crate::tile_provider(&data.settings);
                            data.globe.set_provider(provider);
                            data.persist_settings();
                            ui.close();
                        }
                    }
                });
            });
        });

        ui.horizontal(|ui| {
            draw_toolbar(data, i18n, ui);
            ui.separator();
            let ruler_label = tr(i18n, "ruler.start").to_string();
            let route_label = tr(i18n, "routePlanner.toggle").to_string();
            if ui
                .selectable_label(data.ruler_adding, ruler_label)
                .on_hover_text(tr(i18n, "ruler.panelTitle"))
                .clicked()
            {
                data.ruler_adding = !data.ruler_adding;
                data.route_adding = false;
                data.drawing.cancel();
            }
            if ui
                .selectable_label(data.route_adding, route_label)
                .on_hover_text(tr(i18n, "routePlanner.panelTitle"))
                .clicked()
            {
                data.route_adding = !data.route_adding;
                data.ruler_adding = false;
                data.drawing.cancel();
            }
            ui.separator();
            if !data.geocode.results.is_empty() {
                ui.label(tr(i18n, "geocoding.addressLabel"));
                let mut chosen = None;
                egui::ComboBox::from_id_salt("geocode-results")
                    .selected_text(format!("{} results", data.geocode.results.len()))
                    .show_ui(ui, |ui| {
                        for (index, result) in data.geocode.results.iter().enumerate() {
                            if ui.selectable_label(false, &result.label).clicked() {
                                chosen = Some(index);
                            }
                        }
                    });
                if let Some(index) = chosen {
                    if let Some(result) = data.geocode.results.get(index).cloned() {
                        let added = data
                            .store
                            .add_geocoded_placemark(result.label.clone(), result.point);
                        if let Ok(placemark) = added {
                            data.geocode.results.clear();
                            data.geocode.query.clear();
                            let now = data.now_seconds();
                            if let Some(placemark) = data.store.library().placemark(&placemark.id) {
                                data.globe.fly_to_geometry(&placemark.geometry, now);
                            }
                            data.select(Some(placemark.id));
                            data.autosave();
                        }
                    }
                }
            }
        });
    });
}

fn draw_toolbar(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let tools: [(DrawTool, &str); 5] = [
        (DrawTool::Marker, "drawingToolbar.marker"),
        (DrawTool::Line, "drawingToolbar.line"),
        (DrawTool::Polygon, "drawingToolbar.polygon"),
        (DrawTool::Circle, "drawingToolbar.circle"),
        (DrawTool::Rectangle, "drawingToolbar.rectangle"),
    ];
    ui.label(tr(i18n, "drawingToolbar.geometryLabel"));
    for (tool, key) in tools {
        let active = data.drawing.tool() == Some(tool);
        if ui.selectable_label(active, tr(i18n, key)).clicked() {
            data.drawing.start(tool);
            data.ruler_adding = false;
            data.route_adding = false;
        }
    }
    if data.drawing.is_active() {
        ui.separator();
        if ui.button(tr(i18n, "drawingToolbar.finish")).clicked() {
            finish_drawing(data);
        }
        if ui.button(tr(i18n, "drawingToolbar.cancel")).clicked() {
            data.drawing.cancel();
        }
    }
}

fn finish_drawing(data: &mut AppData) {
    if let Some(geometry) = data.drawing.finish() {
        create_placemark(data, geometry);
    }
}

fn create_placemark(data: &mut AppData, geometry: PlacemarkGeometry) {
    let folder = data.selected_folder.clone();
    match data.store.create_placemark(geometry, folder) {
        Ok(placemark) => {
            data.select(Some(placemark.id.clone()));
            data.autosave();
            data.notify(format!("created {}", placemark.name));
        }
        Err(error) => data.notify_error(error.to_string()),
    }
}

// ---------------------------------------------------------------------------
// Left panel: library tree
// ---------------------------------------------------------------------------

fn left_panel(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let places = tr(i18n, "folders.placesHeader").to_string();
    let new_folder = tr(i18n, "folders.newFolder").to_string();
    let search_hint = tr(i18n, "folders.searchPlaceholder").to_string();
    let rename_label = tr(i18n, "folders.rename").to_string();
    let create_label = tr(i18n, "folders.create").to_string();
    let toggle_folder = tr(i18n, "folders.toggleFolderVisibility").to_string();
    let toggle_placemark = tr(i18n, "folders.togglePlacemarkVisibility").to_string();
    let folder_placeholder = tr(i18n, "folders.folderNamePlaceholder").to_string();
    let nothing = tr(i18n, "importExport.noFoldersOrPlacemarks").to_string();

    egui::Panel::left("library")
        .resizable(true)
        .default_size(280.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(places);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(new_folder).clicked() {
                        let name = data.store.library().default_folder_name(None);
                        match data.store.create_folder(&name, None) {
                            Ok(folder) => {
                                data.renaming = Some((folder.id.clone(), folder.name.clone()));
                                data.selected_folder = Some(folder.id);
                                data.autosave();
                            }
                            Err(error) => data.notify_error(error.to_string()),
                        }
                    }
                });
            });
            ui.add(
                egui::TextEdit::singleline(&mut data.search)
                    .hint_text(search_hint)
                    .desired_width(f32::INFINITY),
            );
            ui.separator();

            let query = data.search.to_lowercase();
            let root_folders: Vec<String> = data
                .store
                .library()
                .child_folders(None)
                .iter()
                .map(|folder| folder.id.clone())
                .collect();
            let root_placemarks: Vec<String> = data
                .store
                .library()
                .placemarks_in(None)
                .iter()
                .map(|placemark| placemark.id.clone())
                .collect();

            egui::ScrollArea::vertical().show(ui, |ui| {
                for folder_id in &root_folders {
                    render_folder(data, i18n, ui, folder_id, 0, &query);
                }
                for placemark_id in &root_placemarks {
                    render_placemark_row(data, i18n, ui, placemark_id, 0, &query);
                }
                if root_folders.is_empty() && root_placemarks.is_empty() {
                    ui.label(egui::RichText::new(nothing).color(theme::TEXT_MUTED));
                }
            });

            if let Some((id, buffer)) = data.renaming.clone() {
                let window_title = rename_label.clone();
                let field_name = rename_label.clone();
                egui::Window::new(window_title)
                    .collapsible(false)
                    .resizable(false)
                    .show(ui.ctx(), |ui| {
                        let mut text = buffer;
                        let response = ui.name_inputs(field_name.clone(), |ui| {
                            ui.add(
                                egui::TextEdit::singleline(&mut text).hint_text(folder_placeholder),
                            )
                        });
                        ui.horizontal(|ui| {
                            if ui.button(create_label.clone()).clicked()
                                || (response.lost_focus()
                                    && ui.input(|input| input.key_pressed(egui::Key::Enter)))
                            {
                                let result = if data.store.library().folder(&id).is_some() {
                                    data.store.rename_folder(&id, text.clone())
                                } else {
                                    Ok(())
                                };
                                match result {
                                    Ok(()) => {
                                        data.renaming = None;
                                        data.autosave();
                                    }
                                    Err(error) => data.notify_error(error.to_string()),
                                }
                            }
                            if ui.button(tr(i18n, "common.cancel")).clicked() {
                                data.renaming = None;
                            }
                        });
                    });
            }

            let _ = (toggle_folder, toggle_placemark);
        });
}

fn render_folder(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    folder_id: &str,
    depth: usize,
    query: &str,
) {
    let Some(folder) = data.store.library().folder(folder_id).cloned() else {
        return;
    };
    if !query.is_empty()
        && !folder.name.to_lowercase().contains(query)
        && !subtree_matches(data, folder_id, query)
    {
        return;
    }

    let inherited_visible = crate::folder_is_visible(data.store.library(), &folder);
    let row = ui
        .horizontal(|ui| {
            ui.add_space(depth as f32 * 12.0);
            let arrow = if folder.open.unwrap_or(true) {
                "v"
            } else {
                ">"
            };
            if ui.small_button(arrow).clicked() {
                let open = folder.open.unwrap_or(true);
                if let Err(error) = data.store.set_folder_open(&folder.id, !open) {
                    data.notify_error(error.to_string());
                }
            }
            let eye = if folder.visibility { "ON" } else { "OFF" };
            if ui
                .small_button(eye)
                .on_hover_text(tr(i18n, "folders.toggleFolderVisibility"))
                .clicked()
            {
                if let Err(error) = data
                    .store
                    .set_folder_visibility(&folder.id, !folder.visibility)
                {
                    data.notify_error(error.to_string());
                }
            }
            let color = if inherited_visible {
                theme::TEXT
            } else {
                theme::TEXT_MUTED
            };
            let label = egui::RichText::new(folder.name.clone()).color(color);
            let response = ui.selectable_label(
                data.selected_folder.as_deref() == Some(folder.id.as_str()),
                label,
            );
            if response.clicked() {
                data.selected_folder = Some(folder.id.clone());
            }
            if response.double_clicked() {
                if let Some(placemark) =
                    data.store.library().placemarks_in(Some(&folder.id)).first()
                {
                    let geometry = placemark.geometry.clone();
                    let now = data.now_seconds();
                    data.globe.fly_to_geometry(&geometry, now);
                }
            }
            response.context_menu(|ui| {
                if ui.button(tr(i18n, "folders.rename")).clicked() {
                    data.renaming = Some((folder.id.clone(), folder.name.clone()));
                    ui.close();
                }
                if ui.button(tr(i18n, "folders.newFolder")).clicked() {
                    let name = data.store.library().default_folder_name(Some(&folder.id));
                    match data.store.create_folder(&name, Some(folder.id.clone())) {
                        Ok(child) => {
                            data.renaming = Some((child.id.clone(), child.name.clone()));
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
        })
        .response;

    // Response-based drop target: wrapping rows in a drop zone would paint a
    // widget on top of them and swallow their clicks.
    if row.dnd_hover_payload::<DragPayload>().is_some() {
        ui.painter().rect_stroke(
            row.rect.expand(2.0),
            3.0,
            egui::Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(payload) = row.dnd_release_payload::<DragPayload>() {
        apply_drop(data, &payload, Some(folder.id.clone()));
    }

    if folder.open.unwrap_or(true) {
        let child_folders: Vec<String> = data
            .store
            .library()
            .child_folders(Some(&folder.id))
            .iter()
            .map(|child| child.id.clone())
            .collect();
        for child in child_folders {
            render_folder(data, i18n, ui, &child, depth + 1, query);
        }
        let children: Vec<String> = data
            .store
            .library()
            .placemarks_in(Some(&folder.id))
            .iter()
            .map(|placemark| placemark.id.clone())
            .collect();
        for child in children {
            render_placemark_row(data, i18n, ui, &child, depth + 1, query);
        }
    }
}

fn subtree_matches(data: &AppData, folder_id: &str, query: &str) -> bool {
    for placemark in data.store.library().placemarks_in(Some(folder_id)) {
        if placemark.name.to_lowercase().contains(query) {
            return true;
        }
    }
    for child in data.store.library().child_folders(Some(folder_id)) {
        if child.name.to_lowercase().contains(query) || subtree_matches(data, &child.id, query) {
            return true;
        }
    }
    false
}

fn render_placemark_row(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    placemark_id: &str,
    depth: usize,
    query: &str,
) {
    let Some(placemark) = data.store.library().placemark(placemark_id).cloned() else {
        return;
    };
    if !query.is_empty() && !placemark.name.to_lowercase().contains(query) {
        return;
    }

    let inherited_visible = data.store.library().is_placemark_visible(&placemark);
    ui.horizontal(|ui| {
        ui.add_space(depth as f32 * 12.0 + 20.0);
        let eye = if placemark.visibility { "ON" } else { "OFF" };
        if ui
            .small_button(eye)
            .on_hover_text(tr(i18n, "folders.togglePlacemarkVisibility"))
            .clicked()
        {
            if let Err(error) = data
                .store
                .set_placemark_visibility(&placemark.id, !placemark.visibility)
            {
                data.notify_error(error.to_string());
            }
        }
        let color = if inherited_visible {
            theme::TEXT
        } else {
            theme::TEXT_MUTED
        };
        let label = egui::RichText::new(placemark.name.clone()).color(color);

        // A dedicated grip keeps the label clickable: wrapping the label in a
        // drag source would turn clicks into drag starts.
        ui.dnd_drag_source(
            egui::Id::unique(("placemark-drag", &placemark.id)),
            DragPayload::Placemark(placemark.id.clone()),
            |ui| ui.small_button("=").on_hover_text("Drag to move"),
        );

        let response = ui.selectable_label(
            data.selected.as_deref() == Some(placemark.id.as_str()),
            label,
        );

        if response.clicked() {
            data.select(Some(placemark.id.clone()));
            data.selected_folder = placemark.folder_id.clone();
        }
        if response.double_clicked() {
            let now = data.now_seconds();
            data.globe.fly_to_geometry(&placemark.geometry, now);
        }
        response.context_menu(|ui| {
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

// ---------------------------------------------------------------------------
// Right panel: placemark editor
// ---------------------------------------------------------------------------

fn right_panel(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    if data.editor.is_none() {
        return;
    }
    let title = tr(i18n, "placemarkEditor.header").to_string();
    egui::Panel::right("editor")
        .resizable(true)
        .default_size(300.0)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(tr(i18n, "placemarkEditor.close")).clicked() {
                        data.close_editor();
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                editor_body(data, i18n, ui);
            });
        });
}

fn editor_body(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let Some(mut editor) = data.editor.clone() else {
        return;
    };
    let Some(placemark) = data.store.library().placemark(&editor.id).cloned() else {
        data.close_editor();
        return;
    };

    ui.name_inputs(tr(i18n, "placemarkEditor.name"), |ui| {
        ui.add(egui::TextEdit::singleline(&mut editor.name).desired_width(f32::INFINITY))
    });
    ui.name_inputs(tr(i18n, "placemarkEditor.description"), |ui| {
        ui.add(
            egui::TextEdit::multiline(&mut editor.description)
                .desired_rows(2)
                .desired_width(f32::INFINITY),
        )
    });
    ui.checkbox(&mut editor.visible, tr(i18n, "placemarkEditor.visible"));

    if matches!(placemark.geometry, PlacemarkGeometry::Point { .. }) {
        ui.separator();
        let current = editor
            .style
            .marker_icon
            .unwrap_or(terra_core::DEFAULT_MARKER_ICON);
        egui::ComboBox::from_label(tr(i18n, "placemarkEditor.marker"))
            .selected_text(current.as_str())
            .show_ui(ui, |ui| {
                for icon in terra_core::MarkerIconId::all() {
                    if ui
                        .selectable_label(current == icon, icon.as_str())
                        .clicked()
                    {
                        editor.style.marker_icon = Some(icon);
                    }
                }
            });
    }

    ui.separator();
    ui.label(tr(i18n, "placemarkEditor.outline"));
    ui.horizontal(|ui| {
        let mut color = theme::color32_from_hex(&editor.style.outline_color);
        let changed = ui
            .name_inputs(tr(i18n, "placemarkEditor.outlineColor"), |ui| {
                ui.color_edit_button_srgba(&mut color)
            })
            .on_hover_text(tr(i18n, "placemarkEditor.outlineColor"))
            .changed();
        if changed {
            editor.style.outline_color = theme::hex_from_color32(color);
        }
        ui.add(
            egui::Slider::new(&mut editor.style.outline_width, 0.5..=12.0)
                .text(tr(i18n, "placemarkEditor.outlineWidth")),
        );
        ui.name_inputs(tr(i18n, "placemarkEditor.outline"), |ui| {
            ui.checkbox(&mut editor.style.outline_enabled, "")
        });
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut editor.style.filled, tr(i18n, "placemarkEditor.filled"));
        let mut color = theme::color32_from_hex(&editor.style.fill_color);
        let changed = ui
            .name_inputs(tr(i18n, "placemarkEditor.fillColor"), |ui| {
                ui.color_edit_button_srgba(&mut color)
            })
            .on_hover_text(tr(i18n, "placemarkEditor.fillColor"))
            .changed();
        if changed {
            editor.style.fill_color = theme::hex_from_color32(color);
        }
        ui.add(
            egui::Slider::new(&mut editor.style.fill_opacity, 0.0..=1.0)
                .text(tr(i18n, "placemarkEditor.color")),
        );
        let _ = &editor.style.fill_opacity;
    });

    ui.separator();
    ui.label(tr(i18n, "placemarkEditor.measurements"));
    for (key, value) in tools::formatted_measurements(
        &placemark.geometry,
        data.settings.unit_system,
        data.settings.coordinate_format,
    ) {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(tr(i18n, key)).color(theme::TEXT_MUTED));
            ui.label(value);
        });
    }

    if matches!(placemark.geometry, PlacemarkGeometry::Point { .. }) {
        ui.separator();
        ui.checkbox(
            &mut editor.rings_enabled,
            tr(i18n, "placemarkEditor.showDistanceRings"),
        );
        ui.horizontal(|ui| {
            ui.label(tr(i18n, "placemarkEditor.ringSpacing"));
            let mut spacing =
                terra_core::meters_to_unit(editor.ring_spacing_meters, data.settings.unit_system);
            let changed = ui
                .name_inputs(tr(i18n, "placemarkEditor.ringSpacing"), |ui| {
                    ui.add(egui::DragValue::new(&mut spacing).speed(1.0))
                })
                .changed();
            if changed {
                editor.ring_spacing_meters =
                    terra_core::unit_to_meters(spacing, data.settings.unit_system);
            }
            ui.label(terra_core::distance_unit_label(data.settings.unit_system));
        });
    }

    if terra_core::domain::measurements::has_elevation_data(&placemark.geometry)
        && ui
            .button(tr(i18n, "placemarkEditor.elevationProfile"))
            .clicked()
    {
        data.modal = Some(ModalKind::HeightProfile);
    }

    ui.add_space(8.0);
    ui.horizontal(|ui| {
        if ui.button(tr(i18n, "placemarkEditor.save")).clicked() {
            let patch = PlacemarkPatch {
                name: Patch::Set(editor.name.clone()),
                description: Patch::Set(
                    (!editor.description.trim().is_empty()).then(|| editor.description.clone()),
                ),
                visibility: Patch::Set(editor.visible),
                ..Default::default()
            };
            let result = data
                .store
                .update_placemark(&editor.id, patch)
                .and_then(|_| data.store.update_style(editor.style.clone()));
            match result {
                Ok(()) => {
                    data.editor = Some(editor.clone());
                    data.autosave();
                    data.notify("saved");
                }
                Err(error) => data.notify_error(error.to_string()),
            }
        }
        if ui
            .button(egui::RichText::new(tr(i18n, "placemarkEditor.delete")).color(theme::DANGER))
            .clicked()
        {
            data.confirm = Some(ConfirmAction::DeletePlacemark(editor.id.clone()));
        }
    });

    data.editor = Some(editor);
}

// ---------------------------------------------------------------------------
// Globe viewport
// ---------------------------------------------------------------------------

fn central_globe(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    render_state: Option<&egui_wgpu::RenderState>,
) {
    egui::CentralPanel::default()
        .frame(egui::Frame::NONE.fill(theme::SPACE))
        .show(ui, |ui| {
            let available = ui.available_rect_before_wrap();
            let pixels_per_point = ui.ctx().pixels_per_point();
            let width = (available.width() * pixels_per_point).round().max(1.0) as u32;
            let height = (available.height() * pixels_per_point).round().max(1.0) as u32;
            let scale = f64::from(pixels_per_point);
            let (rect, response) =
                ui.allocate_exact_size(available.size(), egui::Sense::click_and_drag());

            data.globe.resize(width, height);
            handle_globe_input(data, ui, rect, &response);
            data.cursor_world = ui
                .input(|input| input.pointer.hover_pos())
                .filter(|_| response.hovered())
                .and_then(|pos| {
                    data.globe.pick(pixel_pos(pos, rect, scale)).map(|hit| {
                        GeoPoint::new(hit.lon_deg, hit.lat_deg).unwrap_or(GeoPoint {
                            lon: 0.0,
                            lat: 0.0,
                            altitude: None,
                            altitude_mode: None,
                        })
                    })
                });

            let (revision, scene) = data.build_scene();
            data.globe.set_scene(revision, scene);

            let start = Instant::now();
            data.globe.render(data.now_seconds());
            register_texture(data, render_state);
            if let Some(texture) = data.texture {
                ui.painter().image(
                    texture,
                    rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );
            }
            draw_overlays(data, i18n, ui, rect, scale);
            draw_hud(data, ui, rect);
            let elapsed = start.elapsed();
            data.frame_ms = data.frame_ms * 0.9 + elapsed.as_secs_f32() * 1000.0 * 0.1;

            // Keep animating while tiles or camera animations are in flight: the
            // fetcher is only pumped from `render`, so a slow idle repaint would
            // throttle imagery streaming to a couple of tiles per second.
            let busy = data.globe.camera.is_animating()
                || data.globe.is_loading_imagery()
                || data.route.pending
                || data.geocode.pending;
            ui.ctx().request_repaint_after(if busy {
                std::time::Duration::from_millis(16)
            } else {
                std::time::Duration::from_millis(120)
            });
        });
}

fn pixel_pos(pos: egui::Pos2, rect: egui::Rect, pixels_per_point: f64) -> (f64, f64) {
    (
        f64::from(pos.x - rect.min.x) * pixels_per_point,
        f64::from(pos.y - rect.min.y) * pixels_per_point,
    )
}

fn handle_globe_input(
    data: &mut AppData,
    ui: &egui::Ui,
    rect: egui::Rect,
    response: &egui::Response,
) {
    let pixels_per_point = f64::from(ui.ctx().pixels_per_point());
    let cursor_pixel = ui
        .input(|input| input.pointer.hover_pos())
        .map(|pos| pixel_pos(pos, rect, pixels_per_point))
        .unwrap_or((
            f64::from(rect.width()) * pixels_per_point * 0.5,
            f64::from(rect.height()) * pixels_per_point * 0.5,
        ));
    let (shift, secondary_down, middle_down, pointer_delta) = ui.input(|input| {
        (
            input.modifiers.shift,
            input.pointer.button_down(egui::PointerButton::Secondary),
            input.pointer.button_down(egui::PointerButton::Middle),
            input.pointer.delta(),
        )
    });

    // Left drag moves the globe, the way Google Earth does. Shift turns the
    // drag into tilt/rotate instead.
    if response.dragged_by(egui::PointerButton::Primary) && !data.drawing.is_active() {
        let delta = response.drag_delta();
        if shift {
            data.globe.orbit(f64::from(delta.x), f64::from(delta.y));
        } else {
            // The globe works in physical pixels; egui reports drags in points.
            data.globe.pan_pixels(
                cursor_pixel,
                (
                    f64::from(delta.x) * pixels_per_point,
                    f64::from(delta.y) * pixels_per_point,
                ),
            );
        }
    }

    if response.hovered() {
        // Right drag: up/down zooms towards the cursor, left/right rotates.
        if secondary_down {
            if pointer_delta.y.abs() > 0.0 {
                data.globe
                    .zoom_at(cursor_pixel, (f64::from(pointer_delta.y) * 0.01).exp());
            }
            if pointer_delta.x.abs() > 0.0 {
                data.globe.rotate_by(f64::from(pointer_delta.x) * 0.005);
            }
        }
        // Middle drag tilts and rotates, like Google Earth's navigation pad.
        if middle_down {
            data.globe
                .orbit(f64::from(pointer_delta.x), f64::from(pointer_delta.y));
        }

        let (scroll, pinch) = ui.input(|input| (input.smooth_scroll_delta.y, input.zoom_delta()));
        if (pinch - 1.0).abs() > 0.01 {
            data.globe.zoom_at(cursor_pixel, 1.0 / f64::from(pinch));
        } else if scroll.abs() > 0.5 {
            data.globe
                .zoom_at(cursor_pixel, (-f64::from(scroll) * 0.003).exp());
        }

        if let Some(pos) = ui.input(|input| input.pointer.hover_pos()) {
            data.hovered = data
                .globe
                .pick_placemark(pixel_pos(pos, rect, pixels_per_point))
                .map(|hit| hit.id);
        }
    } else {
        data.hovered = None;
    }

    globe_keyboard(data, ui, response);

    if response.clicked() {
        response.request_focus();
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        let pixel = pixel_pos(pos, rect, pixels_per_point);
        let surface = data
            .globe
            .pick(pixel)
            .and_then(|hit| GeoPoint::new(hit.lon_deg, hit.lat_deg).ok());

        if data.route_adding {
            if let Some(point) = surface {
                data.route.waypoints.push(point);
            }
            return;
        }
        if data.ruler_adding {
            if let Some(point) = surface {
                data.ruler.add(point);
            }
            return;
        }
        if data.drawing.is_active() {
            let Some(point) = surface else {
                return;
            };
            if let Some(geometry) = data.drawing.click(point) {
                create_placemark(data, geometry);
            }
            return;
        }

        match data.globe.pick_placemark(pixel) {
            Some(hit) => data.select(Some(hit.id)),
            None => {
                data.close_editor();
                data.selected_folder = None;
            }
        }
    }

    // Google Earth: double-click zooms towards the cursor, right double-click
    // zooms away from it.
    let adding = data.drawing.is_active() || data.ruler_adding || data.route_adding;
    if response.double_clicked_by(egui::PointerButton::Secondary) {
        data.globe.zoom_at(cursor_pixel, 2.5);
    } else if response.double_clicked() && !adding {
        let Some(pos) = response.interact_pointer_pos() else {
            return;
        };
        let pixel = pixel_pos(pos, rect, pixels_per_point);
        match data.globe.pick_placemark(pixel).map(|hit| hit.id) {
            Some(id) => {
                let now = data.now_seconds();
                if let Some(geometry) = data
                    .store
                    .library()
                    .placemark(&id)
                    .map(|placemark| placemark.geometry.clone())
                {
                    data.globe.fly_to_geometry(&geometry, now);
                }
            }
            None => data.globe.zoom_at(pixel, 0.5),
        }
    }
}

/// Rotation speed for shift+arrow, radians per second.
const ROTATE_RATE: f64 = 0.9;
/// Tilt speed for shift+arrow, radians per second.
const TILT_RATE: f64 = 0.6;
/// Pan speed as a fraction of the camera distance per second.
const PAN_RATE: f64 = 0.4;
/// One press of `+`/`-`/
const ZOOM_STEP: f64 = 0.75;

/// Google Earth's 3D-viewer keys: arrows move, shift+arrows rotate and tilt,
/// `+`/`-`/Page Up/Page Down zoom, `n`/`u`/`r` reset the orientation and space
/// stops a fly-to.
fn globe_keyboard(data: &mut AppData, ui: &egui::Ui, response: &egui::Response) {
    use egui::Key;
    let editing_text = ui
        .memory(|memory| memory.focused())
        .is_some_and(|id| id != response.id);
    if editing_text || !(response.has_focus() || response.hovered()) {
        data.globe_key_hold = 0.0;
        return;
    }

    let (shift, held, pressed, tapped, dt) = ui.input(|input| {
        let down = |key: Key| input.key_down(key);
        (
            input.modifiers.shift,
            [
                down(Key::ArrowLeft),
                down(Key::ArrowRight),
                down(Key::ArrowUp),
                down(Key::ArrowDown),
            ],
            (
                input.key_pressed(Key::Plus) || input.key_pressed(Key::Equals),
                input.key_pressed(Key::Minus),
                input.key_pressed(Key::PageUp),
                input.key_pressed(Key::PageDown),
                input.key_pressed(Key::N),
                input.key_pressed(Key::U),
                input.key_pressed(Key::R),
                input.key_pressed(Key::Space),
            ),
            input.key_pressed(Key::ArrowLeft)
                || input.key_pressed(Key::ArrowRight)
                || input.key_pressed(Key::ArrowUp)
                || input.key_pressed(Key::ArrowDown),
            f64::from(input.stable_dt).clamp(0.001, 0.1),
        )
    });
    let [left, right, up, down] = held;
    let (zoom_in, zoom_out, page_up, page_down, north, top_down, reset, stop) = pressed;

    if north {
        data.globe.set_north_up();
    }
    if top_down {
        data.globe.set_top_down();
    }
    if reset {
        data.globe.reset_view();
    }
    if stop {
        data.globe.stop_motion();
    }
    if zoom_in || page_up {
        data.globe.zoom_by(ZOOM_STEP);
    }
    if zoom_out || page_down {
        data.globe.zoom_by(1.0 / ZOOM_STEP);
    }

    // A single tap still has to turn or move a visible amount.
    let tap_dt = if tapped { dt.max(1.0 / 12.0) } else { dt };
    let mut busy = false;
    if shift {
        // Shift+left/right rotate the imagery, shift+up tilts towards top-down
        // and shift+down towards the horizon.
        if left || right {
            let direction = f64::from(i32::from(right) - i32::from(left));
            data.globe.rotate_by(direction * ROTATE_RATE * tap_dt);
            busy = true;
        }
        if up || down {
            let direction = f64::from(i32::from(up) - i32::from(down));
            data.globe.tilt_by(direction * TILT_RATE * tap_dt);
            busy = true;
        }
    } else if left || right || up || down {
        // Google Earth moves a fraction of the current altitude per second and
        // speeds up while a key is held.
        data.globe_key_hold = (data.globe_key_hold + dt as f32).min(1.5);
        let step_dt = tap_dt;
        let speed = PAN_RATE * f64::from(1.0 + 2.0 * data.globe_key_hold);
        let step = data.globe.view_distance() * speed * step_dt;
        let forward = f64::from(i32::from(up) - i32::from(down)) * step;
        let sideways = f64::from(i32::from(right) - i32::from(left)) * step;
        data.globe.move_view(forward, sideways);
        busy = true;
    } else {
        data.globe_key_hold = 0.0;
    }

    if busy || zoom_in || zoom_out || page_up || page_down || reset {
        ui.ctx().request_repaint();
    }
}

fn register_texture(data: &mut AppData, render_state: Option<&egui_wgpu::RenderState>) {
    let generation = data.globe.renderer.generation();
    if data.texture.is_some() && generation == data.registered_generation {
        return;
    }
    let Some(render_state) = render_state else {
        return;
    };
    if let Some(texture) = data.texture.take() {
        render_state.renderer.write().free_texture(&texture);
    }
    let texture = render_state.renderer.write().register_native_texture(
        &render_state.device,
        data.globe.renderer.texture_view(),
        egui_wgpu::wgpu::FilterMode::Linear,
    );
    data.texture = Some(texture);
    data.registered_generation = generation;
}

fn draw_overlays(
    data: &AppData,
    i18n: &I18n,
    ui: &egui::Ui,
    rect: egui::Rect,
    pixels_per_point: f64,
) {
    let painter = ui.painter().with_clip_rect(rect);
    let viewport = data.globe.viewport();

    let mut marker_count = 0usize;
    for placemark in data.store.library().visible_placemarks() {
        if marker_count > 600 {
            break;
        }
        let style = data.store.library().style_for(placemark);
        let selected = data.selected.as_deref() == Some(placemark.id.as_str());
        let hovered = data.hovered.as_deref() == Some(placemark.id.as_str());

        match &placemark.geometry {
            PlacemarkGeometry::Point { coordinates }
            | PlacemarkGeometry::Model {
                position: coordinates,
                ..
            } => {
                let screen = data.globe.camera.project(
                    terra_globe::geodetic_to_ecef(terra_globe::Geodetic::from_degrees(
                        coordinates.lon,
                        coordinates.lat,
                        coordinates.altitude.unwrap_or(0.0),
                    )),
                    viewport,
                );
                let Some(screen) = screen else { continue };
                let position = egui::pos2(
                    rect.min.x + (screen.x / pixels_per_point) as f32,
                    rect.min.y + (screen.y / pixels_per_point) as f32,
                );
                if !rect.contains(position) {
                    continue;
                }
                let fill = theme::color32_from_hex(&style.fill_color);
                let outline = if selected {
                    theme::ACCENT
                } else if hovered {
                    egui::Color32::WHITE
                } else {
                    theme::color32_from_hex(&style.outline_color)
                };
                draw_marker(
                    &painter,
                    position,
                    style.marker_icon,
                    fill,
                    outline,
                    selected,
                );
                marker_count += 1;
                if data.settings.show_labels && data.globe.view_distance() < 3_000_000.0 {
                    painter.text(
                        position + egui::vec2(10.0, 4.0),
                        egui::Align2::LEFT_TOP,
                        &placemark.name,
                        egui::FontId::proportional(label_size(data.globe.view_distance())),
                        theme::TEXT,
                    );
                }
            }
            _ => {
                if !data.settings.show_labels {
                    continue;
                }
                let center = terra_core::geometry_center(&placemark.geometry);
                let Some(screen) = data.globe.project(terra_globe::Geodetic::from_degrees(
                    center.lon, center.lat, 0.0,
                )) else {
                    continue;
                };
                let position = egui::pos2(
                    rect.min.x + (screen.x / pixels_per_point) as f32,
                    rect.min.y + (screen.y / pixels_per_point) as f32,
                );
                if !rect.contains(position) || data.globe.view_distance() > 2_000_000.0 {
                    continue;
                }
                painter.text(
                    position,
                    egui::Align2::CENTER_CENTER,
                    &placemark.name,
                    egui::FontId::proportional(label_size(data.globe.view_distance())),
                    if selected { theme::ACCENT } else { theme::TEXT },
                );
            }
        }
    }

    for overlay in &data.store.library().overlays {
        if !overlay.visibility {
            continue;
        }
        let position = anchor_position(
            &overlay.screen_xy,
            rect,
            overlay.size_x.unwrap_or(140.0) as f32,
            overlay.size_y.unwrap_or(40.0) as f32,
        );
        let size = egui::vec2(
            overlay.size_x.unwrap_or(140.0) as f32,
            overlay.size_y.unwrap_or(40.0) as f32,
        );
        let overlay_rect = egui::Rect::from_min_size(position, size);
        painter.rect_filled(
            overlay_rect,
            3.0,
            theme::SURFACE_RAISED.gamma_multiply(0.85),
        );
        painter.rect_stroke(
            overlay_rect,
            3.0,
            egui::Stroke::new(1.0, theme::ACCENT),
            egui::StrokeKind::Inside,
        );
        painter.text(
            overlay_rect.center(),
            egui::Align2::CENTER_CENTER,
            &overlay.name,
            egui::FontId::proportional(11.0),
            theme::TEXT,
        );
    }

    if data.ruler_adding || data.ruler.points.len() >= 2 {
        if let Some(last) = data.ruler.points.last() {
            if data.settings.show_labels {
                let _ = format_coordinate(last, data.settings.coordinate_format);
            }
        }
    }
    let _ = i18n;
}

/// Resolves a KML ScreenOverlay anchor into a viewport position.
fn anchor_position(
    anchor: &terra_core::ScreenOverlayAnchor,
    rect: egui::Rect,
    width: f32,
    height: f32,
) -> egui::Pos2 {
    use terra_core::AnchorUnits;
    let resolve = |value: f64, units: AnchorUnits, span: f32, size: f32| -> f32 {
        match units {
            AnchorUnits::Fraction => value as f32 * span,
            AnchorUnits::Pixels => value as f32,
            AnchorUnits::InsetPixels => span - size - value as f32,
        }
    };
    egui::pos2(
        rect.min.x + resolve(anchor.x, anchor.x_units, rect.width(), width),
        rect.min.y + resolve(anchor.y, anchor.y_units, rect.height(), height),
    )
}

fn label_size(distance: f64) -> f32 {
    let scaled = 18.0 - (distance / 2_000_000.0).clamp(0.0, 1.0) * 7.0;
    scaled.clamp(11.0, 18.0) as f32
}

fn draw_marker(
    painter: &egui::Painter,
    position: egui::Pos2,
    icon: Option<terra_core::MarkerIconId>,
    fill: egui::Color32,
    outline: egui::Color32,
    selected: bool,
) {
    let radius = if selected { 8.0 } else { 6.0 };
    let icon = icon.unwrap_or(terra_core::DEFAULT_MARKER_ICON);
    let stroke = egui::Stroke::new(if selected { 2.0 } else { 1.5 }, outline);
    match icon {
        terra_core::MarkerIconId::Star => {
            let mut points = Vec::new();
            for index in 0..10 {
                let angle =
                    -std::f32::consts::FRAC_PI_2 + index as f32 * std::f32::consts::PI / 5.0;
                let length = if index % 2 == 0 {
                    radius + 2.0
                } else {
                    radius * 0.45
                };
                points.push(position + egui::vec2(angle.cos(), angle.sin()) * length);
            }
            painter.add(egui::Shape::convex_polygon(points, fill, stroke));
        }
        terra_core::MarkerIconId::Flag => {
            painter.line_segment(
                [
                    position - egui::vec2(0.0, radius),
                    position + egui::vec2(0.0, radius * 1.4),
                ],
                egui::Stroke::new(1.8, outline),
            );
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position - egui::vec2(0.0, radius),
                    position + egui::vec2(radius + 3.0, -radius + 3.0),
                    position + egui::vec2(0.0, -radius + 7.0),
                ],
                fill,
                stroke,
            ));
        }
        terra_core::MarkerIconId::Navigation => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position + egui::vec2(0.0, -radius - 2.0),
                    position + egui::vec2(radius + 1.0, radius),
                    position + egui::vec2(-radius - 1.0, radius),
                ],
                fill,
                stroke,
            ));
        }
        terra_core::MarkerIconId::Landmark => {
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position + egui::vec2(-radius, -radius * 0.2),
                    position + egui::vec2(0.0, -radius - 2.0),
                    position + egui::vec2(radius, -radius * 0.2),
                ],
                outline,
                stroke,
            ));
            painter.rect_filled(
                egui::Rect::from_center_size(
                    position + egui::vec2(0.0, radius * 0.4),
                    egui::vec2(radius * 1.6, radius),
                ),
                2.0,
                fill,
            );
        }
        terra_core::MarkerIconId::Pin => {
            painter.circle_filled(position - egui::vec2(0.0, radius * 0.4), radius * 0.7, fill);
            painter.line_segment(
                [
                    position - egui::vec2(0.0, radius * 0.4),
                    position + egui::vec2(0.0, radius),
                ],
                egui::Stroke::new(1.8, outline),
            );
        }
        terra_core::MarkerIconId::MapPin => {
            painter.circle(position, radius, fill, stroke);
            painter.add(egui::Shape::convex_polygon(
                vec![
                    position + egui::vec2(-radius * 0.75, radius * 0.6),
                    position + egui::vec2(0.0, radius * 1.9),
                    position + egui::vec2(radius * 0.75, radius * 0.6),
                ],
                fill,
                stroke,
            ));
        }
    }
}

fn draw_hud(data: &AppData, ui: &egui::Ui, rect: egui::Rect) {
    let painter = ui.painter().with_clip_rect(rect);
    painter.text(
        rect.left_bottom() + egui::vec2(10.0, -10.0),
        egui::Align2::LEFT_BOTTOM,
        data.globe.provider.credit(),
        egui::FontId::proportional(11.0),
        theme::TEXT_MUTED,
    );
    if let Some(point) = data.cursor_world {
        painter.text(
            rect.right_bottom() + egui::vec2(-10.0, -10.0),
            egui::Align2::RIGHT_BOTTOM,
            format_coordinate(&point, data.settings.coordinate_format),
            egui::FontId::proportional(11.0),
            theme::TEXT_MUTED,
        );
    }
    if let Some(error) = &data.error_banner {
        painter.text(
            rect.center_top() + egui::vec2(0.0, 12.0),
            egui::Align2::CENTER_TOP,
            error,
            egui::FontId::proportional(13.0),
            theme::DANGER,
        );
    }
    if data.show_stats {
        let stats = data.globe.renderer.stats();
        let lines = format!(
            "{:.1} ms/frame\n{} tiles drawn\n{} textures\n{} requests\nlevel {}\n{} scene items",
            data.frame_ms,
            stats.tiles_drawn,
            stats.textures_ready,
            stats.requests_in_flight,
            stats.deepest_level,
            stats.scene_items
        );
        painter.text(
            rect.right_top() + egui::vec2(-10.0, 10.0),
            egui::Align2::RIGHT_TOP,
            lines,
            egui::FontId::monospace(11.0),
            theme::TEXT_MUTED,
        );
    }
}

// ---------------------------------------------------------------------------
// Floating panels, modals, notices
// ---------------------------------------------------------------------------

fn floating_panels(data: &mut AppData, i18n: &I18n, ctx: &egui::Context) {
    if !data.ruler.points.is_empty() {
        let mut open = true;
        egui::Window::new(tr(i18n, "ruler.panelTitle"))
            .open(&mut open)
            .default_pos([320.0, 560.0])
            .resizable(false)
            .show(ctx, |ui| {
                let segments = data.ruler.segment_meters();
                for (index, distance) in segments.iter().enumerate() {
                    ui.label(format!(
                        "{} {}: {}",
                        tr(i18n, "ruler.segment"),
                        index + 1,
                        format_distance(*distance, data.settings.unit_system)
                    ));
                }
                ui.separator();
                ui.label(format!(
                    "{}: {}",
                    tr(i18n, "ruler.total"),
                    format_distance(data.ruler.total_meters(), data.settings.unit_system)
                ));
                ui.horizontal(|ui| {
                    if ui.button(tr(i18n, "ruler.undo")).clicked() {
                        data.ruler.undo();
                    }
                    if ui.button(tr(i18n, "ruler.finish")).clicked() {
                        data.ruler.finish();
                        data.ruler_adding = false;
                    }
                    if ui.button(tr(i18n, "ruler.close")).clicked() {
                        data.ruler.clear();
                        data.ruler_adding = false;
                    }
                });
            });
        if !open {
            data.ruler.clear();
        }
    }

    if data.route.is_active() || data.route.pending {
        let mut open = true;
        egui::Window::new(tr(i18n, "routePlanner.panelTitle"))
            .open(&mut open)
            .default_pos([360.0, 200.0])
            .default_width(300.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(tr(i18n, "routePlanner.mode"));
                    let mode = data.route.mode.unwrap_or(RouteMode::Car);
                    egui::ComboBox::from_id_salt("route-mode")
                        .selected_text(tr(i18n, mode.label_key()))
                        .show_ui(ui, |ui| {
                            for candidate in [
                                RouteMode::Car,
                                RouteMode::Foot,
                                RouteMode::Bike,
                                RouteMode::Plane,
                                RouteMode::Train,
                            ] {
                                if ui
                                    .selectable_label(
                                        mode == candidate,
                                        tr(i18n, candidate.label_key()),
                                    )
                                    .clicked()
                                {
                                    data.route.mode = Some(candidate);
                                }
                            }
                        });
                });
                ui.label(format!(
                    "{} {}",
                    tr(i18n, "routePlanner.stop"),
                    data.route.waypoints.len()
                ));
                let mut remove = None;
                for (index, waypoint) in data.route.waypoints.iter().enumerate() {
                    ui.horizontal(|ui| {
                        ui.label(format!(
                            "{}. {}",
                            index + 1,
                            format_coordinate(waypoint, data.settings.coordinate_format)
                        ));
                        if ui
                            .small_button("x")
                            .on_hover_text(tr(i18n, "routePlanner.removeStop"))
                            .clicked()
                        {
                            remove = Some(index);
                        }
                    });
                }
                if let Some(index) = remove {
                    data.route.waypoints.remove(index);
                }
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            !data.route.pending,
                            egui::Button::new(tr(i18n, "routePlanner.calculate")),
                        )
                        .clicked()
                    {
                        data.start_route();
                    }
                    if ui.button(tr(i18n, "routePlanner.clear")).clicked() {
                        data.route.clear();
                        data.route_adding = false;
                    }
                    if data.route.pending {
                        ui.spinner();
                        ui.label(tr(i18n, "routePlanner.calculating"));
                    }
                });
                if let Some(error) = &data.route.error {
                    ui.colored_label(theme::DANGER, error);
                }
                for (index, leg) in data.route.legs.iter().enumerate() {
                    ui.label(format!(
                        "{} {} - {}",
                        tr(i18n, "routePlanner.routeAlternative")
                            .replace("{{n}}", &(index + 1).to_string()),
                        format_distance(leg.distance_meters, data.settings.unit_system),
                        terra_core::format_duration(leg.duration_seconds)
                    ));
                }
                if !data.route.legs.is_empty() {
                    ui.label(
                        egui::RichText::new(tr(i18n, "routePlanner.attribution"))
                            .small()
                            .color(theme::TEXT_MUTED),
                    );
                }
            });
        if !open {
            data.route.clear();
            data.route_adding = false;
        }
    }
}

fn modals(data: &mut AppData, i18n: &mut I18n, ctx: &egui::Context) {
    let Some(modal) = data.modal else {
        if data.confirm.is_some() {
            confirm_modal(data, i18n, ctx);
        }
        return;
    };
    match modal {
        ModalKind::Settings => settings_modal(data, i18n, ctx),
        ModalKind::About => about_modal(data, i18n, ctx),
        ModalKind::HeightProfile => height_profile_modal(data, i18n, ctx),
    }
    if data.confirm.is_some() {
        confirm_modal(data, i18n, ctx);
    }
}

fn settings_modal(data: &mut AppData, i18n: &mut I18n, ctx: &egui::Context) {
    let mut open = true;
    let mut language_changed = None;
    egui::Window::new(tr(i18n, "settings.title"))
        .open(&mut open)
        .resizable(false)
        .default_width(380.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new(tr(i18n, "settings.sectionUnits")).strong());
            ui.horizontal(|ui| {
                ui.label(tr(i18n, "settings.unitSystem"));
                for (system, key) in [
                    (UnitSystem::Metric, "settings.metric"),
                    (UnitSystem::Imperial, "settings.imperial"),
                ] {
                    if ui
                        .selectable_label(data.settings.unit_system == system, tr(i18n, key))
                        .clicked()
                    {
                        data.settings.unit_system = system;
                    }
                }
            });
            ui.horizontal(|ui| {
                ui.label(tr(i18n, "settings.coordinateFormat"));
                for (format, key) in [
                    (CoordinateFormat::Decimal, "settings.decimalDegrees"),
                    (CoordinateFormat::Dms, "settings.dms"),
                ] {
                    if ui
                        .selectable_label(data.settings.coordinate_format == format, tr(i18n, key))
                        .clicked()
                    {
                        data.settings.coordinate_format = format;
                    }
                }
            });
            ui.separator();
            ui.label(egui::RichText::new(tr(i18n, "settings.sectionLanguage")).strong());
            egui::ComboBox::from_label(tr(i18n, "settings.language"))
                .selected_text(data.settings.language.clone())
                .show_ui(ui, |ui| {
                    for code in crate::i18n::available_locales() {
                        if ui
                            .selectable_label(data.settings.language == code, code)
                            .clicked()
                        {
                            language_changed = Some(code.to_string());
                        }
                    }
                });
            ui.separator();
            ui.label(egui::RichText::new(tr(i18n, "settings.sectionSave")).strong());
            ui.checkbox(&mut data.settings.auto_save, tr(i18n, "settings.autoSave"));
            ui.checkbox(&mut data.settings.show_labels, "Labels");
            ui.separator();
            ui.label(egui::RichText::new(tr(i18n, "settings.sectionProviders")).strong());
            ui.horizontal(|ui| {
                egui::ComboBox::from_label(tr(i18n, "providers.geocodingKind"))
                    .selected_text(data.settings.geocoding_preset.clone())
                    .show_ui(ui, |ui| {
                        for preset in ["nominatim", "locationiq", "mapbox-geocoding", "opencage"] {
                            if ui
                                .selectable_label(data.settings.geocoding_preset == preset, preset)
                                .clicked()
                            {
                                data.settings.geocoding_preset = preset.to_string();
                                data.settings_draft.api_key.clear();
                            }
                        }
                    });
            });
            ui.horizontal(|ui| {
                egui::ComboBox::from_label(tr(i18n, "providers.routingKind"))
                    .selected_text(data.settings.routing_preset.clone())
                    .show_ui(ui, |ui| {
                        for preset in [
                            "osrm",
                            "graphhopper",
                            "openrouteservice",
                            "mapbox-directions",
                        ] {
                            if ui
                                .selectable_label(data.settings.routing_preset == preset, preset)
                                .clicked()
                            {
                                data.settings.routing_preset = preset.to_string();
                                data.settings_draft.api_key.clear();
                            }
                        }
                    });
            });
            let preset = data.settings.geocoding_preset.clone();
            let has_key = data.secret_for(&preset).is_some();
            ui.horizontal(|ui| {
                ui.label(tr(i18n, "providers.apiKey"));
                ui.name_inputs(tr(i18n, "providers.apiKey"), |ui| {
                    ui.add(
                        egui::TextEdit::singleline(&mut data.settings_draft.api_key)
                            .password(true)
                            .desired_width(160.0),
                    )
                });
                if ui.button(tr(i18n, "common.save")).clicked()
                    && !data.settings_draft.api_key.is_empty()
                {
                    if let Err(error) = data
                        .deps
                        .secrets
                        .set(&format!("provider:{preset}"), &data.settings_draft.api_key)
                    {
                        data.notify_error(error.to_string());
                    } else {
                        data.settings_draft.api_key.clear();
                        data.notify("API key saved");
                    }
                }
                ui.label(if has_key { "stored" } else { "no key" });
            });
            ui.label(
                egui::RichText::new(tr(i18n, "providers.desktopWarning"))
                    .small()
                    .color(theme::TEXT_MUTED),
            );
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button(tr(i18n, "common.close")).clicked() {
                    data.modal = None;
                }
            });
        });
    if let Some(locale) = language_changed {
        data.settings.language = locale.clone();
        i18n.set_locale(&locale);
    }
    if !open {
        data.modal = None;
        data.persist_settings();
    }
}

fn about_modal(data: &mut AppData, i18n: &I18n, ctx: &egui::Context) {
    let mut open = true;
    egui::Window::new(tr(i18n, "about.title").replace("{{name}}", "Terra Globe"))
        .open(&mut open)
        .resizable(false)
        .show(ctx, |ui| {
            ui.label(tr(i18n, "about.version").replace("{{version}}", env!("CARGO_PKG_VERSION")));
            ui.separator();
            ui.label("Copyright © Terra Globe contributors");
            ui.label(format!(
                "{} GNU General Public License v3",
                tr(i18n, "about.licensePrefix")
            ));
            ui.hyperlink_to(
                tr(i18n, "about.githubLink"),
                "https://github.com/floriankraemer/webglobe",
            );
            ui.separator();
            ui.label(egui::RichText::new(tr(i18n, "about.builtWith")).strong());
            ui.label("Rust, wgpu, egui, OpenStreetMap");
        });
    if !open {
        data.modal = None;
    }
}

fn height_profile_modal(data: &mut AppData, i18n: &I18n, ctx: &egui::Context) {
    let mut open = true;
    let geometry = data
        .selected
        .as_deref()
        .and_then(|id| data.store.library().placemark(id))
        .map(|placemark| placemark.geometry.clone());
    egui::Window::new(tr(i18n, "heightProfile.title"))
        .open(&mut open)
        .default_width(420.0)
        .resizable(false)
        .show(ctx, |ui| {
            let Some(PlacemarkGeometry::LineString { path, .. }) = geometry.as_ref() else {
                ui.label(tr(i18n, "importExport.nothingImportable"));
                return;
            };
            let profile = terra_core::domain::measurements::compute_track_profile(path);
            draw_profile_chart(ui, &profile, data.settings.unit_system);
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    tr(i18n, "ruler.total"),
                    format_distance(
                        profile
                            .last()
                            .map(|point| point.distance_meters)
                            .unwrap_or(0.0),
                        data.settings.unit_system
                    )
                ))
                .color(theme::TEXT_MUTED),
            );
        });
    if !open {
        data.modal = None;
    }
}

fn draw_profile_chart(
    ui: &mut egui::Ui,
    profile: &[terra_core::domain::measurements::TrackProfilePoint],
    unit_system: UnitSystem,
) {
    let height = 150.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    ui.painter().rect_filled(rect, 4.0, theme::SURFACE_RAISED);
    if profile.len() < 2 {
        return;
    }
    let max_distance = profile
        .last()
        .map(|point| point.distance_meters)
        .unwrap_or(1.0)
        .max(1.0);
    let min_altitude = profile
        .iter()
        .map(|point| point.altitude_meters)
        .fold(f64::INFINITY, f64::min);
    let max_altitude = profile
        .iter()
        .map(|point| point.altitude_meters)
        .fold(f64::NEG_INFINITY, f64::max);
    let span = (max_altitude - min_altitude).max(1.0);
    let points: Vec<egui::Pos2> = profile
        .iter()
        .map(|point| {
            egui::pos2(
                rect.left() + (point.distance_meters / max_distance) as f32 * rect.width(),
                rect.bottom()
                    - ((point.altitude_meters - min_altitude) / span) as f32 * rect.height(),
            )
        })
        .collect();
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(1.8, theme::ACCENT),
    ));
    ui.painter().text(
        rect.left_top() + egui::vec2(6.0, 6.0),
        egui::Align2::LEFT_TOP,
        format_distance(max_altitude, unit_system),
        egui::FontId::proportional(10.0),
        theme::TEXT_MUTED,
    );
    ui.painter().text(
        rect.left_bottom() + egui::vec2(6.0, -6.0),
        egui::Align2::LEFT_BOTTOM,
        format_distance(min_altitude, unit_system),
        egui::FontId::proportional(10.0),
        theme::TEXT_MUTED,
    );
}

fn confirm_modal(data: &mut AppData, i18n: &I18n, ctx: &egui::Context) {
    let Some(action) = data.confirm.clone() else {
        return;
    };
    let message = match &action {
        ConfirmAction::DeletePlacemark(_) => tr(i18n, "confirm.deletePlacemarkMessage"),
        ConfirmAction::DeleteFolder(_) => tr(i18n, "confirm.deletePlacemarkMessage"),
    };
    let open = true;
    let mut confirmed = false;
    let mut cancelled = false;
    egui::Modal::new(egui::Id::unique("confirm-delete")).show(ctx, |ui| {
        ui.heading(tr(i18n, "confirm.deletePlacemarkTitle"));
        ui.label(message);
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.button(tr(i18n, "common.cancel")).clicked() {
                cancelled = true;
            }
            if ui
                .button(egui::RichText::new(tr(i18n, "common.delete")).color(theme::DANGER))
                .clicked()
            {
                confirmed = true;
            }
        });
    });
    if cancelled {
        data.confirm = None;
        return;
    }
    if confirmed {
        let result = match &action {
            ConfirmAction::DeletePlacemark(id) => data.store.delete_placemark(id),
            ConfirmAction::DeleteFolder(id) => data.store.delete_folder(id),
        };
        match result {
            Ok(()) => {
                if matches!(action, ConfirmAction::DeletePlacemark(_)) {
                    data.close_editor();
                }
                data.autosave();
                data.notify(tr(i18n, "common.delete").to_string());
            }
            Err(error) => data.notify_error(error.to_string()),
        }
        data.confirm = None;
    }
    let _ = open;
}

fn notice_toast(data: &mut AppData, i18n: &I18n, ctx: &egui::Context, notice: &Notice) {
    if notice.created.elapsed().as_secs() > 6 {
        data.notice = None;
        return;
    }
    let color = if notice.error {
        theme::DANGER
    } else {
        theme::ACCENT
    };
    egui::Area::new(egui::Id::unique("notice"))
        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(16.0, -16.0))
        .show(ctx, |ui| {
            egui::Frame::NONE
                .fill(theme::SURFACE_RAISED)
                .stroke(egui::Stroke::new(1.0, color))
                .corner_radius(6.0)
                .inner_margin(egui::Margin::symmetric(10, 6))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.colored_label(color, &notice.message);
                        if ui.small_button(tr(i18n, "notice.dismiss")).clicked() {
                            data.notice = None;
                        }
                    });
                });
        });
    ctx.request_repaint_after(std::time::Duration::from_millis(200));
}
