//! Desktop entry point (C# `Arcana.App`).

mod bootstrap;
mod smoke;

use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use arcana_ui::{Shell, UiServices, fonts};

struct App {
    shell: Shell,
    smoke: Option<smoke::Smoke>,
    // Keeps the async runtime (and with it every in-flight task) alive for the window.
    _runtime: tokio::runtime::Runtime,
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.shell.show(ui);
        if let Some(s) = &mut self.smoke {
            s.frame(ui.ctx());
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let paths = bootstrap::AppPaths::default_location();
    let _log_guard = arcana_infra::logging::init_logging(&paths.data_dir.join("logs")).ok();
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    let services = runtime.block_on(bootstrap::bootstrap(&paths))?;
    let network = bootstrap::network_monitor();
    let ui_services = UiServices::from_arcana(&services, network as Arc<_>, &paths.data_dir);
    let shell = Shell::new(ui_services, runtime.handle().clone());
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(Shell::TITLE)
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([900.0, 600.0]),
        ..eframe::NativeOptions::default()
    };
    // `ARCANA_SMOKE_SCREENSHOT=<file.ppm>`: CI GUI smoke test (see `smoke`).
    let exit = Arc::new(AtomicI32::new(0));
    let smoke = std::env::var_os("ARCANA_SMOKE_SCREENSHOT")
        .map(|p| smoke::Smoke::new(p.into(), exit.clone()));
    let mut shell = shell;
    eframe::run_native(
        Shell::TITLE,
        options,
        Box::new(move |cc| {
            shell.set_font_status(&fonts::install(&cc.egui_ctx));
            Ok(Box::new(App {
                shell,
                smoke,
                _runtime: runtime,
            }))
        }),
    )?;
    match exit.load(Ordering::SeqCst) {
        0 => Ok(()),
        code => std::process::exit(code),
    }
}
