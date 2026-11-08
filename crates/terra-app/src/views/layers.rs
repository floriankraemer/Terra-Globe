//! Left panel, lower section: base map choice and transparent tile overlays.

use eframe::egui;

use super::tr;
use super::tree::{self, IconKind, RowControl, RowSpec};
use crate::AppData;
use crate::i18n::I18n;

/// Layers never take more than this share of the left panel; Places gets the rest.
const MAX_SHARE: f32 = 0.5;
/// Section separator and title row above the scroll area.
const HEADER_HEIGHT: f32 = 40.0;
const ROW_MIN: f32 = 60.0;

/// Draws the collapse chevron and the section title; returns whether it is open.
pub(super) fn section_toggle(ui: &mut egui::Ui, i18n: &I18n, title: &str, key: &str) -> bool {
    let open_id = egui::Id::unique(("section-open", key));
    let mut open = ui
        .data_mut(|d| d.get_persisted::<bool>(open_id))
        .unwrap_or(true);
    let (rect, toggle) = ui.allocate_exact_size(egui::vec2(16.0, 16.0), egui::Sense::click());
    let label = i18n.t_args(
        if open {
            "folders.collapse"
        } else {
            "folders.expand"
        },
        &[("name", title)],
    );
    toggle.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label.clone()));
    tree::paint_chevron(ui.painter(), rect.center(), open, 4.0);
    if toggle.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(open_id, open));
    }
    ui.heading(title);
    open
}

fn height_id() -> egui::Id {
    egui::Id::unique("layers-section-height")
}

/// Height the section took last frame, so the Places tree above it can leave room.
pub(super) fn layers_height(ui: &egui::Ui) -> f32 {
    ui.data(|d| d.get_temp(height_id())).unwrap_or(0.0)
}

pub(super) fn layers_section(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    panel_height: f32,
) {
    let max_height = (panel_height * MAX_SHARE - HEADER_HEIGHT).max(ROW_MIN);
    let section = ui.scope(|ui| {
        ui.separator();
        let title = tr(i18n, "layers.header").to_string();
        let open = ui
            .horizontal(|ui| section_toggle(ui, i18n, &title, "layers"))
            .inner;
        if !open {
            return None;
        }
        let area = egui::ScrollArea::vertical()
            .id_salt("layers-scroll")
            .max_height(max_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                terrain_row(data, i18n, ui);
                base_map_group(data, i18n, ui);
                overlay_group(data, i18n, ui);
            });
        Some((
            area.inner_rect.height(),
            area.content_size.y.min(max_height),
        ))
    });
    // The height the section wants, not the (possibly squeezed) one it got, or
    // the Places tree above would never leave it room to grow.
    let height = match section.inner {
        Some((shown, wanted)) => section.response.rect.height() - shown + wanted,
        None => section.response.rect.height(),
    };
    if (height - layers_height(ui)).abs() > 0.5 {
        ui.data_mut(|d| d.insert_temp(height_id(), height));
        ui.ctx().request_repaint();
    }
}

fn group_open(ui: &egui::Ui, key: &str) -> (egui::Id, bool) {
    let id = egui::Id::unique(("layers-group-open", key));
    (id, ui.data_mut(|d| d.get_persisted(id)).unwrap_or(true))
}

/// Draws a group row and applies a click on it or its chevron.
fn group_row(
    ui: &mut egui::Ui,
    i18n: &I18n,
    key: &str,
    title: &str,
    has_sibling_below: bool,
) -> bool {
    let (open_id, mut open) = group_open(ui, key);
    let chevron_label = i18n.t_args(
        if open {
            "folders.collapse"
        } else {
            "folders.expand"
        },
        &[("name", title)],
    );
    let spec = RowSpec {
        id: egui::Id::unique(("layers-row", key)),
        depth: 0,
        ancestors_have_sibling_below: &[],
        has_sibling_below,
        has_children: true,
        open,
        icon: if open {
            IconKind::FolderOpen
        } else {
            IconKind::FolderClosed
        },
        tint: None,
        label: title,
        selected: false,
        dimmed: false,
        visible: true,
        control: RowControl::None,
        eye_label: "",
        chevron_label: &chevron_label,
        accepts_drop: false,
    };
    let out = tree::tree_row(ui, &spec);
    if out.chevron_clicked || out.row.clicked() {
        open = !open;
        ui.data_mut(|d| d.insert_persisted(open_id, open));
    }
    open
}

