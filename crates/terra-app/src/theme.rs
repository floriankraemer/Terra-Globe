//! Dark theme tokens matching the web app's look.

use eframe::egui;

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(94, 164, 255);
pub const SURFACE: egui::Color32 = egui::Color32::from_rgb(24, 27, 34);
pub const SURFACE_RAISED: egui::Color32 = egui::Color32::from_rgb(31, 35, 44);
pub const TEXT: egui::Color32 = egui::Color32::from_rgb(226, 230, 238);
pub const TEXT_MUTED: egui::Color32 = egui::Color32::from_rgb(150, 158, 172);
/// Tree rows: hover fill and selection fill (the accent blended into the surface).
pub const ROW_HOVER: egui::Color32 = egui::Color32::from_rgb(41, 46, 58);
pub const ROW_SELECTED: egui::Color32 = egui::Color32::from_rgb(58, 74, 107);
pub const DANGER: egui::Color32 = egui::Color32::from_rgb(232, 93, 86);
pub const SPACE: egui::Color32 = egui::Color32::from_rgb(5, 7, 12);

pub fn apply(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.panel_fill = SURFACE;
    visuals.window_fill = SURFACE_RAISED;
    visuals.extreme_bg_color = egui::Color32::from_rgb(16, 18, 24);
    visuals.faint_bg_color = SURFACE_RAISED;
    visuals.override_text_color = Some(TEXT);
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, egui::Color32::from_rgb(44, 49, 60));
    visuals.widgets.inactive.bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.weak_bg_fill = SURFACE_RAISED;
    visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(41, 46, 58);
    visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, egui::Color32::WHITE);
    visuals.widgets.active.bg_fill = egui::Color32::from_rgb(52, 58, 72);
    visuals.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);
    visuals.hyperlink_color = ACCENT;
    ctx.set_visuals(visuals);

    ctx.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.spacing.interact_size.y = 24.0;
        style
            .text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(17.0));
        style
            .text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(13.0));
        style
            .text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(11.0));
    });
}

/// Parses a `#rrggbb` string for the colour picker.
pub fn color32_from_hex(hex: &str) -> egui::Color32 {
    let channel = |range: std::ops::Range<usize>| {
        u8::from_str_radix(hex.get(range).unwrap_or("ff"), 16).unwrap_or(255)
    };
    egui::Color32::from_rgb(channel(1..3), channel(3..5), channel(5..7))
}

pub fn hex_from_color32(color: egui::Color32) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r(), color.g(), color.b())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_round_trip_through_hex() {
        for hex in ["#3366cc", "#ffffff", "#000000", "#abcdef"] {
            assert_eq!(hex_from_color32(color32_from_hex(hex)), hex);
        }
    }
}
