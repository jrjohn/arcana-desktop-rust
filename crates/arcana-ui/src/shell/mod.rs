//! The application shell (C# `MainWindow` + `DocumentTabHost` + `DocumentManager` +
//! `FloatingDocumentWindow`): menu bar, side navigation, document tabs, floating windows,
//! status bar, dialogs and toasts.
//!
//! The state machine (navigation, effect interpretation, commands, dialogs) lives in
//! [`crate::shell_vm::ShellViewModel`]; this module owns the dock layout, applies the
//! [`ShellEffect`]s that need it, and draws (in [`chrome`] and [`tabs`]).

mod chrome;
pub mod plugins;
pub mod shortcuts;
mod tabs;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use egui_dock::DockState;
use tokio::runtime::Handle;

use crate::commands::CommandId;
use crate::nav::{NavRequest, Route};
use crate::pages::{Page, PageMsg};
use crate::runtime::Executor;
use crate::services::UiServices;
pub use crate::shell_vm::{ACCENT_KEY, Dialog, Tab, Toast};
use crate::shell_vm::{ShellEffect, ShellInput, ShellViewModel};
use crate::theme;

pub use crate::runtime::TabId;

/// The egui shell: the dock layout plus the [`ShellViewModel`] it renders.
pub struct Shell {
    vm: ShellViewModel,
    dock: DockState<TabId>,
    search: String,
    /// One-time "no CJK font" warning, until dismissed.
    font_warning: bool,
    /// Menu registry revision the contributed quick actions were last pushed at.
    contrib_rev: Option<u64>,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("vm", &self.vm)
            .finish_non_exhaustive()
    }
}

impl Shell {
    /// Window title shown by the desktop entry point.
    pub const TITLE: &'static str = "Arcana - Enterprise Management System";

    /// Build the shell on `svc`, running tasks on `runtime`, and open Home (C# opens the
    /// home page at start-up).
    #[must_use]
    pub fn new(svc: UiServices, runtime: Handle) -> Self {
        let mut shell = Self {
            vm: ShellViewModel::new(svc, runtime, theme::ACCENTS.len()),
            dock: DockState::new(Vec::new()),
            search: String::new(),
            font_warning: false,
            contrib_rev: None,
        };
        shell.navigate(&NavRequest::new_tab(Route::Home), None);
        shell
    }

    // ---------- queries (used by views and tests) ----------

    /// The state machine behind the shell.
    #[must_use]
    pub fn view_model(&self) -> &ShellViewModel {
        &self.vm
    }

    /// All open tabs, docked and floating.
    #[must_use]
    pub fn tabs(&self) -> &BTreeMap<TabId, Tab> {
        &self.vm.tabs
    }

    /// Tabs shown in their own window.
    #[must_use]
    pub fn floating(&self) -> &[TabId] {
        &self.vm.floating
    }

    /// The docked tab that has focus.
    #[must_use]
    pub fn focused_tab(&self) -> Option<TabId> {
        let path = self.dock.focused_leaf()?;
        let leaf = self.dock.leaf(path).ok()?;
        leaf.tabs.get(leaf.active.0).copied()
    }

    /// Route of the focused tab.
    #[must_use]
    pub fn focused_route(&self) -> Option<Route> {
        self.focused_tab()
            .and_then(|t| self.vm.tabs.get(&t))
            .map(|t| t.page.route())
    }

    /// Tab showing `route` (same-document rules).
    #[must_use]
    pub fn find_tab(&self, route: &Route) -> Option<TabId> {
        self.vm.find_tab(route)
    }

    /// Mutable page access (tests, screenshots).
    pub fn page_mut(&mut self, tab: TabId) -> Option<&mut Page> {
        self.vm.tabs.get_mut(&tab).map(|t| &mut t.page)
    }

    #[must_use]
    pub fn dialog(&self) -> Option<&Dialog> {
        self.vm.dialog.as_ref()
    }

    #[must_use]
    pub fn toasts(&self) -> &[Toast] {
        &self.vm.toasts
    }

    /// Tell the shell whether a CJK font was found; when not, the status bar shows a warning
    /// once (until the user dismisses it).
    pub fn set_font_status(&mut self, status: &crate::fonts::FontStatus) {
        self.font_warning = *status == crate::fonts::FontStatus::Missing;
    }

    #[must_use]
    pub fn font_warning(&self) -> bool {
        self.font_warning
    }

    /// Plugin status-bar items, by id.
    #[must_use]
    pub fn status_items(&self) -> &BTreeMap<String, String> {
        &self.vm.status_items
    }

    #[must_use]
    pub fn theme_id(&self) -> &str {
        &self.vm.theme_id
    }

    #[must_use]
    pub fn accent(&self) -> Option<usize> {
        self.vm.accent
    }

    #[must_use]
    pub fn services(&self) -> &UiServices {
        self.vm.services()
    }

    #[must_use]
    pub fn executor(&self) -> &Executor {
        self.vm.executor()
    }

    /// Order of tabs in the main dock surface (for reorder tests).
    #[must_use]
    pub fn docked_order(&self) -> Vec<TabId> {
        self.dock.iter_all_tabs().map(|(_, t)| *t).collect()
    }

