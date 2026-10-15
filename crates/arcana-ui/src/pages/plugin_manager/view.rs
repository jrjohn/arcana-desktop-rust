use arcana_plugin_api::schema::{SettingField, SettingKind};
use arcana_plugin_api::{HealthState, PluginState};
use egui::{Color32, RichText, Ui};

use super::{Input, PluginRow, RuntimeAction, State};
use crate::widgets::{ViewCx, badge, card, page_title, subtitle, tool_button};

fn state_text(cx: &ViewCx<'_>, s: PluginState) -> String {
    match s {
        PluginState::Active => cx.t("ui.plugins.active"),
        PluginState::Loaded | PluginState::NotLoaded => cx.t("ui.plugins.loaded"),
        other => format!("{other:?}"),
    }
}

fn state_badge(ui: &mut Ui, cx: &ViewCx<'_>, s: PluginState) {
    let fill = match s {
        PluginState::Active => cx.palette.success,
        PluginState::Error => Color32::from_rgb(196, 43, 28),
        _ => Color32::GRAY,
    };
    badge(ui, &state_text(cx, s), fill);
}

fn health_text(cx: &ViewCx<'_>, h: HealthState) -> String {
    match h {
        HealthState::Healthy => cx.t("ui.plugins.healthy"),
        HealthState::Unknown => cx.t("ui.plugins.unknown"),
        other => other.name().to_string(),
    }
}

fn health_badge(ui: &mut Ui, cx: &ViewCx<'_>, h: HealthState) {
    let fill = match h {
        HealthState::Healthy => cx.palette.success,
        HealthState::Degraded => Color32::from_rgb(230, 140, 0),
        HealthState::Unhealthy => Color32::from_rgb(196, 43, 28),
        HealthState::Unknown => Color32::GRAY,
    };
    badge(ui, &health_text(cx, h), fill);
}

fn action_button(ui: &mut Ui, s: &State, cx: &ViewCx<'_>, a: RuntimeAction, out: &mut Vec<Input>) {
    if ui
        .add_enabled(s.can(a), egui::Button::new(cx.t(a.label_key())))
        .clicked()
    {
        out.push(Input::Runtime(a));
    }
}

fn plugin_card(ui: &mut Ui, s: &State, p: &PluginRow, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    let selected = s.selected.as_deref() == Some(p.id.as_str());
    let r = ui
        .scope(|ui| {
            card(ui, cx, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(RichText::new("🔌").size(28.0).color(cx.palette.info));
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            if ui
                                .selectable_label(selected, RichText::new(&p.name).strong())
                                .clicked()
                            {
                                out.push(Input::Select(p.id.clone()));
                            }
                            ui.label(RichText::new(&p.version).weak());
                        });
                        ui.label(RichText::new(&p.description).small());
                        ui.label(
                            RichText::new(format!(
                                "{}   {}",
                                p.kind,
                                p.author.as_deref().unwrap_or("")
                            ))
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
        out.push(Input::Select(p.id.clone()));
    }
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
        });
    });
    ui.horizontal(|ui| {
        let mut path = s.package_path.clone();
        if ui
            .add(
                egui::TextEdit::singleline(&mut path)
                    .hint_text(cx.t("ui.plugins.packagePath"))
                    .desired_width(ui.available_width() - 120.0),
            )
            .changed()
        {
            out.push(Input::PackagePathChanged(path));
        }
        action_button(ui, s, cx, RuntimeAction::Install, out);
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
        plugin_card(ui, s, p, cx, out);
    }
    ui.label(cx.tw("ui.plugins.loadedFormat", &[visible.len().to_string()]));
}

fn setting_widget(ui: &mut Ui, f: &SettingField, v: &serde_json::Value, out: &mut Vec<Input>) {
    let send = |out: &mut Vec<Input>, value| out.push(Input::SettingChanged(f.key.clone(), value));
    match &f.kind {
        SettingKind::Bool => {
            let mut b = v.as_bool().unwrap_or(false);
            if ui.checkbox(&mut b, "").changed() {
                send(out, serde_json::Value::Bool(b));
            }
        }
        SettingKind::Integer { min, max } => {
            let mut n = v.as_i64().unwrap_or(0);
            #[allow(clippy::cast_possible_truncation)]
            let range = min.map_or(i64::MIN, |m| m as i64)..=max.map_or(i64::MAX, |m| m as i64);
            if ui.add(egui::DragValue::new(&mut n).range(range)).changed() {
                send(out, serde_json::Value::from(n));
            }
        }
        SettingKind::Number { min, max } => {
            let mut n = v.as_f64().unwrap_or(0.0);
            let range = min.unwrap_or(f64::MIN)..=max.unwrap_or(f64::MAX);
            if ui.add(egui::DragValue::new(&mut n).range(range)).changed() {
                send(out, serde_json::Value::from(n));
            }
        }
        SettingKind::Text => {
            let mut t = v.as_str().unwrap_or_default().to_string();
            if ui.text_edit_singleline(&mut t).changed() {
                send(out, serde_json::Value::String(t));
            }
        }
        SettingKind::Choice(options) => {
            let current = options
                .iter()
                .find(|(x, _)| x == v)
                .map_or_else(String::new, |(_, l)| l.clone());
            egui::ComboBox::from_id_salt(("plugin_setting", &f.key))
                .selected_text(current)
                .show_ui(ui, |ui| {
                    for (value, label) in options {
                        if ui.selectable_label(value == v, label).clicked() {
                            send(out, value.clone());
                        }
                    }
                });
        }
    }
}

