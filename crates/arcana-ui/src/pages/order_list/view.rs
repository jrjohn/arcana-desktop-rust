use arcana_domain::{OrderStatus, PaymentStatus};
use egui::{Color32, RichText, Ui};

use super::{Input, SortColumn, State};
use crate::fmt;
use crate::widgets::{ViewCx, badge, card, page_title, tool_button};

/// Column layout: (header key, sort column, width).
const COLUMNS: [(&str, SortColumn, f32); 6] = [
    ("order.number", SortColumn::OrderNumber, 150.0),
    ("order.date", SortColumn::OrderDate, 100.0),
    ("order.customer", SortColumn::Customer, 0.0),
    ("order.amount", SortColumn::Amount, 90.0),
    ("order.paymentStatus", SortColumn::PaymentStatus, 110.0),
    ("order.orderStatus", SortColumn::Status, 100.0),
];

fn payment_color(s: PaymentStatus) -> Color32 {
    match s {
        PaymentStatus::Paid | PaymentStatus::PartialPaid => Color32::from_rgb(41, 182, 246),
        PaymentStatus::Unpaid => Color32::from_rgb(3, 155, 229),
        PaymentStatus::Refunded => Color32::GRAY,
    }
}

fn header(ui: &mut Ui, cx: &ViewCx<'_>, state: &State, out: &mut Vec<Input>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            page_title(
                ui,
                &cx.t("order.list"),
                Some(&cx.tw("order.countFormat", &[state.total_count.to_string()])),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if tool_button(ui, "🖶", &cx.t("common.print"), true) {
                out.push(Input::Print);
            }
            if tool_button(ui, "⬇", &cx.t("common.export"), true) {
                out.push(Input::Export);
            }
            ui.separator();
            if tool_button(ui, "⟳", &cx.t("common.refresh"), !state.is_loading) {
                out.push(Input::Refresh);
            }
            if tool_button(ui, "➕", &cx.t("order.new"), true) {
                out.push(Input::CreateOrder);
            }
        });
    });
}

fn filters(ui: &mut Ui, cx: &ViewCx<'_>, state: &State, out: &mut Vec<Input>) {
    card(ui, cx, |ui| {
        ui.horizontal(|ui| {
            let mut search = state.search_text.clone();
            let r = ui.add(
                egui::TextEdit::singleline(&mut search)
                    .hint_text(cx.t("order.searchPlaceholder"))
                    .desired_width(280.0)
                    .id_salt("order_search"),
            );
            if r.changed() {
                out.push(Input::SearchTextChanged(search));
            }
            if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                out.push(Input::Search);
            }
            if ui.button("🔍").clicked() {
                out.push(Input::Search);
            }
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(cx.t("common.clearFilter")).clicked() {
                    out.push(Input::ClearFilter);
                }
                for (value, key, make) in [
                    (
                        &state.to_date,
                        "order.toDate",
                        Input::ToDateChanged as fn(String) -> Input,
                    ),
                    (&state.from_date, "order.fromDate", Input::FromDateChanged),
                ] {
                    let mut d = value.clone();
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut d)
                            .hint_text(format!("{} (yyyy-mm-dd)", cx.t(key)))
                            .desired_width(150.0)
                            .id_salt(key),
                    );
                    if r.changed() {
                        out.push(make(d));
                    }
                    if r.lost_focus() {
                        out.push(Input::Search);
                    }
                }
                let current = state.status_filter.map_or_else(
                    || cx.t("order.statusFilter"),
                    |s| cx.t(fmt::order_status_key(s)),
                );
                egui::ComboBox::from_id_salt("status_filter")
                    .selected_text(current)
                    .show_ui(ui, |ui| {
                        if ui
                            .selectable_label(state.status_filter.is_none(), cx.t("common.all"))
                            .clicked()
                        {
                            out.push(Input::StatusFilterChanged(None));
                        }
                        for s in OrderStatus::ALL {
                            if ui
                                .selectable_label(
                                    state.status_filter == Some(*s),
                                    cx.t(fmt::order_status_key(*s)),
                                )
                                .clicked()
                            {
                                out.push(Input::StatusFilterChanged(Some(*s)));
                            }
                        }
                    });
            });
        });
    });
}

