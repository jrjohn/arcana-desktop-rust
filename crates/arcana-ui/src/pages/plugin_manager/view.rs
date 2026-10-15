use egui::{Color32, RichText, Ui};

use super::{Health, Input, PluginState, RuntimeAction, State};
use crate::widgets::{ViewCx, badge, card, page_title, subtitle, tool_button};

fn state_badge(ui: &mut Ui, cx: &ViewCx<'_>, s: PluginState) {
    let key = match s {
        PluginState::Active => "ui.plugins.active",
        PluginState::Loaded => "ui.plugins.loaded",
    };
    badge(ui, &cx.t(key), cx.palette.success);
}

fn health_badge(ui: &mut Ui, cx: &ViewCx<'_>, h: Health) {
    match h {
        Health::Healthy => badge(ui, &cx.t("ui.plugins.healthy"), cx.palette.success),
        Health::Unknown => badge(ui, &cx.t("ui.plugins.unknown"), Color32::GRAY),
    }
}

fn runtime_button(ui: &mut Ui, cx: &ViewCx<'_>, a: RuntimeAction) {
    ui.add_enabled(false, egui::Button::new(cx.t(a.label_key())))
        .on_disabled_hover_text(cx.t("ui.plugins.runtimeNote"));
}

fn list(ui: &mut Ui, s: &State, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| page_title(ui, &cx.t("nav.plugins"), Some(&cx.t("ui.plugins.subtitle"))));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if tool_button(ui, "♡", &cx.t("ui.plugins.checkHealth"), true) {
                out.push(Input::CheckAllHealth);
            }
            if tool_button(ui, "⟳", &cx.t("common.refresh"), true) {
                out.push(Input::Refresh);
            }
            ui.add_enabled(
                false,
                egui::Button::new(format!("➕ {}", cx.t("ui.plugins.install"))).frame(false),
            )
            .on_disabled_hover_text(cx.t("ui.plugins.runtimeNote"));
        });
    });
    let mut q = s.search.clone();
    if ui
        .add(
            egui::TextEdit::singleline(&mut q)
                .hint_text(cx.t("ui.plugins.search"))
                .desired_width(f32::INFINITY),
        )
        .changed()
    {
        out.push(Input::SearchChanged(q));
    }
    ui.add_space(8.0);
    let visible = s.visible();
    for p in &visible {
        let selected = s.selected == Some(p.id);
        let r = ui
            .scope(|ui| {
                card(ui, cx, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("🔌").size(28.0).color(cx.palette.info));
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                if ui
                                    .selectable_label(selected, RichText::new(p.name).strong())
                                    .clicked()
                                {
                                    out.push(Input::Select(p.id));
                                }
                                ui.label(RichText::new(p.version).weak());
                            });
                            ui.label(RichText::new(p.description).small());
                            ui.label(
                                RichText::new(format!("{}   {}", p.kind, p.author.unwrap_or("")))
                                    .small()
                                    .weak(),
                            );
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.vertical(|ui| {
                                state_badge(ui, cx, p.state);
                                health_badge(ui, cx, p.health);
                            });
                            if selected {
                                ui.label(RichText::new("▶").color(cx.palette.accent));
                            }
                        });
                    });
                });
            })
            .response
            .interact(egui::Sense::click());
        if r.clicked() {
            out.push(Input::Select(p.id));
        }
    }
    ui.label(cx.tw("ui.plugins.loadedFormat", &[visible.len().to_string()]));
}

fn details(ui: &mut Ui, s: &State, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    let Some(p) = s.selected_plugin() else {
        ui.label(RichText::new(cx.t("ui.plugins.selectHint")).weak());
        return;
    };
    ui.label(RichText::new(p.name).size(22.0).strong());
    ui.label(p.description);
    ui.horizontal(|ui| {
        for a in [
            RuntimeAction::Activate,
            RuntimeAction::Deactivate,
            RuntimeAction::Reload,
        ] {
            runtime_button(ui, cx, a);
        }
    });
    ui.separator();
    subtitle(ui, &cx.t("ui.plugins.information"));
    egui::Grid::new("plugin_info")
        .num_columns(2)
        .show(ui, |ui| {
            let state = match p.state {
                PluginState::Active => cx.t("ui.plugins.active"),
                PluginState::Loaded => cx.t("ui.plugins.loaded"),
            };
            for (k, v) in [
                ("ui.plugins.id", p.id.to_owned()),
                ("ui.plugins.version", p.version.to_owned()),
                ("ui.plugins.author", p.author.unwrap_or("").to_owned()),
                ("ui.plugins.type", p.kind.to_owned()),
                ("ui.plugins.state", state),
            ] {
                ui.label(cx.t(k));
                ui.label(v);
                ui.end_row();
            }
        });
    ui.separator();
    ui.horizontal(|ui| {
        subtitle(ui, &cx.t("ui.plugins.health"));
        if ui.button(cx.t("ui.plugins.check")).clicked() {
            out.push(Input::CheckHealth(p.id));
        }
    });
    egui::Grid::new("plugin_health")
        .num_columns(2)
        .show(ui, |ui| {
            let status = match p.health {
                Health::Healthy => cx.t("ui.plugins.healthy"),
                Health::Unknown => cx.t("ui.plugins.unknown"),
            };
            for (k, v) in [
                ("ui.plugins.status", status),
                ("ui.plugins.errorCount", p.error_count.to_string()),
            ] {
                ui.label(cx.t(k));
                ui.label(v);
                ui.end_row();
            }
        });
    ui.separator();
    subtitle(ui, &cx.t("ui.plugins.versions"));
    ui.horizontal(|ui| {
        for a in [
            RuntimeAction::Upgrade,
            RuntimeAction::Uninstall,
            RuntimeAction::Rollback,
        ] {
            runtime_button(ui, cx, a);
        }
    });
    ui.label(RichText::new(cx.t("ui.plugins.runtimeNote")).small().weak());
}

/// Draw the plugin manager; returns the inputs the user produced.
pub fn show(ui: &mut Ui, s: &State, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out = Vec::new();
    let width = ui.available_width();
    let side = (width * 0.32).clamp(260.0, 400.0);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(width - side - 12.0, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::ScrollArea::vertical()
                    .id_salt("plugins_list")
                    .show(ui, |ui| list(ui, s, cx, &mut out));
            },
        );
        ui.allocate_ui_with_layout(
            egui::vec2(side, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                egui::Frame::new()
                    .fill(cx.palette.pane)
                    .inner_margin(16)
                    .show(ui, |ui| {
                        ui.set_min_size(egui::vec2(side - 32.0, ui.available_height()));
                        details(ui, s, cx, &mut out);
                    });
            },
        );
    });
    out
}
