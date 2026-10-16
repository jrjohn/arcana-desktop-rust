//! Bridge between the plugin host and the shell: plugin menus, plugin commands, and the
//! navigation / shell-command / window requests plugins queue.

use arcana_plugin_api::MenuLocation;

use super::Shell;
use crate::plugin_bridge::{self, CommandOutcome, ShellAction};
use crate::shell_vm::ShellInput;

pub use crate::plugin_menu::{
    Contribution, MenuNode, contributions, function_tree, icon_glyph, menu_tree, module_of,
    relocalize,
};

impl Shell {
    /// Contributed items at `location` (see [`contributions`]).
    #[must_use]
    pub fn contributed(&self, location: MenuLocation, module: Option<&str>) -> Vec<Contribution> {
        contributions(
            &plugin_bridge::menu_items(&self.vm.services().plugins),
            location,
            module,
        )
    }

    /// Push the contributed quick actions to Home pages when the menu registry changed.
    pub(super) fn sync_contributions(&mut self) {
        let rev = plugin_bridge::menu_revision(&self.vm.services().plugins);
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
        self.vm
            .tabs
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
        menu_tree(&plugin_bridge::menu_items(&self.vm.services().plugins))
    }

    /// Whether a plugin menu command is currently enabled (shell commands follow
    /// [`Shell::is_enabled`]; plugin-only commands are enabled when registered).
    #[must_use]
    pub fn plugin_command_enabled(&self, name: &str) -> bool {
        plugin_bridge::command_enabled(&self.vm.services().plugins, name, |cmd| {
            self.is_enabled(cmd)
        })
    }

    /// Run a command from a plugin menu (C# `CommandService.ExecuteAsync`), then apply
    /// whatever it queued.
    pub fn run_plugin_command(&mut self, name: &str) {
        match plugin_bridge::run_command(&self.vm.services().plugins, name) {
            CommandOutcome::Plugin(None) => {}
            CommandOutcome::Plugin(Some(n)) | CommandOutcome::Developing(n) => {
                self.dispatch(ShellInput::Notify(n));
            }
            CommandOutcome::Shell(cmd) => self.execute(cmd),
        }
        self.pump_plugins();
    }

    /// Apply navigation, shell commands and window requests plugins queued.
    pub fn pump_plugins(&mut self) {
        for action in plugin_bridge::drain(&self.vm.services().plugins) {
            self.apply_plugin(action);
        }
    }

    fn apply_plugin(&mut self, action: ShellAction) {
        match action {
            ShellAction::Navigate(req) => {
                self.navigate(&req, None);
            }
            ShellAction::Notify(n) => self.dispatch(ShellInput::Notify(n)),
            ShellAction::Back => self.on_focused(|s, t| {
                s.back(t);
            }),
            ShellAction::Close => self.on_focused(|s, t| {
                s.close_tab(t, false);
            }),
            ShellAction::Execute(cmd) => self.execute(cmd),
            ShellAction::Confirm(c) => self.dispatch(ShellInput::Confirm(c.into())),
            ShellAction::Status { id, text } => self.dispatch(ShellInput::Status { id, text }),
        }
    }

    fn on_focused(&mut self, f: impl FnOnce(&mut Self, super::TabId)) {
        if let Some(t) = self.focused_tab() {
            f(self, t);
        }
    }
}
