use egui::{RichText, Ui};

use super::{FieldKind, FormKind, Input, State};
use crate::widgets::{ViewCx, card, messages, page_title, tool_button};

/// Draw an entity form; returns the inputs the user produced.
pub fn show<K: FormKind>(
    ui: &mut Ui,
    s: &State<K::Entity>,
    cx: &ViewCx<'_>,
) -> Vec<Input<K::Entity>> {
    let mut out = Vec::new();
    egui::ScrollArea::vertical()
        .id_salt(K::SALT)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                let title = if s.is_new {
                    cx.t(K::NEW_KEY)
                } else {
                    format!("{} {}", cx.t(K::TITLE_KEY), K::label(&s.entity))
                };
                page_title(ui, &title, None);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if tool_button(ui, "🗑", &cx.t("common.delete"), !s.is_new && !s.is_busy) {
                        out.push(Input::Delete);
                    }
                    if tool_button(ui, "✖", &cx.t("common.cancel"), true) {
                        out.push(Input::Cancel);
                    }
                    if tool_button(ui, "💾", &cx.t("common.save"), !s.is_busy) {
                        out.push(Input::Save);
                    }
                });
            });
            messages(ui, &s.validation);
            if let Some(e) = &s.error {
                messages(ui, std::slice::from_ref(e));
            }
            ui.add_space(8.0);
            card(ui, cx, |ui| {
                ui.set_max_width(720.0);
                egui::Grid::new((K::SALT, "form"))
                    .num_columns(2)
                    .spacing([16.0, 8.0])
                    .show(ui, |ui| {
                        for (i, def) in K::FIELDS.iter().enumerate() {
                            ui.label(RichText::new(cx.t(def.label_key)));
                            let text = &s.texts[i];
                            match def.kind {
                                FieldKind::Bool => {
                                    let mut v = text == "true";
                                    if ui.checkbox(&mut v, "").changed() {
                                        out.push(Input::FieldChanged(i, v.to_string()));
                                    }
                                }
                                FieldKind::Choice => {
                                    let current = s
                                        .options
                                        .iter()
                                        .find(|(id, _)| id.to_string() == *text)
                                        .map_or("—".to_owned(), |(_, n)| n.clone());
                                    egui::ComboBox::from_id_salt((K::SALT, i))
                                        .selected_text(current)
                                        .width(300.0)
                                        .show_ui(ui, |ui| {
                                            if ui.selectable_label(text.is_empty(), "—").clicked()
                                            {
                                                out.push(Input::FieldChanged(i, String::new()));
                                            }
                                            for (id, name) in &s.options {
                                                if ui
                                                    .selectable_label(id.to_string() == *text, name)
                                                    .clicked()
                                                {
                                                    out.push(Input::FieldChanged(
                                                        i,
                                                        id.to_string(),
                                                    ));
                                                }
                                            }
                                        });
                                }
                                kind => {
                                    let mut v = text.clone();
                                    let edit = if kind == FieldKind::Multiline {
                                        egui::TextEdit::multiline(&mut v).desired_rows(3)
                                    } else {
                                        egui::TextEdit::singleline(&mut v)
                                    };
                                    let r = ui.add_enabled(
                                        kind != FieldKind::ReadOnly,
                                        edit.desired_width(300.0).id_salt((K::SALT, i)),
                                    );
                                    if r.changed() {
                                        out.push(Input::FieldChanged(i, v));
                                    }
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
        });
    out
}
