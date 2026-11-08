//! Modal dialogs, the height profile chart, and notice toasts.

use eframe::egui;
use terra_core::{CoordinateFormat, PlacemarkGeometry, UnitSystem, format_distance};

use super::tr;
use crate::i18n::I18n;
use crate::{AppData, ConfirmAction, ModalKind, Notice, theme};

pub(super) fn modals(data: &mut AppData, i18n: &mut I18n, ctx: &egui::Context) {
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

/// Shown at the bottom centre of `globe`, above the attribution line, so it
/// never covers the side panels.
pub(super) fn notice_toast(
    data: &mut AppData,
    i18n: &I18n,
    ctx: &egui::Context,
    globe: egui::Rect,
    notice: &Notice,
) {
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
        .pivot(egui::Align2::CENTER_BOTTOM)
        .fixed_pos(globe.center_bottom() + egui::vec2(0.0, -36.0))
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