fn details(ui: &mut Ui, s: &State, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    let Some(p) = s.selected_plugin() else {
        ui.label(RichText::new(cx.t("ui.plugins.selectHint")).weak());
        return;
    };
    ui.label(RichText::new(&p.name).size(22.0).strong());
    ui.label(&p.description);
    ui.horizontal_wrapped(|ui| {
        for a in [
            RuntimeAction::Activate,
            RuntimeAction::Deactivate,
            RuntimeAction::Reload,
        ] {
            action_button(ui, s, cx, a, out);
        }
    });
    ui.separator();
    subtitle(ui, &cx.t("ui.plugins.information"));
    egui::Grid::new("plugin_info")
        .num_columns(2)
        .show(ui, |ui| {
            for (k, v) in [
                ("ui.plugins.id", p.id.clone()),
                ("ui.plugins.version", p.version.clone()),
                ("ui.plugins.author", p.author.clone().unwrap_or_default()),
                ("ui.plugins.type", p.kind.clone()),
                ("ui.plugins.state", state_text(cx, p.state)),
                ("ui.plugins.permissions", p.granted_permissions.clone()),
            ] {
                ui.label(cx.t(k));
                ui.label(v);
                ui.end_row();
            }
        });
    if !p.pending_permissions.is_empty() {
        ui.label(
            RichText::new(cx.tw(
                "ui.plugins.pendingPermissions",
                std::slice::from_ref(&p.pending_permissions),
            ))
            .color(Color32::from_rgb(230, 140, 0)),
        );
        action_button(ui, s, cx, RuntimeAction::Approve, out);
    }
    ui.separator();
    ui.horizontal(|ui| {
        subtitle(ui, &cx.t("ui.plugins.health"));
        if ui.button(cx.t("ui.plugins.check")).clicked() {
            out.push(Input::CheckHealth(p.id.clone()));
        }
    });
    egui::Grid::new("plugin_health")
        .num_columns(2)
        .show(ui, |ui| {
            for (k, v) in [
                ("ui.plugins.status", health_text(cx, p.health)),
                ("ui.plugins.errorCount", p.error_count.to_string()),
            ] {
                ui.label(cx.t(k));
                ui.label(v);
                ui.end_row();
            }
        });
    ui.separator();
    subtitle(ui, &cx.t("ui.plugins.versions"));
    ui.horizontal_wrapped(|ui| {
        egui::ComboBox::from_id_salt("rollback_version")
            .selected_text(s.rollback_to.clone().unwrap_or_default())
            .show_ui(ui, |ui| {
                for v in &s.versions {
                    if ui
                        .selectable_label(s.rollback_to.as_ref() == Some(v), v)
                        .clicked()
                    {
                        out.push(Input::RollbackTarget(v.clone()));
                    }
                }
            });
        for a in [
            RuntimeAction::Rollback,
            RuntimeAction::Upgrade,
            RuntimeAction::Uninstall,
        ] {
            action_button(ui, s, cx, a, out);
        }
    });
    if !p.settings.is_empty() {
        ui.separator();
        subtitle(ui, &cx.t("ui.plugins.settings"));
        egui::Grid::new("plugin_settings")
            .num_columns(2)
            .show(ui, |ui| {
                for (f, v) in &p.settings {
                    let label = f.description.clone().unwrap_or_else(|| f.key.clone());
                    ui.label(label);
                    setting_widget(ui, f, v, out);
                    ui.end_row();
                }
            });
    }
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
                        egui::ScrollArea::vertical()
                            .id_salt("plugin_details")
                            .show(ui, |ui| details(ui, s, cx, &mut out));
                    });
            },
        );
    });
    out
}
