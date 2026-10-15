//! The application shell (C# `MainWindow` + `DocumentTabHost` + `DocumentManager` +
//! `FloatingDocumentWindow`): menu bar, side navigation, document tabs, floating windows,
//! status bar, dialogs and toasts.
//!
//! The logic here (navigation, effect interpretation, commands) does not draw and is tested
//! without egui rendering; drawing lives in [`chrome`] and [`tabs`].

mod chrome;
mod tabs;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use egui_dock::DockState;
use tokio::runtime::Handle;

use crate::commands::{CommandId, CommandRegistry};
use crate::effect::{DataKind, Effect, Notice, NoticeLevel, Text};
use crate::nav::{NavMode, NavRequest, Route};
use crate::pages::{Page, PageMsg, PageTask, run_task, settings};
use crate::runtime::Executor;
use crate::services::UiServices;
use crate::theme;
pub use tabs::Tab;

/// Settings key for the accent override (index into [`theme::ACCENTS`]).
pub const ACCENT_KEY: &str = "app.accent";

/// How long a toast stays visible.
const TOAST_TTL: Duration = Duration::from_secs(4);

/// Stable identity of a document tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TabId(pub u64);

/// A modal question.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Dialog {
    /// A page asked to confirm deleting `label`.
    Delete {
        tab: TabId,
        id: i32,
        label: String,
    },
    /// A page asked to confirm discarding unsaved edits.
    Discard {
        tab: TabId,
    },
    /// The user closed a tab with unsaved edits.
    CloseDirty {
        tab: TabId,
    },
    About,
}

/// A visible toast.
#[derive(Debug, Clone)]
pub struct Toast {
    pub notice: Notice,
    pub until: Instant,
}

/// Shell state.
#[allow(clippy::struct_excessive_bools)]
pub struct Shell {
    svc: UiServices,
    exec: Executor,
    dock: DockState<TabId>,
    tabs: BTreeMap<TabId, Tab>,
    /// Tabs torn off into their own OS window (egui viewport).
    floating: Vec<TabId>,
    next_id: u64,
    toasts: Vec<Toast>,
    dialog: Option<Dialog>,
    theme_id: String,
    accent: Option<usize>,
    theme_dirty: bool,
    sidebar_open: bool,
    statusbar_visible: bool,
    fullscreen: bool,
    exit_requested: bool,
    search: String,
    focus_search: bool,
}

