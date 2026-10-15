use egui::{RichText, Ui};

use super::{Input, State};
use crate::fmt;
use crate::widgets::{ViewCx, card, page_title, subtitle};

fn quick_action(ui: &mut Ui, cx: &ViewCx<'_>, icon: &str, label: &str) -> bool {
    let size = egui::vec2(150.0, 110.0);
    let r = ui.add_sized(
        size,
        egui::Button::new(RichText::new(format!("{icon}\n\n{label}")).size(15.0))
            .fill(cx.palette.card),
    );
    r.clicked()
}

fn stat(ui: &mut Ui, cx: &ViewCx<'_>, label: &str, value: &str, unit: &str, width: f32) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 120.0),
        egui::Layout::top_down(egui::Align::Min),
        |ui| {
            card(ui, cx, |ui| {
                ui.set_min_size(egui::vec2(width - 34.0, 90.0));
                ui.label(RichText::new(label).small());
                ui.label(RichText::new(value).size(40.0));
                ui.label(RichText::new(unit).small().weak());
            });
        },
    );
}

#[allow(clippy::too_many_lines)]
/// Draw the dashboard; returns the inputs the user produced.
pub fn show(ui: &mut Ui, state: &State, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.add_space(8.0);
        page_title(ui, &cx.t("home.welcome"), None);
        ui.label(RichText::new(cx.t("home.subtitle")).size(20.0));
        ui.add_space(16.0);
        subtitle(ui, &cx.t("home.quickActions"));
        ui.horizontal_wrapped(|ui| {
            for a in &state.quick_actions {
                if quick_action(ui, cx, &a.icon, &a.title) {
                    out.push(Input::RunCommand(a.command.clone()));
                }
            }
            if !state.quick_actions.is_empty() {
                return;
            }
            for (icon, key, input) in [
                ("➕", "home.newOrder", Input::NewOrder),
                ("👤", "home.newCustomer", Input::NewCustomer),
                ("📦", "home.newProduct", Input::NewProduct),
                ("📊", "home.viewReports", Input::ViewReports),
            ] {
                if quick_action(ui, cx, icon, &cx.t(key)) {
                    out.push(input);
                }
            }
        });
        ui.add_space(16.0);
        let d = &state.dashboard;
        let w = ((ui.available_width() - 36.0) / 4.0).max(140.0);
        ui.horizontal(|ui| {
            stat(
                ui,
                cx,
                &cx.t("home.todayOrders"),
                &d.today_orders.to_string(),
                &cx.t("unit.count"),
                w,
            );
            stat(
                ui,
                cx,
                &cx.t("home.todayRevenue"),
                &fmt::money(d.today_revenue),
                &cx.t("unit.currency"),
                w,
            );
            stat(
                ui,
                cx,
                &cx.t("home.pendingOrders"),
                &d.pending_orders.to_string(),
                &cx.t("unit.count"),
                w,
            );
            stat(
                ui,
                cx,
                &cx.t("home.totalCustomers"),
                &d.total_customers.to_string(),
                &cx.t("unit.people"),
                w,
            );
        });
        ui.add_space(16.0);
        ui.horizontal(|ui| {
            subtitle(ui, &cx.t("home.recentOrders"));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.link(cx.t("home.viewAll")).clicked() {
                    out.push(Input::ViewAllOrders);
                }
            });
        });
        card(ui, cx, |ui| {
            ui.set_min_width(ui.available_width());
            if d.recent.is_empty() {
                ui.vertical_centered(|ui| {
                    ui.label(RichText::new("🗄").size(36.0));
                    ui.label(cx.t("home.noRecentOrders"));
                });
            }
            egui::Grid::new("recent_orders")
                .num_columns(4)
                .striped(true)
                .show(ui, |ui| {
                    for o in &d.recent {
                        if ui.link(RichText::new(&o.order_number).strong()).clicked() {
                            out.push(Input::OpenOrder(o.base.id));
                        }
                        ui.label(&o.customer_name);
                        ui.label(fmt::number(o.total_amount, 0));
                        ui.label(cx.t(fmt::order_status_key(o.status)));
                        ui.end_row();
                    }
                });
        });
        ui.add_space(16.0);
        card(ui, cx, |ui| {
            ui.set_min_width(ui.available_width());
            subtitle(ui, &cx.t("home.systemInfo"));
            egui::Grid::new("sysinfo").num_columns(2).show(ui, |ui| {
                ui.label(cx.t("home.version"));
                ui.label(env!("CARGO_PKG_VERSION"));
                ui.end_row();
                ui.label(cx.t("home.database"));
                ui.label(cx.t("home.connected"));
                ui.end_row();
                ui.label(cx.t("home.lastSync"));
                ui.label(cx.t("home.never"));
                ui.end_row();
            });
        });
        if let Some(e) = &state.error {
            ui.colored_label(ui.visuals().error_fg_color, e);
        }
    });
    out
}
