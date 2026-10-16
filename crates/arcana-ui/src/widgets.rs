//! Shared drawing helpers and the per-frame view context.

use egui::{Color32, CornerRadius, Frame, Margin, RichText, Stroke, Ui};

use crate::l10n;
use crate::theme::Palette;

/// What every view gets: strings and colours.
#[derive(Clone, Copy)]
pub struct ViewCx<'a> {
    pub l10n: l10n::Strings<'a>,
    pub palette: Palette,
}

impl ViewCx<'_> {
    /// Localized string.
    #[must_use]
    pub fn t(&self, key: &str) -> String {
        self.l10n.t(key)
    }

    /// Localized string with `{0}`… arguments.
    #[must_use]
    pub fn tw(&self, key: &str, args: &[String]) -> String {
        self.l10n.tw(key, args)
    }
}

/// Big page title + optional caption (C# `TitleTextBlockStyle` + caption).
pub fn page_title(ui: &mut Ui, title: &str, caption: Option<&str>) {
    ui.label(RichText::new(title).size(28.0).strong());
    if let Some(c) = caption {
        ui.label(RichText::new(c).small().weak());
    }
}

/// Section heading inside a card (C# `SubtitleTextBlockStyle`).
pub fn subtitle(ui: &mut Ui, text: &str) {
    ui.label(RichText::new(text).size(19.0).strong());
}

/// Rounded card (C# `CardStyle`).
pub fn card<R>(ui: &mut Ui, cx: &ViewCx<'_>, add: impl FnOnce(&mut Ui) -> R) -> R {
    Frame::new()
        .fill(cx.palette.card)
        .stroke(Stroke::new(1.0, cx.palette.text_weak.gamma_multiply(0.25)))
        .corner_radius(CornerRadius::same(8))
        .inner_margin(Margin::same(16))
        .show(ui, add)
        .inner
}

/// Coloured pill (order / payment status badges).
pub fn badge(ui: &mut Ui, text: &str, fill: Color32) {
    Frame::new()
        .fill(fill)
        .corner_radius(CornerRadius::same(4))
        .inner_margin(Margin::symmetric(8, 2))
        .show(ui, |ui| {
            ui.label(RichText::new(text).small().color(Color32::WHITE));
        });
}

/// Toolbar button with icon; returns whether it was clicked.
pub fn tool_button(ui: &mut Ui, icon: &str, label: &str, enabled: bool) -> bool {
    ui.add_enabled(
        enabled,
        egui::Button::new(format!("{icon} {label}")).frame(false),
    )
    .clicked()
}

/// Labelled single-line text field; returns the new text when changed.
pub fn field(ui: &mut Ui, label: &str, value: &str, enabled: bool) -> Option<String> {
    ui.label(RichText::new(label).small());
    let mut buf = value.to_owned();
    let r = ui.add_enabled(
        enabled,
        egui::TextEdit::singleline(&mut buf)
            .desired_width(f32::INFINITY)
            .id_salt(label),
    );
    (r.changed() && buf != value).then_some(buf)
}

/// Inline validation / error list.
pub fn messages(ui: &mut Ui, list: &[String]) {
    for m in list {
        ui.colored_label(ui.visuals().error_fg_color, format!("⚠ {m}"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme;
    use arcana_infra::{LocalizationService as _, LocalizationServiceImpl};

    #[test]
    fn helpers_draw_without_panicking() {
        let l = LocalizationServiceImpl::new();
        l.set_culture("en-US");
        let cx = ViewCx {
            l10n: l10n::Strings::new(&l),
            palette: theme::palette(theme::find("Light"), None, false),
        };
        assert_eq!(cx.t("nav.home"), "Home");
        assert_eq!(cx.tw("order.countFormat", &["3".into()]), "Total 3 orders");
        egui::__run_test_ui(|ui| {
            page_title(ui, "T", Some("c"));
            subtitle(ui, "S");
            card(ui, &cx, |ui| badge(ui, "b", Color32::RED));
            assert!(!tool_button(ui, "+", "x", false));
            assert!(field(ui, "L", "v", true).is_none());
            messages(ui, &["m".into()]);
        });
    }
}