/// The 3D terrain switch, at the top of the list: it is not a map, it changes
/// how every map is drawn. Hidden where the build cannot show terrain.
fn terrain_row(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    if !crate::TERRAIN_SUPPORTED {
        return;
    }
    let title = tr(i18n, "layers.terrain").to_string();
    let enabled = data.settings.terrain_enabled;
    let label = i18n.t_args("layers.toggle", &[("name", &title)]);
    let spec = RowSpec {
        id: egui::Id::unique(("layers-row", "terrain")),
        depth: 0,
        ancestors_have_sibling_below: &[],
        has_sibling_below: true,
        has_children: false,
        open: false,
        icon: IconKind::Layer,
        tint: None,
        label: &title,
        selected: false,
        dimmed: !enabled,
        visible: enabled,
        control: RowControl::Eye,
        eye_label: &label,
        chevron_label: "",
        accepts_drop: false,
    };
    let out = tree::tree_row(ui, &spec);
    if out.eye_clicked {
        data.set_terrain_enabled(!enabled);
    }
}

fn base_map_group(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let title = tr(i18n, "layers.baseMap").to_string();
    if !group_row(ui, i18n, "base-map", &title, true) {
        return;
    }
    let sources = terra_providers::builtin_tile_sources();
    let last = sources.len();
    for (index, source) in sources.iter().enumerate() {
        let chosen = data.settings.tile_source_id == source.id;
        let label = i18n.t_args("layers.useBaseMap", &[("name", &source.name)]);
        let spec = RowSpec {
            id: egui::Id::unique(("layers-row", &source.id)),
            depth: 1,
            ancestors_have_sibling_below: &[true],
            has_sibling_below: index + 1 < last,
            has_children: false,
            open: false,
            icon: IconKind::Layer,
            tint: None,
            label: &source.name,
            selected: data.selected_layer.as_deref() == Some(source.id.as_str()),
            dimmed: false,
            visible: chosen,
            control: RowControl::Radio,
            eye_label: &label,
            chevron_label: "",
            accepts_drop: false,
        };
        let out = tree::tree_row(ui, &spec);
        if out.eye_clicked || out.row.clicked() {
            data.selected_layer = Some(source.id.clone());
            data.set_basemap(&source.id);
        }
    }
}

fn overlay_group(data: &mut AppData, i18n: &I18n, ui: &mut egui::Ui) {
    let title = tr(i18n, "layers.overlays").to_string();
    if !group_row(ui, i18n, "overlays", &title, false) {
        return;
    }
    let presets = terra_providers::overlay_presets();
    let last = presets.len();
    for (index, source) in presets.iter().enumerate() {
        let (enabled, opacity) = data.layer_state(&source.id);
        let selected = data.selected_layer.as_deref() == Some(source.id.as_str());
        let label = i18n.t_args("layers.toggle", &[("name", &source.name)]);
        let spec = RowSpec {
            id: egui::Id::unique(("layers-row", &source.id)),
            depth: 1,
            ancestors_have_sibling_below: &[false],
            has_sibling_below: index + 1 < last,
            has_children: false,
            open: false,
            icon: IconKind::Layer,
            tint: None,
            label: &source.name,
            selected,
            dimmed: !enabled,
            visible: enabled,
            control: RowControl::Eye,
            eye_label: &label,
            chevron_label: "",
            accepts_drop: false,
        };
        let out = tree::tree_row(ui, &spec);
        if out.eye_clicked {
            data.selected_layer = Some(source.id.clone());
            data.set_layer_enabled(&source.id, !enabled);
        } else if out.row.clicked() {
            data.selected_layer = Some(source.id.clone());
        }
        if selected {
            opacity_row(data, i18n, ui, &source.id, opacity, index + 1 < last);
        }
    }
}

fn opacity_row(
    data: &mut AppData,
    i18n: &I18n,
    ui: &mut egui::Ui,
    id: &str,
    opacity: f32,
    has_sibling_below: bool,
) {
    // Room the "100%" value box takes at the end of the slider.
    const VALUE_WIDTH: f32 = 50.0;
    let mut percent = (opacity * 100.0).round();
    let left = ui.cursor().left();
    let spacing_y = ui.spacing().item_spacing.y;
    let row = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        ui.add_space(tree::content_indent(1));
        let label = ui.label(tr(i18n, "layers.opacity"));
        let spacing = ui.spacing().item_spacing.x;
        ui.spacing_mut().slider_width = (ui.available_width() - VALUE_WIDTH - spacing).max(40.0);
        let response = ui
            .add(
                egui::Slider::new(&mut percent, 0.0..=100.0)
                    .integer()
                    .suffix("%"),
            )
            .labelled_by(label.id);
        if response.changed() {
            data.set_layer_opacity(id, percent / 100.0);
        }
    });
    if has_sibling_below {
        let rect = row.response.rect;
        let full = egui::Rect::from_min_max(
            egui::pos2(left, rect.top() - spacing_y),
            egui::pos2(rect.right(), rect.bottom() + spacing_y),
        );
        tree::paint_guide_through(ui.painter(), full, 1);
    }
}
