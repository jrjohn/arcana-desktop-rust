//! Bridge between the plugin host and the shell: plugin menus, plugin commands, and the
//! navigation / shell-command / window requests plugins queue.

use arcana_plugin_api::MenuLocation;

use super::Shell;
use crate::commands::{CommandId, CommandRegistry};
use crate::effect::{Notice, NoticeLevel, Text};
use crate::plugin_bridge::{self, ShellAction};

pub use crate::plugin_menu::{
    Contribution, MenuNode, contributions, function_tree, icon_glyph, menu_tree, module_of,
    relocalize,
};

impl Shell {
    /// Contributed items at `location` (see [`contributions`]).
    #[must_use]
    pub fn contributed(&self, location: MenuLocation, module: Option<&str>) -> Vec<Contribution> {
        contributions(
            &self.svc.plugins.host.services.menus.all_menu_items(),
            location,
            module,
        )
    }

    /// Push the contributed quick actions to Home pages when the menu registry changed.
    pub(super) fn sync_contributions(&mut self) {
        let rev = self.svc.plugins.host.services.menus.revision();
        if self.contrib_rev == Some(rev) {
            return;
        }
        self.contrib_rev = Some(rev);
        let quick = self.quick_actions();
        for id in self.home_tabs() {
            self.send(
                id,
                crate::pages::PageMsg::Home(crate::pages::home::Input::SetQuickActions(
                    quick.clone(),
                )),
            );
        }
    }

    fn quick_actions(&self) -> Vec<crate::pages::home::QuickAction> {
        self.contributed(MenuLocation::QuickAccess, None)
            .into_iter()
            .filter_map(|c| {
                Some(crate::pages::home::QuickAction {
                    icon: icon_glyph(c.icon.as_deref()).to_owned(),
                    title: c.title,
                    command: c.command?,
                })
            })
            .collect()
    }

    fn home_tabs(&self) -> Vec<super::TabId> {
        self.tabs
            .iter()
            .filter(|(_, t)| matches!(t.page, crate::pages::Page::Home(_)))
            .map(|(id, _)| *id)
            .collect()
    }

    /// Force the next [`Shell::sync_contributions`] to push (a new Home tab opened).
    pub(super) fn invalidate_contributions(&mut self) {
        self.contrib_rev = None;
    }

    /// The plugin-contributed main menu.
    #[must_use]
    pub fn plugin_menu(&self) -> Vec<MenuNode> {
        menu_tree(&self.svc.plugins.host.services.menus.all_menu_items())
    }

    /// Whether a plugin menu command is currently enabled (shell commands follow
    /// [`Shell::is_enabled`]; plugin-only commands are enabled when registered).
    #[must_use]
    pub fn plugin_command_enabled(&self, name: &str) -> bool {
        match CommandRegistry::by_name(name) {
            Some(cmd) if !self.svc.plugins.host.services.commands.has_command(name) => {
                self.is_enabled(cmd)
            }
            Some(cmd @ (CommandId::Undo | CommandId::Redo | CommandId::Save)) => {
                self.is_enabled(cmd)
            }
            _ => self.svc.plugins.host.services.commands.has_command(name),
        }
    }

    /// Run a command from a plugin menu (C# `CommandService.ExecuteAsync`), then apply
    /// whatever it queued.
    pub fn run_plugin_command(&mut self, name: &str) {
        let commands = self.svc.plugins.host.services.commands.clone();
        if commands.has_command(name) {
            if let Err(e) = commands.execute(name, Vec::new()) {
                self.notify(Notice::error(e.to_user_message()));
            }
        } else if let Some(cmd) = CommandRegistry::by_name(name) {
            self.execute(cmd);
        } else {
            self.notify(Notice::new(
                NoticeLevel::Info,
                Text::Key("common.developing"),
            ));
        }
        self.pump_plugins();
    }

    /// Apply navigation, shell commands and window requests plugins queued.
    pub fn pump_plugins(&mut self) {
        for action in plugin_bridge::drain(&self.svc.plugins) {
            self.apply_plugin(action);
        }
    }

    fn apply_plugin(&mut self, action: ShellAction) {
        match action {
            ShellAction::Navigate(req) => {
                self.navigate(&req, None);
            }
            ShellAction::Notify(n) => self.notify(n),
            ShellAction::Back => self.on_focused(|s, t| {
                s.back(t);
            }),
            ShellAction::Close => self.on_focused(|s, t| {
                s.close_tab(t, false);
            }),
            ShellAction::Execute(cmd) => self.execute(cmd),
            ShellAction::Confirm(c) => self.queue_confirm(c.into()),
            ShellAction::Status { id, text } => self.set_status(id, text),
        }
    }

    fn on_focused(&mut self, f: impl FnOnce(&mut Self, super::TabId)) {
        if let Some(t) = self.focused_tab() {
            f(self, t);
        }
    }

    fn queue_confirm(&mut self, d: super::Dialog) {
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
