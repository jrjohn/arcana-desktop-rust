//! The `egui_dock` viewer for document tabs (the [`Tab`] model lives in the view model).

use egui::{Id, Ui, WidgetText};
use egui_dock::TabViewer;
use egui_dock::tab_viewer::OnCloseResponse;

use super::{Tab, TabId};
use crate::pages::PageMsg;
use crate::widgets::ViewCx;

/// What the viewer collected while drawing.
#[derive(Debug, Default)]
pub struct ViewerOutput {
    pub messages: Vec<(TabId, PageMsg)>,
    pub close: Vec<TabId>,
    pub pop_out: Vec<TabId>,
    pub back: Vec<TabId>,
}

/// `egui_dock` adapter.
pub struct Viewer<'a> {
    pub tabs: &'a mut std::collections::BTreeMap<TabId, Tab>,
    pub cx: ViewCx<'a>,
    pub out: ViewerOutput,
}

/// Draw one tab's content (also used by floating windows).
pub fn tab_ui(ui: &mut Ui, tab: &mut Tab, cx: &ViewCx<'_>, out: &mut ViewerOutput) {
    if tab.can_go_back() && ui.button(format!("← {}", cx.t("ui.nav.back"))).clicked() {
        out.back.push(tab.id);
    }
    for m in tab.page.show(ui, cx) {
        out.messages.push((tab.id, m));
    }
}

impl TabViewer for Viewer<'_> {
    type Tab = TabId;

    fn id(&mut self, tab: &mut TabId) -> Id {
        Id::new(("doc", tab.0))
    }

    fn title(&mut self, tab: &mut TabId) -> WidgetText {
        self.tabs
            .get(tab)
            .map_or_else(String::new, |t| {
                let dirty = if t.page.is_dirty() { " •" } else { "" };
                format!("{}{dirty}", t.page.title(&self.cx))
            })
            .into()
    }

    fn ui(&mut self, ui: &mut Ui, tab: &mut TabId) {
        if let Some(t) = self.tabs.get_mut(tab) {
            egui::Frame::new().inner_margin(16).show(ui, |ui| {
                tab_ui(ui, t, &self.cx, &mut self.out);
            });
        }
    }

    fn context_menu(&mut self, ui: &mut Ui, tab: &mut TabId, _path: egui_dock::NodePath) {
        if ui
            .button(format!("↗ {}", self.cx.t("common.popOut")))
            .clicked()
        {
            self.out.pop_out.push(*tab);
            ui.close();
        }
        if ui.button(self.cx.t("common.close")).clicked() {
            self.out.close.push(*tab);
            ui.close();
        }
    }

    fn on_close(&mut self, tab: &mut TabId) -> OnCloseResponse {
        // The shell decides (unsaved changes ask first).
        self.out.close.push(*tab);
        OnCloseResponse::Ignore
    }
}
