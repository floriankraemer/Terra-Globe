//! Right panel: the placemark editor.

use eframe::egui;
use terra_core::{Patch, PlacemarkGeometry, PlacemarkPatch};

use super::tr;
use crate::i18n::I18n;
use crate::tools;
use crate::{AppData, ConfirmAction, ModalKind, OverlayEditor, theme};

pub(super) fn right_panel(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
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

    if let Some(overlay) = editor.overlay.as_mut() {
        ui.separator();
        overlay_fields(data, i18n, ui, overlay);
    }

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
    ui.spacing_mut().slider_width = 70.0;
    ui.horizontal(|ui| {
        ui.checkbox(
            &mut editor.style.outline_enabled,
            tr(i18n, "placemarkEditor.outline"),
        );
        color_button(
            ui,
            tr(i18n, "placemarkEditor.outlineColor"),
            &mut editor.style.outline_color,
        );
        let slider =
            ui.add(egui::Slider::new(&mut editor.style.outline_width, 0.5..=12.0).max_decimals(1));
        name_widget(ui, &slider, tr(i18n, "placemarkEditor.outlineWidth"));
    });
    ui.horizontal(|ui| {
        ui.checkbox(&mut editor.style.filled, tr(i18n, "placemarkEditor.filled"));
        color_button(
            ui,
            tr(i18n, "placemarkEditor.fillColor"),
            &mut editor.style.fill_color,
        );
        let slider =
            ui.add(egui::Slider::new(&mut editor.style.fill_opacity, 0.0..=1.0).max_decimals(2));
        name_widget(ui, &slider, tr(i18n, "layers.opacity"));
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
            let mut geometry = Patch::default();
            if let Some(overlay) = editor.overlay.as_mut() {
                match overlay.to_geometry() {
                    Ok(valid) => {
                        overlay.error = None;
                        geometry = Patch::Set(valid);
                    }
                    Err(error) => {
                        overlay.error = Some(error);
                        return;
                    }
                }
            }
            let patch = PlacemarkPatch {
                geometry,
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

fn color_button(ui: &mut egui::Ui, name: &str, hex: &mut String) {
    let mut color = theme::color32_from_hex(hex);
    let response = ui.color_edit_button_srgba(&mut color);
    name_widget(ui, &response, name);
    response.clone().on_hover_text(name);
    if response.changed() {
        *hex = theme::hex_from_color32(color);
    }
}

/// Gives a widget an accessible name where it cannot be built with one.
fn name_widget(ui: &egui::Ui, response: &egui::Response, name: &str) {
    ui.ctx()
        .accesskit_node_builder(response.id, |node| node.set_label(name));
}

fn overlay_fields(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui, overlay: &mut OverlayEditor) {
    for (key, value) in [
        ("imageOverlay.north", &mut overlay.north),
        ("imageOverlay.south", &mut overlay.south),
        ("imageOverlay.east", &mut overlay.east),
        ("imageOverlay.west", &mut overlay.west),
        ("imageOverlay.rotation", &mut overlay.rotation),
    ] {
        ui.name_inputs(tr(i18n, key), |ui| {
            ui.add(egui::TextEdit::singleline(value).desired_width(f32::INFINITY))
        });
    }
    let mut percent = (overlay.opacity * 100.0).round();
    ui.add(
        egui::Slider::new(&mut percent, 0.0..=100.0)
            .integer()
            .suffix("%")
            .text(tr(i18n, "imageOverlay.opacity")),
    );
    overlay.opacity = (percent / 100.0).clamp(0.0, 1.0);
    ui.name_inputs(tr(i18n, "imageOverlay.image"), |ui| {
        ui.add(egui::TextEdit::singleline(&mut overlay.href).desired_width(f32::INFINITY))
    });
    if ui.button(tr(i18n, "imageOverlay.change")).clicked() {
        if let Some((_, href)) = data.pick_image() {
            overlay.href = href;
        }
    }
    if let Some(error) = &overlay.error {
        let message = i18n.t_args("imageOverlay.invalid", &[("error", error)]);
        ui.label(egui::RichText::new(message).color(theme::DANGER));
    }
}
