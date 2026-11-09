//! Top bar and the drawing toolbar.

use eframe::egui;
use terra_core::PlacemarkGeometry;
use terra_globe::{Camera, DrawTool, SceneMode};

use super::{apply_undo, tr};
use crate::i18n::I18n;
use crate::{AppData, ModalKind, theme};

pub(super) fn top_bar(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
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
    let search_hint = tr(i18n, "geocoding.placeholder").to_string();
    let search_label = tr(i18n, "geocoding.search").to_string();
    let searching = tr(i18n, "geocoding.searching").to_string();
    let no_results = tr(i18n, "geocoding.noResults").to_string();
    let labels_label = tr(i18n, "placemarkEditor.showDistanceRings").to_string();
    let reset_label = "Reset view".to_string();

    egui::Panel::top("topbar").show(ui, |ui| {
        ui.horizontal_wrapped(|ui| {
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
            ui.separator();
            draw_toolbar(data, i18n, ui);
            // Right-align the group when it fits on the current line, otherwise
            // wrap it onto its own line; its width is measured from the last frame.
            // `available_width` is the whole row in a wrapping layout, so the room
            // left on this line has to come from `available_rect_before_wrap`.
            let width_id = ui.scope_id().with("right-width");
            let width: f32 = ui.data(|d| d.get_temp(width_id)).unwrap_or(0.0);
            let spacing = ui.spacing().item_spacing.x;
            let mut remaining = ui.available_rect_before_wrap().width();
            if remaining < width + spacing {
                ui.end_row();
                remaining = ui.available_rect_before_wrap().width();
            }
            if width > 0.0 {
                ui.add_space((remaining - width - spacing).max(0.0));
            }
            let settings = ui.button(settings_label);
            if settings.clicked() {
                data.modal = Some(ModalKind::Settings);
            }
            let about = ui.button(about_label);
            if about.clicked() {
                data.modal = Some(ModalKind::About);
            }
            let measured = about.rect.right() - settings.rect.left();
            if (measured - width).abs() > 0.5 {
                ui.data_mut(|d| d.insert_temp(width_id, measured));
                ui.ctx().request_repaint();
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
    let mut label = tr(i18n, "drawingToolbar.geometryLabel").to_string();
    if let Some((_, key)) = tools.iter().find(|(t, _)| data.drawing.tool() == Some(*t)) {
        label = format!("{label}: {}", tr(i18n, key));
    }
    ui.menu_button(label, |ui| {
        for (tool, key) in tools {
            let active = data.drawing.tool() == Some(tool);
            if ui.selectable_label(active, tr(i18n, key)).clicked() {
                data.drawing.start(tool);
                data.ruler_adding = false;
                data.route_adding = false;
                ui.close();
            }
        }
        ui.separator();
        if ui.button(tr(i18n, "imageOverlay.menu")).clicked() {
            data.add_image_overlay();
            ui.close();
        }
    });
    if data.drawing.is_active() {
        if ui.button(tr(i18n, "drawingToolbar.finish")).clicked() {
            finish_drawing(data);
        }
        if ui.button(tr(i18n, "drawingToolbar.cancel")).clicked() {
            data.drawing.cancel();
        }
    }
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
}

fn finish_drawing(data: &mut AppData) {
    if let Some(geometry) = data.drawing.finish() {
        create_placemark(data, geometry);
    }
}

pub(super) fn create_placemark(data: &mut AppData, geometry: PlacemarkGeometry) {
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
