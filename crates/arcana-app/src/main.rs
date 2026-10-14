//! Desktop entry point (C# `Arcana.App`).

use arcana_ui::Shell;

struct App {
    shell: Shell,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.show(ui);
    }
}

fn main() -> eframe::Result {
    eframe::run_native(
        Shell::TITLE,
        eframe::NativeOptions::default(),
        Box::new(|_cc| Ok(Box::new(App { shell: Shell }))),
    )
}
