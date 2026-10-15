//! Drawing the shell: menu bar + command search, side navigation, dock, floating windows,
//! status bar, dialogs and toasts (C# `MainWindow.xaml`).

use std::time::Instant;

use egui::{Align, Color32, Layout, RichText, Ui, ViewportBuilder, ViewportCommand, ViewportId};
use egui_dock::{DockArea, Style};

use super::tabs::{Viewer, ViewerOutput, tab_ui};
use super::{Dialog, Shell};
use crate::commands::{CommandId, CommandRegistry};
use crate::effect::NoticeLevel;
use crate::l10n;
use crate::nav::routes;
use crate::theme;
use crate::widgets::ViewCx;

/// Side navigation: (section id, icon, label key, command); `None` rows are headers.
const NAV_TOP: &[(&str, &str, &str, Option<CommandId>)] = &[
    (routes::HOME, "🏠", "nav.home", Some(CommandId::Home)),
    ("", "", "nav.header.business", None),
    (
        routes::CUSTOMER_LIST,
        "👥",
        "nav.customers",
        Some(CommandId::CustomerList),
    ),
    (
        routes::PRODUCT_LIST,
        "📦",
        "nav.products",
        Some(CommandId::ProductList),
    ),
    (
        routes::ORDER_LIST,
        "📄",
        "nav.orders",
        Some(CommandId::OrderList),
    ),
    ("", "", "nav.header.reports", None),
    // C# `SalesReportPage` has no page behind it.
    ("SalesReportPage", "📚", "nav.reports", None),
];

const NAV_BOTTOM: &[(&str, &str, &str, Option<CommandId>)] = &[
    (
        routes::PLUGIN_MANAGER,
        "🔌",
        "nav.plugins",
        Some(CommandId::PluginManager),
    ),
    // C# `SyncPage` has no page behind it and there is no sync server (PORTING_PLAN §4).
    ("SyncPage", "🔄", "nav.sync", None),
    (
        routes::SETTINGS,
        "⚙",
        "nav.settings",
        Some(CommandId::Settings),
    ),
];

impl Shell {
    fn palette(&self, ctx: &egui::Context) -> theme::Palette {
        theme::palette(
            theme::find(&self.theme_id),
            self.accent,
            ctx.theme() == egui::Theme::Dark,
        )
    }

    /// Draw one frame of the whole application.
    pub fn show(&mut self, ui: &mut Ui) {
        let ctx = ui.ctx().clone();
        self.exec.set_repaint_context(&ctx);
        if self.theme_dirty {
            theme::apply(&ctx, theme::find(&self.theme_id), self.accent);
            self.theme_dirty = false;
        }
        self.pump();
        self.pump_plugins();
        for cmd in CommandRegistry::take_shortcuts(&ctx) {
            self.execute(cmd);
        }
        if self.exit_requested {
            ctx.send_viewport_cmd(ViewportCommand::Close);
            self.exit_requested = false;
        }
        ctx.send_viewport_cmd(ViewportCommand::Fullscreen(self.fullscreen));
        let now = Instant::now();
        self.toasts.retain(|t| t.until > now);
        if !self.toasts.is_empty() || self.exec.in_flight() > 0 {
            ctx.request_repaint_after(std::time::Duration::from_millis(250));
        }

        let palette = self.palette(&ctx);
        let l10n = self.svc.localization.clone();
        let cx = ViewCx {
            l10n: l10n.as_ref(),
            palette,
        };

        egui::Panel::top("menu_bar").show(ui, |ui| self.menu_bar(ui, &cx));
        if self.statusbar_visible {
            egui::Panel::bottom("status_bar").show(ui, |ui| self.status_bar(ui, &cx));
        }
        egui::Panel::left("side_nav")
            .resizable(false)
            .exact_size(if self.sidebar_open { 240.0 } else { 48.0 })
            .frame(egui::Frame::new().fill(palette.pane).inner_margin(8))
            .show(ui, |ui| self.side_nav(ui, &cx));
        let mut out = ViewerOutput::default();
        egui::CentralPanel::default().show(ui, |ui| {
            let mut viewer = Viewer {
                tabs: &mut self.tabs,
                cx,
                out: ViewerOutput::default(),
            };
            DockArea::new(&mut self.dock)
                .style(Style::from_egui(ui.style().as_ref()))
                .show_leaf_collapse_buttons(false)
                .show_leaf_close_all_buttons(false)
                .show_inside(ui, &mut viewer);
            out = viewer.out;
            if self.tabs.is_empty() {
                ui.centered_and_justified(|ui| ui.label(cx.t("common.noDocuments")));
            }
        });
        self.floating_windows(&ctx, &cx, &mut out);
        self.dialogs(&ctx, &cx);
        self.toasts_overlay(&ctx, &cx);
        self.apply_viewer(out);
    }

