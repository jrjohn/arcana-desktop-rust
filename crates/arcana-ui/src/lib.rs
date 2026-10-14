//! egui views and view models (C# `Arcana.App` views). Milestone 4.

/// Root shell. Milestone 4 replaces this with the dock, navigation and pages.
#[derive(Debug, Default)]
pub struct Shell;

impl Shell {
    /// Window title shown by the desktop entry point.
    pub const TITLE: &'static str = "Arcana";

    /// Draws the shell into the central panel.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        egui::CentralPanel::default().show(ui, |ui| {
            ui.heading(Self::TITLE);
            ui.label("arcana-desktop-rust — milestone 1 skeleton");
        });
    }
}
