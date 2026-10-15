use egui::{RichText, Ui};

use super::{Input, ListKind, State};
use crate::widgets::{ViewCx, card, page_title, tool_button};

#[allow(clippy::too_many_lines)]
/// Draw an entity list; returns the inputs the user produced.
pub fn show<K: ListKind>(ui: &mut Ui, s: &State<K::Row>, cx: &ViewCx<'_>) -> Vec<Input<K::Row>> {
    let mut out = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt(K::SALT)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                page_title(ui, &cx.t(K::TITLE_KEY), None);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tool_button(ui, "⟳", &cx.t("common.refresh"), !s.is_loading) {
                        out.push(Input::Refresh);
                    }
                    if tool_button(ui, "➕", &cx.t(K::NEW_KEY), true) {
                        out.push(Input::New);
                    }
                });
            });
            ui.add_space(8.0);
            card(ui, cx, |ui| {
                ui.horizontal(|ui| {
                    let mut q = s.search_text.clone();
                    let r = ui.add(
                        egui::TextEdit::singleline(&mut q)
                            .hint_text(cx.t(K::SEARCH_HINT_KEY))
                            .desired_width(300.0)
                            .id_salt((K::SALT, "search")),
                    );
                    if r.changed() {
                        out.push(Input::SearchTextChanged(q));
                    }
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        out.push(Input::Search);
                    }
                    if ui.button("🔍").clicked() {
                        out.push(Input::Search);
                    }
                });
            });
            ui.add_space(8.0);
            card(ui, cx, |ui| {
                ui.set_min_width(ui.available_width());
                egui::Grid::new((K::SALT, "grid"))
                    .num_columns(K::COLUMNS.len() + 1)
                    .striped(true)
                    .spacing([14.0, 8.0])
                    .show(ui, |ui| {
                        for key in K::COLUMNS {
                            ui.label(RichText::new(cx.t(key)).strong());
                        }
                        ui.label(RichText::new(cx.t("common.actions")).strong());
                        ui.end_row();
                        for row in &s.rows {
                            let id = K::id(row);
                            for (i, cell) in K::cells(row).into_iter().enumerate() {
                                if i == 0 {
                                    if ui.link(RichText::new(cell).strong()).clicked() {
                                        out.push(Input::Open(id));
                                    }
                                } else {
                                    ui.label(cell);
                                }
                            }
                            ui.horizontal(|ui| {
                                if ui.button("✏").on_hover_text(cx.t("common.edit")).clicked() {
                                    out.push(Input::Open(id));
                                }
                                if ui
                                    .button("🗑")
                                    .on_hover_text(cx.t("common.delete"))
                                    .clicked()
                                {
                                    out.push(Input::Delete(id));
                                }
                            });
                            ui.end_row();
                        }
                    });
                if s.is_loading {
                    ui.spinner();
                }
            });
            ui.horizontal(|ui| {
                ui.label(cx.tw("ui.totalFormat", &[s.total_count.to_string()]));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_enabled(s.can_go_next(), egui::Button::new(cx.t("common.nextPage")))
                        .clicked()
                    {
                        out.push(Input::NextPage);
                    }
                    ui.label(cx.tw(
                        "common.pageFormat",
                        &[s.current_page.to_string(), s.total_pages.to_string()],
                    ));
                    if ui
                        .add_enabled(
                            s.can_go_previous(),
                            egui::Button::new(cx.t("common.prevPage")),
                        )
                        .clicked()
                    {
                        out.push(Input::PreviousPage);
                    }
                });
            });
        });
    out
}
