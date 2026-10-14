use arcana_domain::PaymentMethod;
use egui::{RichText, Ui};

use super::{Input, State};
use crate::fmt;
use crate::widgets::{ViewCx, card, field, messages, page_title, subtitle, tool_button};

fn toolbar(ui: &mut Ui, cx: &ViewCx<'_>, s: &State, out: &mut Vec<Input>) {
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            let title = if s.is_new {
                cx.t("order.new")
            } else {
                format!("{} {}", cx.t("order.title"), s.order.order_number)
            };
            let mut caption = cx.t(fmt::order_status_key(s.order.status));
            if let Some(from) = &s.copied_from {
                caption = format!("{caption} · {} {from}", cx.t("order.copiedFrom"));
            }
            page_title(ui, &title, Some(&caption));
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if tool_button(ui, "🖶", &cx.t("order.print"), true) {
                out.push(Input::Print);
            }
            ui.separator();
            if tool_button(ui, "✖", &cx.t("order.cancel"), s.is_editing) {
                out.push(Input::Cancel);
            }
            if tool_button(ui, "💾", &cx.t("order.save"), s.can_save()) {
                out.push(Input::Save);
            }
            if tool_button(ui, "✏", &cx.t("order.edit"), s.can_edit()) {
                out.push(Input::Edit);
            }
            if tool_button(ui, "📋", &cx.t("order.copy"), !s.is_new) {
                out.push(Input::Copy);
            }
        });
    });
}

fn info(ui: &mut Ui, cx: &ViewCx<'_>, s: &State, out: &mut Vec<Input>) {
    let edit = s.is_editing;
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        subtitle(ui, &cx.t("order.info"));
        egui::Grid::new("order_info")
            .num_columns(2)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                ui.vertical(|ui| {
                    let _ = field(ui, &cx.t("order.number"), &s.order.order_number, false);
                });
                ui.vertical(|ui| {
                    if let Some(v) = field(ui, &cx.t("order.date"), &s.order_date_text, edit) {
                        out.push(Input::OrderDateChanged(v));
                    }
                });
                ui.end_row();
                ui.vertical(|ui| {
                    ui.label(RichText::new(cx.t("order.customer")).small());
                    let current = s
                        .selected_customer()
                        .map_or_else(|| cx.t("order.selectCustomer"), |c| c.name.clone());
                    ui.add_enabled_ui(edit, |ui| {
                        egui::ComboBox::from_id_salt("customer")
                            .width(240.0)
                            .selected_text(current)
                            .show_ui(ui, |ui| {
                                for c in &s.customers {
                                    if ui
                                        .selectable_label(c.base.id == s.order.customer_id, &c.name)
                                        .clicked()
                                    {
                                        out.push(Input::SelectCustomer(c.base.id));
                                    }
                                }
                            });
                    });
                });
                ui.vertical(|ui| {
                    ui.label(RichText::new(cx.t("order.paymentMethod")).small());
                    ui.add_enabled_ui(edit, |ui| {
                        egui::ComboBox::from_id_salt("payment_method")
                            .width(240.0)
                            .selected_text(cx.t(fmt::payment_method_key(s.order.payment_method)))
                            .show_ui(ui, |ui| {
                                for m in PaymentMethod::ALL {
                                    if ui
                                        .selectable_label(
                                            s.order.payment_method == *m,
                                            cx.t(fmt::payment_method_key(*m)),
                                        )
                                        .clicked()
                                    {
                                        out.push(Input::PaymentMethodChanged(*m));
                                    }
                                }
                            });
                    });
                });
                ui.end_row();
            });
    });
}

fn product_search(ui: &mut Ui, cx: &ViewCx<'_>, s: &State, out: &mut Vec<Input>) {
    ui.horizontal(|ui| {
        subtitle(ui, &cx.t("order.items"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(s.is_editing, |ui| {
                if ui.button(cx.t("order.clearItems")).clicked() {
                    out.push(Input::ClearItems);
                }
                if ui.button("🔍").clicked() {
                    out.push(Input::SearchProducts);
                }
                let mut q = s.product_search.clone();
                let r = ui.add(
                    egui::TextEdit::singleline(&mut q)
                        .hint_text(cx.t("order.searchProduct"))
                        .desired_width(180.0)
                        .id_salt("product_search"),
                );
                if r.changed() {
                    out.push(Input::ProductSearchChanged(q));
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    out.push(Input::SearchProducts);
                }
            });
        });
    });
    if s.is_editing && !s.products.is_empty() {
        ui.horizontal_wrapped(|ui| {
            for p in &s.products {
                let label = format!("➕ {} {} ({})", p.code, p.name, fmt::number(p.price, 0));
                if ui.button(label).clicked() {
                    out.push(Input::AddItem(p.base.id));
                }
            }
        });
    }
}

