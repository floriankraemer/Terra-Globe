//! Terra Globe desktop application. The composition root lives in
//! [`run`]; everything else is a feature module wired through the
//! `terra-core` ports.

pub const APP_NAME: &str = "Terra Globe";

/// Starts the native window.
pub fn run() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([1280.0, 800.0]),
        renderer: eframe::Renderer::Wgpu,
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|_cc| Ok(Box::new(Placeholder))))
}

struct Placeholder;

impl eframe::App for Placeholder {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        ui.label(APP_NAME);
    }
}