impl std::fmt::Debug for Shell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Shell")
            .field("tabs", &self.tabs.len())
            .field("floating", &self.floating)
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
        crate::l10n::register(svc.localization.as_ref());
        let culture = svc.app_settings.language_code();
        if !culture.is_empty() {
            svc.localization.set_culture(&culture);
        }
        let theme_id = svc.app_settings.theme_id();
        let accent = svc
            .settings
            .get_value(ACCENT_KEY)
            .and_then(|v| v.as_u64())
            .and_then(|v| usize::try_from(v).ok())
            .filter(|i| *i < theme::ACCENTS.len());
        let mut shell = Self {
            svc,
            exec: Executor::new(runtime),
            dock: DockState::new(Vec::new()),
            tabs: BTreeMap::new(),
            floating: Vec::new(),
            next_id: 1,
            toasts: Vec::new(),
            dialog: None,
            theme_id,
            accent,
            theme_dirty: true,
            sidebar_open: true,
            statusbar_visible: true,
            fullscreen: false,
            exit_requested: false,
            search: String::new(),
            focus_search: false,
        };
        shell.navigate(&NavRequest::new_tab(Route::Home), None);
        shell
    }

    // ---------- queries (used by views and tests) ----------

    /// All open tabs, docked and floating.
    #[must_use]
    pub fn tabs(&self) -> &BTreeMap<TabId, Tab> {
        &self.tabs
    }

    /// Tabs shown in their own window.
    #[must_use]
    pub fn floating(&self) -> &[TabId] {
        &self.floating
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
            .and_then(|t| self.tabs.get(&t))
            .map(|t| t.page.route())
    }

    /// Tab showing `route` (same-document rules).
    #[must_use]
    pub fn find_tab(&self, route: &Route) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|(_, t)| t.page.route().same_document(route))
            .map(|(id, _)| *id)
    }

    /// Mutable page access (tests, screenshots).
    pub fn page_mut(&mut self, tab: TabId) -> Option<&mut Page> {
        self.tabs.get_mut(&tab).map(|t| &mut t.page)
    }

    #[must_use]
    pub fn dialog(&self) -> Option<&Dialog> {
        self.dialog.as_ref()
    }

    #[must_use]
    pub fn toasts(&self) -> &[Toast] {
        &self.toasts
    }

    #[must_use]
    pub fn theme_id(&self) -> &str {
        &self.theme_id
    }

    #[must_use]
    pub fn accent(&self) -> Option<usize> {
        self.accent
    }

    #[must_use]
    pub fn services(&self) -> &UiServices {
        &self.svc
    }

    #[must_use]
    pub fn executor(&self) -> &Executor {
        &self.exec
    }

    /// Order of tabs in the main dock surface (for reorder tests).
    #[must_use]
    pub fn docked_order(&self) -> Vec<TabId> {
        self.dock.iter_all_tabs().map(|(_, t)| *t).collect()
    }

    fn settings_snapshot(&self) -> settings::Snapshot {
        let a = &self.svc.app_settings;
        settings::Snapshot {
            theme_id: self.theme_id.clone(),
            accent: self.accent,
            culture: self.svc.localization.current_culture(),
            auto_sync: a.auto_sync_enabled(),
            sync_minutes: a.sync_frequency_minutes(),
        }
    }

    // ---------- navigation ----------

    fn alloc(&mut self) -> TabId {
        let id = TabId(self.next_id);
        self.next_id += 1;
        id
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
        let snapshot = self.settings_snapshot();
        if req.mode == NavMode::WithinTab
            && let Some(tab) = origin.or_else(|| self.focused_tab())
            && let Some(t) = self.tabs.get_mut(&tab)
        {
            let (page, fx) = Page::open(&req.route, &snapshot);
            t.push(page);
            self.apply(tab, fx);
            return tab;
        }
        if let Some(existing) = self.find_tab(&req.route) {
            if !self.floating.contains(&existing) {
                self.focus(existing);
            }
            return existing;
        }
        let id = self.alloc();
        let (page, fx) = Page::open(&req.route, &snapshot);
        self.tabs.insert(id, Tab::new(id, page));
        self.dock.push_to_focused_leaf(id);
        self.focus(id);
        self.apply(id, fx);
        id
    }

    /// C# `NavGraph.Back` within a tab.
    pub fn back(&mut self, tab: TabId) -> bool {
        self.tabs.get_mut(&tab).is_some_and(Tab::back)
    }

    // ---------- tabs ----------

    /// Close a tab; with unsaved edits and `force = false` ask first.
    pub fn close_tab(&mut self, tab: TabId, force: bool) {
        let Some(t) = self.tabs.get(&tab) else {
            return;
        };
        if !force && t.page.is_dirty() {
            self.dialog = Some(Dialog::CloseDirty { tab });
            return;
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.remove_tab(path);
        }
        self.floating.retain(|f| *f != tab);
        self.tabs.remove(&tab);
    }

    /// Tear a docked tab off into its own window (C# "Pop Out").
    pub fn pop_out(&mut self, tab: TabId) {
        if self.floating.contains(&tab) {
            return;
        }
        if let Some(path) = self.dock.find_tab(&tab) {
            let _ = self.dock.remove_tab(path);
            self.floating.push(tab);
        }
    }

    /// Return a floating tab to the dock (C# "Dock Back").
    pub fn dock_back(&mut self, tab: TabId) {
        if let Some(pos) = self.floating.iter().position(|f| *f == tab) {
            self.floating.remove(pos);
            self.dock.push_to_focused_leaf(tab);
            self.focus(tab);
        }
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

    // ---------- messages & effects ----------

    /// Deliver an input to the page in `tab`.
    pub fn send(&mut self, tab: TabId, msg: PageMsg) {
        let fx = match self.tabs.get_mut(&tab) {
            Some(t) => t.page.on(msg),
            None => return,
        };
        self.apply(tab, fx);
    }

    /// Process finished async tasks.
    pub fn pump(&mut self) {
        for (tab, msg) in self.exec.drain() {
            self.send(tab, msg);
        }
    }

    /// Process results until no task is in flight (or `timeout`). Tests and the screenshot
    /// tool use this to reach a settled UI.
    pub fn settle(&mut self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        loop {
            let batch = self
                .exec
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

    fn notify(&mut self, notice: Notice) {
        self.toasts.push(Toast {
            notice,
            until: Instant::now() + TOAST_TTL,
        });
    }

    fn apply(&mut self, tab: TabId, effects: Vec<Effect<PageTask>>) {
        for e in effects {
            match e {
                Effect::Run(task) => {
                    let svc = self.svc.clone();
                    self.exec.spawn(tab, run_task(task, svc));
                }
                Effect::Navigate(req) => {
                    self.navigate(&req, Some(tab));
                }
                Effect::CloseSelf => self.close_tab(tab, true),
                Effect::Notify(n) => self.notify(n),
                Effect::ConfirmDiscard => self.dialog = Some(Dialog::Discard { tab }),
                Effect::ConfirmDelete { id, label } => {
                    self.dialog = Some(Dialog::Delete { tab, id, label });
                }
                Effect::DataChanged(kind) => self.broadcast(tab, kind),
                Effect::Theme(id) => {
                    self.svc.app_settings.set_theme_id(&id);
                    self.theme_id = id;
                    self.theme_dirty = true;
                }
                Effect::Accent(a) => {
                    let value = a.map_or(serde_json::Value::Null, |i| {
                        serde_json::Value::from(i as u64)
                    });
                    if let Err(e) = self.svc.settings.set_value(ACCENT_KEY, value) {
                        self.notify(Notice::error(e.to_user_message()));
                    }
                    self.accent = a;
                    self.theme_dirty = true;
                }
                Effect::Culture(c) => {
                    self.svc.localization.set_culture(&c);
                    self.svc.app_settings.set_language_code(&c);
                }
            }
        }
    }

    fn broadcast(&mut self, from: TabId, kind: DataKind) {
        let targets: Vec<(TabId, PageMsg)> = self
            .tabs
            .iter()
            .filter(|(id, _)| **id != from)
            .filter_map(|(id, t)| t.page.on_data_changed(kind).map(|m| (*id, m)))
            .collect();
        for (id, msg) in targets {
            self.send(id, msg);
        }
    }

    /// Answer the open dialog.
    pub fn answer_dialog(&mut self, yes: bool) {
        let Some(d) = self.dialog.take() else {
            return;
        };
        if !yes {
            return;
        }
        match d {
            Dialog::Delete { tab, id, .. } => {
                if let Some(msg) = self.tabs.get(&tab).and_then(|t| t.page.confirm_delete(id)) {
                    self.send(tab, msg);
                }
            }
            Dialog::Discard { tab } => {
                if let Some(msg) = self.tabs.get(&tab).and_then(|t| t.page.confirm_discard()) {
                    self.send(tab, msg);
                }
            }
            Dialog::CloseDirty { tab } => self.close_tab(tab, true),
            Dialog::About => {}
        }
    }

    // ---------- commands ----------

    /// Run a command from any source (menu, shortcut, toolbar, command search).
    pub fn execute(&mut self, cmd: CommandId) {
        if let Some(req) = CommandRegistry::navigation(cmd) {
            self.navigate(&req, None);
            return;
        }
        let focused = self.focused_tab();
        match cmd {
            CommandId::Save | CommandId::Refresh => {
                if let Some(tab) = focused
                    && let Some(msg) = self.tabs.get(&tab).and_then(|t| t.page.command(cmd))
                {
                    self.send(tab, msg);
                }
            }
            CommandId::Back => {
                if let Some(tab) = focused {
                    self.back(tab);
                }
            }
            CommandId::CloseTab => {
                if let Some(tab) = focused {
                    self.close_tab(tab, false);
                }
            }
            CommandId::PopOut => {
                if let Some(tab) = focused {
                    self.pop_out(tab);
                }
            }
            CommandId::ToggleSidebar => self.sidebar_open = !self.sidebar_open,
            CommandId::ToggleStatusBar => self.statusbar_visible = !self.statusbar_visible,
            CommandId::Fullscreen => self.fullscreen = !self.fullscreen,
            CommandId::Exit => self.exit_requested = true,
            CommandId::CommandSearch => self.focus_search = true,
            CommandId::About => self.dialog = Some(Dialog::About),
            // C# registers these menu items without handlers; text fields handle clipboard
            // natively, and undo/redo arrive with op-based commands.
            _ => self.notify(Notice::new(
                NoticeLevel::Info,
                Text::Key("common.developing"),
            )),
        }
    }

    /// Whether a command can run right now (greys out menu items).
    #[must_use]
    pub fn is_enabled(&self, cmd: CommandId) -> bool {
        let focused = self.focused_tab().and_then(|t| self.tabs.get(&t));
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