    // ---------- inputs -> view model -> effects ----------

    /// Hand an input to the view model and carry out what it asks of the view.
    fn dispatch(&mut self, input: ShellInput) {
        let fx = self.vm.on(input);
        self.run_effects(fx);
    }

    fn run_effects(&mut self, effects: Vec<ShellEffect>) {
        for e in effects {
            match e {
                ShellEffect::HomeOpened => {
                    self.invalidate_contributions();
                    self.sync_contributions();
                }
                ShellEffect::PluginCommand(name) => self.run_plugin_command(&name),
                ShellEffect::PumpPlugins => self.pump_plugins(),
                dock => self.apply_dock(&dock),
            }
        }
    }

    /// Carry out a dock-layout effect.
    fn apply_dock(&mut self, e: &ShellEffect) {
        match *e {
            ShellEffect::DockPush(tab) => self.dock.push_to_focused_leaf(tab),
            ShellEffect::DockRemove(tab) => self.undock(tab),
            ShellEffect::DockFocus(tab) => self.focus(tab),
            _ => {}
        }
    }

    fn undock(&mut self, tab: TabId) {
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.remove_tab(path);
        }
    }

    fn focus(&mut self, tab: TabId) {
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.set_active_tab(path);
            self.dock.set_focused_node_and_surface(path.node_path());
        }
    }

    /// Execute a navigation request (C# `NavigationService`). `origin` is the tab that asked
    /// (for within-tab navigation); `None` means the focused tab.
    pub fn navigate(&mut self, req: &NavRequest, origin: Option<TabId>) -> TabId {
        let origin = origin.or_else(|| self.focused_tab());
        let mut fx = Vec::new();
        let tab = self.vm.navigate(req, origin, &mut fx);
        self.run_effects(fx);
        tab
    }

    /// C# `NavGraph.Back` within a tab.
    pub fn back(&mut self, tab: TabId) -> bool {
        let could = self.vm.tabs.get(&tab).is_some_and(Tab::can_go_back);
        self.dispatch(ShellInput::Back(tab));
        could
    }

    /// Close a tab; with unsaved edits and `force = false` ask first.
    pub fn close_tab(&mut self, tab: TabId, force: bool) {
        self.dispatch(ShellInput::Close { tab, force });
    }

    /// Tear a docked tab off into its own window (C# "Pop Out").
    pub fn pop_out(&mut self, tab: TabId) {
        self.dispatch(ShellInput::PopOut(tab));
    }

    /// Return a floating tab to the dock (C# "Dock Back").
    pub fn dock_back(&mut self, tab: TabId) {
        self.dispatch(ShellInput::DockBack(tab));
    }

    /// Move a docked tab to position `to` within its leaf (tab reorder; drag-and-drop in the
    /// dock does the same through `egui_dock`).
    pub fn reorder(&mut self, tab: TabId, to: usize) {
        let Some(path) = self.dock.find_tab(&tab) else {
            return;
        };
        if let Ok(leaf) = self.dock.leaf_mut(path.node_path()) {
            let from = path.tab.0;
            if from < leaf.tabs.len() {
                let t = leaf.tabs.remove(from);
                let to = to.min(leaf.tabs.len());
                leaf.tabs.insert(to, t);
                leaf.active = egui_dock::TabIndex(to);
            }
        }
    }

    /// Deliver an input to the page in `tab`.
    pub fn send(&mut self, tab: TabId, msg: PageMsg) {
        self.dispatch(ShellInput::Send(tab, msg));
    }

    /// Process finished async tasks.
    pub fn pump(&mut self) {
        self.dispatch(ShellInput::Pump);
    }

    /// Process results until no task is in flight (or `timeout`). Tests and the screenshot
    /// tool use this to reach a settled UI.
    pub fn settle(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            let batch = self
                .vm
                .executor()
                .wait_idle(deadline.saturating_duration_since(Instant::now()));
            if batch.is_empty() {
                break;
            }
            for (tab, msg) in batch {
                self.send(tab, msg);
            }
            if Instant::now() >= deadline {
                break;
            }
        }
    }

    /// Answer the open dialog.
    pub fn answer_dialog(&mut self, yes: bool) {
        self.dispatch(ShellInput::AnswerDialog(yes));
    }

    /// Run a command from any source (menu, shortcut, toolbar, command search).
    pub fn execute(&mut self, cmd: CommandId) {
        let focused = self.focused_tab();
        self.dispatch(ShellInput::Execute { cmd, focused });
    }

    /// Whether a command can run right now (greys out menu items).
    #[must_use]
    pub fn is_enabled(&self, cmd: CommandId) -> bool {
        let focused = self.focused_tab().and_then(|t| self.vm.tabs.get(&t));
        match cmd {
            CommandId::Save | CommandId::Refresh => {
                focused.is_some_and(|t| t.page.command(cmd).is_some())
            }
            CommandId::Back => focused.is_some_and(Tab::can_go_back),
            CommandId::CloseTab | CommandId::PopOut => focused.is_some(),
            CommandId::Undo | CommandId::Redo => false,
            _ => true,
        }
    }
}