    fn apply_viewer(&mut self, out: ViewerOutput) {
        for (tab, msg) in out.messages {
            self.send(tab, msg);
        }
        for tab in out.back {
            self.back(tab);
        }
        for tab in out.pop_out {
            self.pop_out(tab);
        }
        for tab in out.close {
            self.close_tab(tab, false);
        }
    }

    fn menu_node(&self, ui: &mut Ui, node: &super::plugins::MenuNode, run: &mut Vec<String>) {
        for child in &node.children {
            let m = &child.item;
            if m.is_separator {
                ui.separator();
            } else if !child.children.is_empty() {
                ui.menu_button(&m.title, |ui| self.menu_node(ui, child, run));
            } else {
                let mut b = egui::Button::new(&m.title);
                if let Some(s) = &m.shortcut {
                    b = b.shortcut_text(s);
                }
                let enabled = m
                    .command
                    .as_deref()
                    .is_some_and(|c| self.plugin_command_enabled(c));
                if ui.add_enabled(enabled, b).clicked() {
                    if let Some(c) = &m.command {
                        run.push(c.clone());
                    }
                    ui.close();
                }
            }
        }
    }

    /// Menu bar built from the plugin `MenuRegistry` (C# `MainWindow.BuildMenus`).
    fn menu_bar(&mut self, ui: &mut Ui, cx: &ViewCx<'_>) {
        let mut run_named = Vec::new();
        let mut run = Vec::new();
        let tree = self.plugin_menu();
        egui::MenuBar::new().ui(ui, |ui| {
            ui.label(RichText::new(cx.t("app.name")).strong());
            for node in &tree {
                ui.menu_button(&node.item.title, |ui| {
                    self.menu_node(ui, node, &mut run_named);
                });
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.command_search(ui, cx, &mut run);
            });
        });
        for name in run_named {
            self.run_plugin_command(&name);
        }
        for cmd in run {
            self.execute(cmd);
        }
    }

    fn command_search(&mut self, ui: &mut Ui, cx: &ViewCx<'_>, run: &mut Vec<CommandId>) {
        let r = ui.add(
            egui::TextEdit::singleline(&mut self.search)
                .hint_text(cx.t("search.placeholder"))
                .desired_width(300.0)
                .id_salt("command_search"),
        );
        if self.focus_search {
            r.request_focus();
            self.focus_search = false;
        }
        let hits = CommandRegistry::search(&self.search, |c| cx.t(c.label_key));
        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            if let Some(first) = hits.first() {
                run.push(*first);
            }
            self.search.clear();
        }
        if !hits.is_empty() && r.has_focus() {
            egui::Area::new(egui::Id::new("command_hits"))
                .fixed_pos(r.rect.left_bottom())
                .order(egui::Order::Foreground)
                .show(ui.ctx(), |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        for id in hits.iter().take(8) {
                            if ui
                                .button(cx.t(CommandRegistry::get(*id).label_key))
                                .clicked()
                            {
                                run.push(*id);
                            }
                        }
                    });
                });
        }
    }

    fn nav_item(
        &mut self,
        ui: &mut Ui,
        cx: &ViewCx<'_>,
        row: &(&str, &str, &str, Option<CommandId>),
        active: &str,
    ) {
        let (section, icon, key, cmd) = *row;
        let Some(cmd) = cmd else {
            if icon.is_empty() {
                ui.add_space(8.0);
                if self.sidebar_open {
                    ui.label(RichText::new(cx.t(key)).strong());
                }
            } else {
                let text = if self.sidebar_open {
                    format!("{icon}  {}", cx.t(key))
                } else {
                    icon.to_owned()
                };
                ui.add_enabled(false, egui::Button::new(text).frame(false))
                    .on_disabled_hover_text(cx.t("common.developing"));
            }
            return;
        };
        let text = if self.sidebar_open {
            format!("{icon}  {}", cx.t(key))
        } else {
            icon.to_owned()
        };
        let selected = section == active;
        let r = ui.add_sized(
            [ui.available_width(), 32.0],
            egui::Button::selectable(selected, RichText::new(text).size(15.0))
                .frame_when_inactive(false),
        );
        if r.clicked() {
            self.execute(cmd);
        }
    }

    fn side_nav(&mut self, ui: &mut Ui, cx: &ViewCx<'_>) {
        let active = self.focused_route().map_or("", |r| r.nav_section());
        ui.with_layout(Layout::top_down(Align::Min), |ui| {
            ui.horizontal(|ui| {
                let back_enabled = self.is_enabled(CommandId::Back);
                if ui
                    .add_enabled(back_enabled, egui::Button::new("←").frame(false))
                    .clicked()
                {
                    self.execute(CommandId::Back);
                }
            });
            let tip = if self.sidebar_open {
                "tooltip.closeNavigation"
            } else {
                "tooltip.openNavigation"
            };
            if ui
                .add(egui::Button::new("☰").frame(false))
                .on_hover_text(cx.t(tip))
                .clicked()
            {
                self.execute(CommandId::ToggleSidebar);
            }
            ui.add_space(8.0);
            for row in NAV_TOP {
                self.nav_item(ui, cx, row, active);
            }
            ui.with_layout(Layout::bottom_up(Align::Min), |ui| {
                for row in NAV_BOTTOM.iter().rev() {
                    self.nav_item(ui, cx, row, active);
                }
            });
        });
    }

    fn status_bar(&self, ui: &mut Ui, cx: &ViewCx<'_>) {
        ui.horizontal(|ui| {
            let busy = self.exec.in_flight() > 0;
            ui.label(if busy {
                cx.t("status.syncing")
            } else {
                cx.t("status.ready")
            });
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(chrono::Local::now().format("%Y/%m/%d %H:%M:%S").to_string());
                ui.label(format!("🔄 {}", cx.t("status.synced")));
                let online = self.svc.network.is_online();
                let (icon, key) = if online {
                    ("📶", "status.online")
                } else {
                    ("⚠", "status.offline")
                };
                ui.label(format!("{icon} {}", cx.t(key)));
            });
        });
    }

    fn floating_windows(&mut self, ctx: &egui::Context, cx: &ViewCx<'_>, out: &mut ViewerOutput) {
        let mut dock_back = Vec::new();
        for tab_id in self.floating.clone() {
            let Some(tab) = self.tabs.get_mut(&tab_id) else {
                continue;
            };
            let title = tab.page.title(cx);
            let vid = ViewportId::from_hash_of(("floating", tab_id.0));
            let builder = ViewportBuilder::default()
                .with_title(format!("{title} — Arcana"))
                .with_inner_size([960.0, 720.0]);
            let mut closed = false;
            ctx.show_viewport_immediate(vid, builder, |ui, _class| {
                egui::Panel::top(egui::Id::new(("float_bar", tab_id.0))).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&title).strong());
                        if ui
                            .button(cx.t("common.dockBack"))
                            .on_hover_text(cx.t("common.dockBackTooltip"))
                            .clicked()
                        {
                            dock_back.push(tab_id);
                        }
                    });
                });
                egui::CentralPanel::default().show(ui, |ui| tab_ui(ui, tab, cx, out));
                if ui.ctx().input(|i| i.viewport().close_requested()) {
                    closed = true;
                }
            });
            if closed {
                dock_back.push(tab_id);
            }
        }
        for t in dock_back {
            self.dock_back(t);
        }
    }

    fn dialogs(&mut self, ctx: &egui::Context, cx: &ViewCx<'_>) {
        let Some(d) = self.dialog.clone() else {
            return;
        };
        let (title, body, ok) = match &d {
            Dialog::Delete { label, .. } => (
                cx.t("common.delete"),
                l10n::t_with(cx.l10n, "ui.confirmDelete", std::slice::from_ref(label)),
                cx.t("common.delete"),
            ),
            Dialog::Discard { .. } | Dialog::CloseDirty { .. } => (
                cx.t("common.discardChanges"),
                cx.t("common.discardChangesMessage"),
                cx.t("common.discardChanges"),
            ),
            Dialog::About => (
                cx.t("menu.help.about"),
                l10n::t_with(
                    cx.l10n,
                    "app.about.content",
                    &[env!("CARGO_PKG_VERSION").to_owned()],
                ),
                cx.t("common.ok"),
            ),
        };
        let mut answer = None;
        egui::Modal::new(egui::Id::new("shell_dialog")).show(ctx, |ui| {
            ui.set_width(380.0);
            ui.heading(title);
            ui.label(body);
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                if ui.button(ok).clicked() {
                    answer = Some(true);
                }
                if !matches!(d, Dialog::About) && ui.button(cx.t("common.cancel")).clicked() {
                    answer = Some(false);
                }
            });
        });
        if let Some(a) = answer {
            self.answer_dialog(a);
        }
    }

    fn toasts_overlay(&self, ctx: &egui::Context, cx: &ViewCx<'_>) {
        if self.toasts.is_empty() {
            return;
        }
        egui::Area::new(egui::Id::new("toasts"))
            .anchor(egui::Align2::RIGHT_BOTTOM, [-16.0, -40.0])
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                for t in &self.toasts {
                    let color = match t.notice.level {
                        NoticeLevel::Info => cx.palette.info,
                        NoticeLevel::Success => cx.palette.success,
                        NoticeLevel::Warning => Color32::from_rgb(230, 140, 0),
                        NoticeLevel::Error => Color32::from_rgb(196, 43, 28),
                    };
                    egui::Frame::new()
                        .fill(color)
                        .corner_radius(6)
                        .inner_margin(10)
                        .show(ui, |ui| {
                            ui.label(
                                RichText::new(l10n::text(cx.l10n, &t.notice.text))
                                    .color(Color32::WHITE),
                            );
                        });
                    ui.add_space(4.0);
                }
            });
    }
}
