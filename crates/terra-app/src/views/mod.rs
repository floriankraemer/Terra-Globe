//! All UI views. Kept in one module because they share one mutable state object.

use eframe::egui;

use self::editor::right_panel;
use self::globe_view::central_globe;
use self::globe_view::floating_panels;
use self::library::left_panel;
use self::modals::modals;
use self::modals::notice_toast;
use self::top_bar::top_bar;
use crate::AppData;
use crate::i18n::I18n;

mod editor;
mod globe_view;
mod layers;
mod library;
mod modals;
pub mod nav_controls;
mod top_bar;
mod tree;

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
    data.sync_images();
    if data.images_loading() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    shortcuts(data, ui.ctx());

    top_bar(data, i18n, ui);
    left_panel(data, i18n, ui);
    right_panel(data, i18n, ui);
    let globe_area = central_globe(data, i18n, ui, render_state);
    floating_panels(data, i18n, ui.ctx());
    modals(data, i18n, ui.ctx());
    if let Some(notice) = data.notice.clone() {
        notice_toast(data, i18n, ui.ctx(), globe_area, &notice);
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