fn items(ui: &mut Ui, cx: &ViewCx<'_>, s: &State, out: &mut Vec<Input>) {
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        product_search(ui, cx, s, out);
        egui::Grid::new("order_items")
            .num_columns(8)
            .striped(true)
            .spacing([10.0, 8.0])
            .show(ui, |ui| {
                for key in [
                    "#",
                    "order.productCode",
                    "order.productName",
                    "order.quantity",
                    "order.unitPrice",
                    "order.discount",
                    "order.subtotal",
                    "",
                ] {
                    let text = if key.contains('.') {
                        cx.t(key)
                    } else {
                        key.to_owned()
                    };
                    ui.label(RichText::new(text).strong());
                }
                ui.end_row();
                for (i, item) in s.order.items.iter().enumerate() {
                    ui.label(item.line_number.to_string());
                    ui.label(&item.product_code);
                    ui.label(&item.product_name);
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(s.is_editing, |ui| {
                            if ui.small_button("-").clicked() {
                                out.push(Input::DecreaseQuantity(i));
                            }
                            ui.label(fmt::number(item.quantity, 1));
                            if ui.small_button("+").clicked() {
                                out.push(Input::IncreaseQuantity(i));
                            }
                        });
                    });
                    ui.label(fmt::number(item.unit_price, 0));
                    let mut d = s.item_discount_text.get(i).cloned().unwrap_or_default();
                    let r = ui.add_enabled(
                        s.is_editing,
                        egui::TextEdit::singleline(&mut d)
                            .desired_width(56.0)
                            .id_salt(("disc", i)),
                    );
                    if r.changed() {
                        out.push(Input::ItemDiscountChanged(i, d));
                    }
                    ui.label(RichText::new(fmt::number(item.line_total(), 0)).strong());
                    if ui
                        .add_enabled(s.is_editing, egui::Button::new("🗑"))
                        .clicked()
                    {
                        out.push(Input::RemoveItem(i));
                    }
                    ui.end_row();
                }
            });
    });
}

fn shipping(ui: &mut Ui, cx: &ViewCx<'_>, s: &State, out: &mut Vec<Input>) {
    let edit = s.is_editing;
    let o = &s.order;
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        subtitle(ui, &cx.t("order.shippingInfo"));
        if let Some(v) = field(
            ui,
            &cx.t("order.shippingAddress"),
            o.shipping_address.as_deref().unwrap_or(""),
            edit,
        ) {
            out.push(Input::ShippingAddressChanged(v));
        }
        ui.columns(2, |cols| {
            if let Some(v) = field(
                &mut cols[0],
                &cx.t("order.shippingCity"),
                o.shipping_city.as_deref().unwrap_or(""),
                edit,
            ) {
                out.push(Input::ShippingCityChanged(v));
            }
            if let Some(v) = field(
                &mut cols[1],
                &cx.t("order.shippingPostalCode"),
                o.shipping_postal_code.as_deref().unwrap_or(""),
                edit,
            ) {
                out.push(Input::ShippingPostalCodeChanged(v));
            }
        });
        if let Some(v) = field(
            ui,
            &cx.t("order.expectedDelivery"),
            &s.expected_delivery_text,
            edit,
        ) {
            out.push(Input::ExpectedDeliveryChanged(v));
        }
        ui.add_space(8.0);
        subtitle(ui, &cx.t("order.notes"));
        let mut notes = o.notes.clone().unwrap_or_default();
        let r = ui.add_enabled(
            edit,
            egui::TextEdit::multiline(&mut notes)
                .desired_rows(4)
                .desired_width(f32::INFINITY)
                .id_salt("notes"),
        );
        if r.changed() {
            out.push(Input::NotesChanged(notes));
        }
    });
}

fn summary(ui: &mut Ui, cx: &ViewCx<'_>, s: &State, out: &mut Vec<Input>) {
    let o = &s.order;
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        subtitle(ui, &cx.t("order.summary"));
        egui::Grid::new("summary")
            .num_columns(2)
            .spacing([16.0, 10.0])
            .show(ui, |ui| {
                ui.label(cx.t("order.subtotal"));
                ui.label(fmt::money(o.subtotal));
                ui.end_row();
                ui.label(cx.t("order.tax"));
                ui.label(fmt::money(o.tax_amount));
                ui.end_row();
                for (key, text, make) in [
                    (
                        "order.shippingFee",
                        &s.shipping_cost_text,
                        Input::ShippingCostChanged as fn(String) -> Input,
                    ),
                    (
                        "ui.order.discountAmount",
                        &s.discount_text,
                        Input::DiscountChanged,
                    ),
                ] {
                    ui.label(cx.t(key));
                    let mut v = text.clone();
                    let r = ui.add_enabled(
                        s.is_editing,
                        egui::TextEdit::singleline(&mut v)
                            .desired_width(110.0)
                            .id_salt(key),
                    );
                    if r.changed() {
                        out.push(make(v));
                    }
                    ui.end_row();
                }
            });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label(RichText::new(cx.t("order.total")).size(20.0).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    RichText::new(fmt::money(o.total_amount))
                        .size(20.0)
                        .strong(),
                );
            });
        });
        if ui
            .add_sized(
                [ui.available_width(), 28.0],
                egui::Button::new(cx.t("order.calculate")),
            )
            .clicked()
        {
            out.push(Input::CalculateTotals);
        }
    });
}

/// Draw the order form; returns the inputs the user produced.
pub fn show(ui: &mut Ui, state: &State, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        toolbar(ui, cx, state, &mut out);
        if state.is_loading {
            ui.spinner();
        }
        messages(ui, &state.validation);
        if let Some(e) = &state.error {
            messages(ui, std::slice::from_ref(e));
        }
        ui.add_space(8.0);
        let total = ui.available_width();
        let right = (total * 0.32).clamp(260.0, 380.0);
        ui.horizontal_top(|ui| {
            ui.allocate_ui_with_layout(
                egui::vec2(total - right - 12.0, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.vertical(|ui| {
                        info(ui, cx, state, &mut out);
                        ui.add_space(8.0);
                        items(ui, cx, state, &mut out);
                        ui.add_space(8.0);
                        shipping(ui, cx, state, &mut out);
                    });
                },
            );
            ui.allocate_ui_with_layout(
                egui::vec2(right, 0.0),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    summary(ui, cx, state, &mut out);
                },
            );
        });
    });
    out
}
