use egui::{Color32, CornerRadius, RichText, Sense, Stroke, Ui, vec2};

use super::{FREQUENCIES, Input, LANGUAGES, State};
use crate::theme::{ACCENTS, THEMES, ThemeDef};
use crate::widgets::{ViewCx, card, page_title, subtitle};

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

/// C# theme tile: pane strip + accent bar + name.
fn theme_tile(ui: &mut Ui, cx: &ViewCx<'_>, t: &ThemeDef, selected: bool) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(120.0, 80.0), Sense::click());
    let name = cx.t(t.name_key);
    r.widget_info(|| egui::WidgetInfo::selected(egui::WidgetType::Button, true, selected, &name));
    let p = ui.painter();
    let radius = CornerRadius::same(8);
    p.rect_filled(rect, radius, rgb(t.background));
    let pane = egui::Rect::from_min_size(rect.min, vec2(30.0, rect.height() - 20.0));
    p.rect_filled(pane, CornerRadius::same(6), rgb(t.pane));
    let bar = egui::Rect::from_min_size(rect.min + vec2(36.0, 8.0), vec2(40.0, 4.0));
    p.rect_filled(bar, CornerRadius::same(2), rgb(t.accent));
    let line = egui::Rect::from_min_size(rect.min + vec2(36.0, 16.0), vec2(30.0, 3.0));
    p.rect_filled(
        line,
        CornerRadius::same(1),
        rgb(t.text_primary).gamma_multiply(0.5),
    );
    let name_rect =
        egui::Rect::from_min_max(rect.left_bottom() - vec2(0.0, 20.0), rect.right_bottom());
    p.rect_filled(name_rect, CornerRadius::same(6), cx.palette.card);
    p.text(
        name_rect.center(),
        egui::Align2::CENTER_CENTER,
        cx.t(t.name_key),
        egui::FontId::proportional(11.0),
        cx.palette.text,
    );
    let stroke = if selected {
        Stroke::new(3.0, cx.palette.accent)
    } else {
        Stroke::new(1.0, cx.palette.text_weak.gamma_multiply(0.4))
    };
    p.rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
    r.on_hover_text(cx.t(t.name_key)).clicked()
}

fn appearance(ui: &mut Ui, s: &State, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        subtitle(ui, &cx.t("settings.appearance"));
        ui.label(RichText::new(cx.t("settings.theme")).strong());
        ui.horizontal_wrapped(|ui| {
            for t in &THEMES {
                if theme_tile(ui, cx, t, s.theme_id == t.id) {
                    out.push(Input::SelectTheme(t.id.to_owned()));
                }
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new(cx.t("ui.settings.accent")).strong());
        ui.horizontal(|ui| {
            if ui
                .selectable_label(s.accent.is_none(), cx.t("ui.settings.accentDefault"))
                .clicked()
            {
                out.push(Input::SelectAccent(None));
            }
            for (i, c) in ACCENTS.iter().enumerate() {
                let (rect, r) = ui.allocate_exact_size(vec2(24.0, 24.0), Sense::click());
                r.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::Button,
                        true,
                        s.accent == Some(i),
                        format!("accent-{i}"),
                    )
                });
                ui.painter().circle_filled(rect.center(), 11.0, rgb(*c));
                if s.accent == Some(i) {
                    ui.painter().circle_stroke(
                        rect.center(),
                        12.0,
                        Stroke::new(2.0, cx.palette.text),
                    );
                }
                if r.clicked() {
                    out.push(Input::SelectAccent(Some(i)));
                }
            }
        });
        ui.add_space(8.0);
        ui.label(RichText::new(cx.t("settings.language")).strong());
        let current = LANGUAGES
            .iter()
            .find(|(c, _)| *c == s.culture)
            .map_or("", |(_, n)| n);
        egui::ComboBox::from_id_salt("language")
            .width(200.0)
            .selected_text(current)
            .show_ui(ui, |ui| {
                for (code, name) in LANGUAGES {
                    if ui.selectable_label(s.culture == code, name).clicked() {
                        out.push(Input::SelectLanguage(code.to_owned()));
                    }
                }
            });
    });
}

fn sync(ui: &mut Ui, s: &State, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    card(ui, cx, |ui| {
        ui.set_min_width(ui.available_width());
        subtitle(ui, &cx.t("settings.syncSettings"));
        let mut on = s.auto_sync;
        let state_text = cx.t(if on { "settings.on" } else { "settings.off" });
        if ui
            .checkbox(
                &mut on,
                format!("{} ({state_text})", cx.t("settings.autoSync")),
            )
            .changed()
        {
            out.push(Input::AutoSyncToggled(on));
        }
        ui.label(cx.t("settings.syncFrequency"));
        let current = FREQUENCIES
            .iter()
            .find(|(m, _)| *m == s.sync_minutes)
            .map_or(FREQUENCIES[0].1, |(_, k)| k);
        egui::ComboBox::from_id_salt("sync_freq")
            .width(200.0)
            .selected_text(cx.t(current))
            .show_ui(ui, |ui| {
                for (m, key) in FREQUENCIES {
                    if ui
                        .selectable_label(s.sync_minutes == m, cx.t(key))
                        .clicked()
                    {
                        out.push(Input::FrequencyChanged(m));
                    }
                }
            });
        if ui.button(cx.t("settings.syncNow")).clicked() {
            out.push(Input::SyncNow);
        }
    });
}

/// Draw settings; returns the inputs the user produced.
pub fn show(ui: &mut Ui, s: &State, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out = Vec::new();
    egui::ScrollArea::vertical().show(ui, |ui| {
        ui.set_max_width(800.0);
        page_title(ui, &cx.t("settings.title"), None);
        ui.add_space(12.0);
        appearance(ui, s, cx, &mut out);
        ui.add_space(12.0);
        sync(ui, s, cx, &mut out);
        ui.add_space(12.0);
        card(ui, cx, |ui| {
            ui.set_min_width(ui.available_width());
            subtitle(ui, &cx.t("settings.dataSettings"));
            for (key, input) in [
                ("settings.export", Input::ExportData),
                ("settings.import", Input::ImportData),
                ("settings.clearCache", Input::ClearCache),
            ] {
                if ui.button(cx.t(key)).clicked() {
                    out.push(input);
                }
            }
        });
        ui.add_space(12.0);
        card(ui, cx, |ui| {
            ui.set_min_width(ui.available_width());
            subtitle(ui, &cx.t("settings.about"));
            ui.label(cx.t("settings.appName"));
            ui.label(
                RichText::new(cx.tw(
                    "settings.versionFormat",
                    &[env!("CARGO_PKG_VERSION").to_owned()],
                ))
                .weak(),
            );
            ui.label(RichText::new("© 2024 Arcana Team").weak());
        });
    });
    out
}
