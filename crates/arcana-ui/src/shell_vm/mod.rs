//! The shell's state machine (C# `DocumentManager` + `NavigationService` + the dialog and
//! command handling of `MainWindow`), UI-agnostic: it owns the open documents, dialogs, toasts
//! and chrome toggles, and turns a [`ShellInput`] into state changes plus the [`ShellEffect`]s
//! only the view can carry out (the dock layout, plugin menus). The egui shell renders this
//! state and translates its events into inputs (MVVM-UDF, like the page view models).

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, VecDeque};
use std::time::{Duration, Instant};

use tokio::runtime::Handle;

use crate::commands::{CommandId, CommandRegistry};
use crate::effect::{DataKind, Effect, Notice, NoticeLevel, Text};
use crate::nav::{NavMode, NavRequest, Route};
use crate::pages::{Page, PageMsg, PageTask, run_task, settings};
use crate::runtime::{Executor, TabId};
use crate::services::UiServices;

/// Settings key for the accent override (index into the theme's accent list).
pub const ACCENT_KEY: &str = "app.accent";

/// How long a toast stays visible.
const TOAST_TTL: Duration = Duration::from_secs(4);

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
    /// A plugin asked a yes/no question (`WindowService::show_confirm`); the answer goes back
    /// through `reply_command` with a `bool` argument.
    PluginConfirm {
        title: String,
        message: String,
        reply_command: Option<String>,
    },
}

impl From<crate::plugin_bridge::ConfirmRequest> for Dialog {
    fn from(c: crate::plugin_bridge::ConfirmRequest) -> Self {
        Self::PluginConfirm {
            title: c.title,
            message: c.message,
            reply_command: c.reply_command,
        }
    }
}

/// A visible toast.
#[derive(Debug, Clone)]
pub struct Toast {
    pub notice: Notice,
    pub until: Instant,
}

/// One document tab (C# `DocumentTabHost` document): a page plus its within-tab back stack.
#[derive(Debug)]
pub struct Tab {
    pub id: TabId,
    pub page: Page,
    back: Vec<Page>,
}

impl Tab {
    #[must_use]
    pub fn new(id: TabId, page: Page) -> Self {
        Self {
            id,
            page,
            back: Vec::new(),
        }
    }

    /// Navigate within the tab, keeping the current page on the back stack.
    pub fn push(&mut self, page: Page) {
        let old = std::mem::replace(&mut self.page, page);
        self.back.push(old);
    }

    /// Go back within the tab; `false` when there is nothing to go back to.
    pub fn back(&mut self) -> bool {
        match self.back.pop() {
            Some(p) => {
                self.page = p;
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn can_go_back(&self) -> bool {
        !self.back.is_empty()
    }
}

/// What the view asks the shell to do. `focused` is the docked tab with focus, which only
/// the view (the dock layout) knows.
// Inputs are short-lived values handed straight to `on`; boxing the page message buys nothing.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum ShellInput {
    /// Open `req`; `origin` is the tab that asked (within-tab navigation).
    Navigate {
        req: NavRequest,
        origin: Option<TabId>,
    },
    /// Run a command (menu, shortcut, toolbar, command search).
    Execute {
        cmd: CommandId,
        focused: Option<TabId>,
    },
    /// Answer the open dialog.
    AnswerDialog(bool),
    /// Deliver a page input to a tab.
    Send(TabId, PageMsg),
    /// Close a tab; with unsaved edits and `force = false` ask first.
    Close { tab: TabId, force: bool },
    /// Go back within a tab.
    Back(TabId),
    /// Tear a docked tab off into its own window.
    PopOut(TabId),
    /// Return a floating tab to the dock.
    DockBack(TabId),
    /// Process finished async tasks.
    Pump,
    /// Show a toast.
    Notify(Notice),
    /// A plugin's question: shown now, or queued behind the open dialog.
    Confirm(Dialog),
    /// Set (`Some`) or remove (`None`) a plugin status-bar item.
    Status { id: String, text: Option<String> },
}

/// What only the view can do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellEffect {
    /// Add a tab to the focused dock leaf.
    DockPush(TabId),
    /// Remove a tab from the dock.
    DockRemove(TabId),
    /// Give a docked tab focus.
    DockFocus(TabId),
    /// A Home page was opened: push the plugin-contributed quick actions to it.
    HomeOpened,
    /// Run a plugin menu command.
    PluginCommand(String),
    /// Apply what plugins queued.
    PumpPlugins,
}

/// Shell state.
#[allow(clippy::struct_excessive_bools)]
pub struct ShellViewModel {
    svc: UiServices,
    exec: Executor,
    next_id: u64,
    pub tabs: BTreeMap<TabId, Tab>,
    /// Tabs torn off into their own OS window.
    pub floating: Vec<TabId>,
    pub toasts: Vec<Toast>,
    pub dialog: Option<Dialog>,
    /// Plugin confirmations waiting for the current dialog to close.
    pub pending_confirms: VecDeque<Dialog>,
    pub theme_id: String,
    pub accent: Option<usize>,
    /// The theme changed and the view must re-apply it.
    pub theme_dirty: bool,
    pub sidebar_open: bool,
    pub statusbar_visible: bool,
    pub fullscreen: bool,
    pub exit_requested: bool,
    pub focus_search: bool,
    /// Plugin status-bar items, by id.
    pub status_items: BTreeMap<String, String>,
}

impl std::fmt::Debug for ShellViewModel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ShellViewModel")
            .field("tabs", &self.tabs.len())
            .field("floating", &self.floating)
            .field("dialog", &self.dialog)
            .finish_non_exhaustive()
    }
}