fn table(ui: &mut Ui, cx: &ViewCx<'_>, state: &State, out: &mut Vec<Input>) {
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        egui::Grid::new("orders_grid")
            .num_columns(COLUMNS.len() + 1)
            .striped(true)
            .min_col_width(60.0)
            .spacing([12.0, 10.0])
            .show(ui, |ui| {
                for (key, col, _) in COLUMNS {
                    let arrow = match (state.sort == col, state.descending) {
                        (true, true) => " ▼",
                        (true, false) => " ▲",
                        _ => "",
                    };
                    let label = RichText::new(format!("{}{arrow}", cx.t(key))).strong();
                    if ui.add(egui::Button::new(label).frame(false)).clicked() {
                        out.push(Input::SortBy(col));
                    }
                }
                ui.label(RichText::new(cx.t("common.actions")).strong());
                ui.end_row();
                for o in &state.orders {
                    let id = o.base.id;
                    let selected = state.selected == Some(id);
                    let r = ui.selectable_label(selected, RichText::new(&o.order_number).strong());
                    if r.clicked() {
                        out.push(Input::Select(Some(id)));
                    }
                    if r.double_clicked() {
                        out.push(Input::ViewOrder(id));
                    }
                    ui.label(fmt::date(o.order_date));
                    ui.label(&o.customer_name);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(fmt::number(o.total_amount, 0));
                    });
                    badge(
                        ui,
                        &cx.t(fmt::payment_status_key(o.payment_status)),
                        payment_color(o.payment_status),
                    );
                    let status_badge = ui
                        .add(
                            egui::Button::new(
                                RichText::new(cx.t(fmt::order_status_key(o.status)))
                                    .small()
                                    .color(Color32::WHITE),
                            )
                            .fill(cx.palette.success),
                        )
                        .on_hover_text(cx.t("ui.order.advanceStatus"));
                    if status_badge.clicked() {
                        out.push(Input::ChangeStatus(id));
                    }
                    ui.horizontal(|ui| {
                        if ui.button("✏").on_hover_text(cx.t("common.edit")).clicked() {
                            out.push(Input::EditOrder(id));
                        }
                        if ui
                            .button("🗑")
                            .on_hover_text(cx.t("common.delete"))
                            .clicked()
                        {
                            out.push(Input::DeleteOrder(id));
                        }
                    });
                    ui.end_row();
                }
            });
        if state.orders.is_empty() && !state.is_loading {
            ui.vertical_centered(|ui| ui.label(cx.t("common.noDocuments")));
        }
        if state.is_loading {
            ui.spinner();
        }
    });
}

fn pager(ui: &mut Ui, cx: &ViewCx<'_>, state: &State, out: &mut Vec<Input>) {
    ui.horizontal(|ui| {
        let (a, b) = state.showing_range();
        ui.label(cx.tw(
            "common.showingFormat",
            &[a.to_string(), b.to_string(), state.total_count.to_string()],
        ));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add_enabled(
                    state.can_go_next(),
                    egui::Button::new(cx.t("common.nextPage")),
                )
                .clicked()
            {
                out.push(Input::NextPage);
            }
            ui.label(cx.tw(
                "common.pageFormat",
                &[
                    state.current_page.to_string(),
                    state.total_pages.to_string(),
                ],
            ));
            if ui
                .add_enabled(
                    state.can_go_previous(),
                    egui::Button::new(cx.t("common.prevPage")),
                )
                .clicked()
            {
                out.push(Input::PreviousPage);
            }
        });
    });
}

/// Draw the order list; returns the inputs the user produced.
pub fn show(ui: &mut Ui, state: &State, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        header(ui, cx, state, &mut out);
        ui.add_space(8.0);
        filters(ui, cx, state, &mut out);
        ui.add_space(8.0);
        table(ui, cx, state, &mut out);
        ui.add_space(8.0);
        pager(ui, cx, state, &mut out);
    });
    out
}
