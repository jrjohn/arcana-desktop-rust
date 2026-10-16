//! `FlowChart` tab view: the plugin's editor canvas plus the path-entry prompt.

use super::{Input, PromptKind, ViewModel};
use crate::widgets::ViewCx;

/// Draw the editor (mutating its VM directly, as the canvas needs) plus the path prompt.
pub fn show(ui: &mut egui::Ui, vm: &mut ViewModel, cx: &ViewCx<'_>) -> Vec<Input> {
    let mut out: Vec<Input> = arcana_plugin_flowchart::ui::show_editor(ui, &mut vm.editor)
        .into_iter()
        .map(Input::Editor)
        .collect();
    if let Some((kind, text)) = &vm.prompt {
        path_prompt(ui, *kind, text, cx, &mut out);
    }
    out
}

/// Modal asking for a file path (open / save as).
fn path_prompt(ui: &egui::Ui, kind: PromptKind, text: &str, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    let mut t = text.to_string();
    egui::Modal::new(egui::Id::new("flowchart_path_prompt")).show(ui.ctx(), |ui| {
        ui.set_width(420.0);
        ui.heading(match kind {
            PromptKind::Open => cx.t("flowchart.open"),
            PromptKind::SaveAs => cx.t("menu.file.save"),
        });
        if ui
            .add(egui::TextEdit::singleline(&mut t).desired_width(f32::INFINITY))
            .changed()
        {
            out.push(Input::PromptText(t.clone()));
        }
        ui.horizontal(|ui| prompt_buttons(ui, cx, out));
    });
}

fn prompt_buttons(ui: &mut egui::Ui, cx: &ViewCx<'_>, out: &mut Vec<Input>) {
    if ui.button(cx.t("common.ok")).clicked() {
        out.push(Input::PromptOk);
    }
    if ui.button(cx.t("common.cancel")).clicked() {
        out.push(Input::PromptCancel);
    }
}