impl ShellViewModel {
    /// Restore the saved culture, theme and accent (`accents` = how many accents the theme
    /// offers); the caller then opens Home.
    #[must_use]
    pub fn new(svc: UiServices, runtime: Handle, accents: usize) -> Self {
        crate::l10n::register(svc.localization.as_ref());
        let culture = svc.app_settings.language_code();
        if !culture.is_empty() && culture != svc.localization.current_culture() {
            svc.localization.set_culture(&culture);
            crate::plugin_menu::relocalize(&svc.plugins);
        }
        let theme_id = svc.app_settings.theme_id();
        let accent = saved_accent(&svc, accents);
        Self::with_state(svc, Executor::new(runtime), theme_id, accent)
    }

    fn with_state(
        svc: UiServices,
        exec: Executor,
        theme_id: String,
        accent: Option<usize>,
    ) -> Self {
        Self {
            svc,
            exec,
            next_id: 1,
            tabs: BTreeMap::new(),
            floating: Vec::new(),
            toasts: Vec::new(),
            dialog: None,
            pending_confirms: VecDeque::new(),
            theme_id,
            accent,
            theme_dirty: true,
            sidebar_open: true,
            statusbar_visible: true,
            fullscreen: false,
            exit_requested: false,
            focus_search: false,
            status_items: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn services(&self) -> &UiServices {
        &self.svc
    }

    #[must_use]
    pub fn executor(&self) -> &Executor {
        &self.exec
    }

    /// Tab showing `route` (same-document rules).
    #[must_use]
    pub fn find_tab(&self, route: &Route) -> Option<TabId> {
        self.tabs
            .iter()
            .find(|(_, t)| t.page.route().same_document(route))
            .map(|(id, _)| *id)
    }

    /// Handle one input.
    pub fn on(&mut self, input: ShellInput) -> Vec<ShellEffect> {
        let mut fx = Vec::new();
        match input {
            ShellInput::Navigate { req, origin } => {
                self.navigate(&req, origin, &mut fx);
            }
            ShellInput::Execute { cmd, focused } => self.execute(cmd, focused, &mut fx),
            ShellInput::AnswerDialog(yes) => self.answer_dialog(yes, &mut fx),
            ShellInput::Send(tab, msg) => self.send(tab, msg, &mut fx),
            ShellInput::Close { tab, force } => self.close_tab(tab, force, &mut fx),
            ShellInput::Back(tab) => {
                self.back(tab);
            }
            ShellInput::PopOut(tab) => self.pop_out(tab, &mut fx),
            ShellInput::DockBack(tab) => self.dock_back(tab, &mut fx),
            ShellInput::Pump => self.pump(&mut fx),
            ShellInput::Notify(n) => self.notify(n),
            ShellInput::Confirm(d) => self.queue_confirm(d),
            ShellInput::Status { id, text } => self.set_status(id, text),
        }
        fx
    }

    /// [`ShellInput::Navigate`], also returning the tab that shows `req`.
    pub fn navigate(
        &mut self,
        req: &NavRequest,
        origin: Option<TabId>,
        fx: &mut Vec<ShellEffect>,
    ) -> TabId {
        let tab = self.open(req, origin, fx);
        if req.route == Route::Home {
            fx.push(ShellEffect::HomeOpened);
        }
        tab
    }

    fn open(
        &mut self,
        req: &NavRequest,
        origin: Option<TabId>,
        fx: &mut Vec<ShellEffect>,
    ) -> TabId {
        let snapshot = self.settings_snapshot();
        if req.mode == NavMode::WithinTab
            && let Some(tab) = origin
            && let Some(t) = self.tabs.get_mut(&tab)
        {
            let (page, page_fx) = Page::open(&req.route, &snapshot);
            t.push(page);
            self.apply(tab, page_fx, fx);
            return tab;
        }
        if let Some(existing) = self.find_tab(&req.route) {
            if !self.floating.contains(&existing) {
                fx.push(ShellEffect::DockFocus(existing));
            }
            return existing;
        }
        let id = TabId(self.next_id);
        self.next_id += 1;
        let (page, page_fx) = Page::open(&req.route, &snapshot);
        self.tabs.insert(id, Tab::new(id, page));
        fx.extend([ShellEffect::DockPush(id), ShellEffect::DockFocus(id)]);
        self.apply(id, page_fx, fx);
        id
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

    fn back(&mut self, tab: TabId) -> bool {
        self.tabs.get_mut(&tab).is_some_and(Tab::back)
    }

    fn close_tab(&mut self, tab: TabId, force: bool, fx: &mut Vec<ShellEffect>) {
        let Some(t) = self.tabs.get(&tab) else {
            return;
        };
        if !force && t.page.is_dirty() {
            self.dialog = Some(Dialog::CloseDirty { tab });
            return;
        }
        if !self.floating.contains(&tab) {
            fx.push(ShellEffect::DockRemove(tab));
        }
        self.floating.retain(|f| *f != tab);
        self.tabs.remove(&tab);
    }

    fn pop_out(&mut self, tab: TabId, fx: &mut Vec<ShellEffect>) {
        if self.tabs.contains_key(&tab) && !self.floating.contains(&tab) {
            self.floating.push(tab);
            fx.push(ShellEffect::DockRemove(tab));
        }
    }

    fn dock_back(&mut self, tab: TabId, fx: &mut Vec<ShellEffect>) {
        if let Some(pos) = self.floating.iter().position(|f| *f == tab) {
            self.floating.remove(pos);
            fx.extend([ShellEffect::DockPush(tab), ShellEffect::DockFocus(tab)]);
        }
    }

    fn send(&mut self, tab: TabId, msg: PageMsg, fx: &mut Vec<ShellEffect>) {
        let page_fx = match self.tabs.get_mut(&tab) {
            Some(t) => t.page.on(msg),
            None => return,
        };
        self.apply(tab, page_fx, fx);
    }

    fn pump(&mut self, fx: &mut Vec<ShellEffect>) {
        for (tab, msg) in self.exec.drain() {
            self.send(tab, msg, fx);
        }
    }

    fn notify(&mut self, notice: Notice) {
        self.toasts.push(Toast {
            notice,
            until: Instant::now() + TOAST_TTL,
        });
    }

    /// Interpret a page's effects.
    fn apply(&mut self, tab: TabId, effects: Vec<Effect<PageTask>>, fx: &mut Vec<ShellEffect>) {
        for e in effects {
            match e {
                Effect::Run(task) => self.exec.spawn(tab, run_task(task, self.svc.clone())),
                Effect::Navigate(req) => {
                    self.navigate(&req, Some(tab), fx);
                }
                Effect::CloseSelf => self.close_tab(tab, true, fx),
                Effect::Notify(n) => self.notify(n),
                Effect::ConfirmDiscard => self.dialog = Some(Dialog::Discard { tab }),
                Effect::ConfirmDelete { id, label } => {
                    self.dialog = Some(Dialog::Delete { tab, id, label });
                }
                Effect::DataChanged(kind) => self.broadcast(tab, kind, fx),
                Effect::Command(name) => fx.push(ShellEffect::PluginCommand(name)),
                Effect::Theme(_) | Effect::Accent(_) | Effect::Culture(_) => self.apply_setting(e),
            }
        }
    }

    /// Persist an appearance / language change.
    fn apply_setting(&mut self, e: Effect<PageTask>) {
        match e {
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
                crate::plugin_menu::relocalize(&self.svc.plugins);
            }
            _ => {}
        }
    }

    fn broadcast(&mut self, from: TabId, kind: DataKind, fx: &mut Vec<ShellEffect>) {
        let targets: Vec<(TabId, PageMsg)> = self
            .tabs
            .iter()
            .filter(|(id, _)| **id != from)
            .filter_map(|(id, t)| t.page.on_data_changed(kind).map(|m| (*id, m)))
            .collect();
        for (id, msg) in targets {
            self.send(id, msg, fx);
        }
    }

    fn answer_dialog(&mut self, yes: bool, fx: &mut Vec<ShellEffect>) {
        let Some(d) = self.dialog.take() else {
            return;
        };
        self.dialog = self.pending_confirms.pop_front();
        match d {
            Dialog::PluginConfirm {
                reply_command: Some(cmd),
                ..
            } => self.reply_to_plugin(&cmd, yes, fx),
            _ if !yes => {}
            Dialog::Delete { tab, id, .. } => {
                let msg = self.tabs.get(&tab).and_then(|t| t.page.confirm_delete(id));
                self.send_some(tab, msg, fx);
            }
            Dialog::Discard { tab } => {
                let msg = self.tabs.get(&tab).and_then(|t| t.page.confirm_discard());
                self.send_some(tab, msg, fx);
            }
            Dialog::CloseDirty { tab } => self.close_tab(tab, true, fx),
            Dialog::About | Dialog::PluginConfirm { .. } => {}
        }
    }

    fn reply_to_plugin(&mut self, cmd: &str, yes: bool, fx: &mut Vec<ShellEffect>) {
        let commands = self.svc.plugins.host.services.commands.clone();
        if let Err(e) = commands.execute(cmd, vec![Box::new(yes)]) {
            self.notify(Notice::error(e.to_user_message()));
        }
        fx.push(ShellEffect::PumpPlugins);
    }

    fn send_some(&mut self, tab: TabId, msg: Option<PageMsg>, fx: &mut Vec<ShellEffect>) {
        if let Some(msg) = msg {
            self.send(tab, msg, fx);
        }
    }

    fn execute(&mut self, cmd: CommandId, focused: Option<TabId>, fx: &mut Vec<ShellEffect>) {
        if let Some(req) = CommandRegistry::navigation(cmd) {
            self.navigate(&req, focused, fx);
            return;
        }
        match (cmd, focused) {
            (CommandId::Save | CommandId::Refresh, Some(tab)) => {
                let msg = self.tabs.get(&tab).and_then(|t| t.page.command(cmd));
                self.send_some(tab, msg, fx);
            }
            (CommandId::Back, Some(tab)) => {
                self.back(tab);
            }
            (CommandId::CloseTab, Some(tab)) => self.close_tab(tab, false, fx),
            (CommandId::PopOut, Some(tab)) => self.pop_out(tab, fx),
            (
                CommandId::Save
                | CommandId::Refresh
                | CommandId::Back
                | CommandId::CloseTab
                | CommandId::PopOut,
                None,
            ) => {}
            _ => self.toggle(cmd),
        }
    }

    /// Chrome commands that need no tab.
    fn toggle(&mut self, cmd: CommandId) {
        match cmd {
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

    fn queue_confirm(&mut self, d: Dialog) {
        if self.dialog.is_none() {
            self.dialog = Some(d);
        } else {
            self.pending_confirms.push_back(d);
        }
    }

    fn set_status(&mut self, id: String, text: Option<String>) {
        match text {
            Some(t) => {
                self.status_items.insert(id, t);
            }
            None => {
                self.status_items.remove(&id);
            }
        }
    }
}

fn saved_accent(svc: &UiServices, accents: usize) -> Option<usize> {
    svc.settings
        .get_value(ACCENT_KEY)
        .and_then(|v| v.as_u64())
        .and_then(|v| usize::try_from(v).ok())
        .filter(|i| *i < accents)
}
