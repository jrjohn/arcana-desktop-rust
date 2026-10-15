//! Bridge between the plugin host and the shell: plugin menus, plugin commands, and the
//! navigation / shell-command / window requests plugins queue.

use arcana_plugin_api::{
    MenuItemDefinition, MenuLocation, MessageLevel, WindowRequest, WindowService as _,
};
use arcana_plugin_flowchart::plugin::EditorArgs;
use arcana_plugin_host::services::nav::NavRequest as PluginNav;

use super::Shell;
use crate::commands::{CommandId, CommandRegistry};
use crate::effect::{Notice, NoticeLevel, Text};
use crate::nav::{FlowMode, NavRequest, Route, routes};

/// A main-menu entry with its children, sorted by `order` (C# `MenuRegistry` tree).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuNode {
    pub item: MenuItemDefinition,
    pub children: Vec<MenuNode>,
}

fn children_of(all: &[MenuItemDefinition], parent: &str, depth: usize) -> Vec<MenuNode> {
    if depth > 8 {
        return Vec::new();
    }
    let mut items: Vec<&MenuItemDefinition> = all
        .iter()
        .filter(|m| m.parent_id.as_deref() == Some(parent))
        .collect();
    items.sort_by_key(|m| m.order);
    items
        .into_iter()
        .map(|m| MenuNode {
            item: m.clone(),
            children: children_of(all, &m.id, depth + 1),
        })
        .collect()
}

/// Build the main menu tree from the registry: roots are `MainMenu` items without a parent.
#[must_use]
pub fn menu_tree(all: &[MenuItemDefinition]) -> Vec<MenuNode> {
    let mut roots: Vec<&MenuItemDefinition> = all
        .iter()
        .filter(|m| m.location == MenuLocation::MainMenu && m.parent_id.is_none())
        .collect();
    roots.sort_by_key(|m| m.order);
    roots
        .into_iter()
        .map(|m| MenuNode {
            item: m.clone(),
            children: children_of(all, &m.id, 0),
        })
        .collect()
}

fn flow_mode(parameter: Option<&Box<dyn std::any::Any + Send>>) -> FlowMode {
    match parameter.and_then(|p| p.downcast_ref::<EditorArgs>()) {
        Some(EditorArgs::Sample) => FlowMode::Sample,
        Some(EditorArgs::OpenDialog) => FlowMode::OpenDialog,
        Some(EditorArgs::Open(p)) => FlowMode::Open(p.clone()),
        Some(EditorArgs::New) | None => FlowMode::New,
    }
}

fn route_for(route_id: &str, parameter: Option<&Box<dyn std::any::Any + Send>>) -> Option<Route> {
    if route_id == routes::FLOWCHART {
        return Some(Route::FlowChart {
            mode: flow_mode(parameter),
        });
    }
    Route::from_id(route_id)
}

/// Re-run every active plugin's contributions so menu / view titles follow a language switch
/// (C# `CoreMenuPlugin.OnCultureChanged` rebuilds its menus; here every plugin does).
pub fn relocalize(rt: &arcana_infra::plugin_runtime::PluginRuntime) {
    for p in rt.host.all_plugins() {
        if p.state == arcana_plugin_api::PluginState::Active {
            let r = rt.host.reload(&p.id);
            if !r.success {
                tracing::warn!(plugin = %p.id, message = ?r.message, "relocalize");
            }
        }
    }
}

/// A contributed entry for a flat list (side navigation, quick actions).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contribution {
    pub id: String,
    pub title: String,
    pub icon: Option<String>,
    pub command: Option<String>,
    /// A separator goes before this entry (its group differs from the previous one's).
    pub separator_before: bool,
}

/// Items at `location` (optionally of one module), sorted by `order`, with group separators
/// (C# `BuildQuickActionsMenu` / `OrderModulePage` flyouts).
#[must_use]
pub fn contributions(
    all: &[MenuItemDefinition],
    location: MenuLocation,
    module: Option<&str>,
) -> Vec<Contribution> {
    let mut items: Vec<&MenuItemDefinition> = all
        .iter()
        .filter(|m| m.location == location && !m.is_separator)
        .filter(|m| module.is_none_or(|id| m.module_id.as_deref() == Some(id)))
        .collect();
    items.sort_by_key(|m| m.order);
    let mut last_group: Option<&str> = None;
    items
        .into_iter()
        .map(|m| {
            let g = m.group.as_deref().filter(|g| !g.is_empty());
            let separator_before = g.is_some() && last_group.is_some() && g != last_group;
            if g.is_some() {
                last_group = g;
            }
            Contribution {
                id: m.id.clone(),
                title: m.title.clone(),
                icon: m.icon.clone(),
                command: m.command.clone(),
                separator_before,
            }
        })
        .collect()
}

/// `FunctionTree` items not already shown by a built-in navigation entry (C#
/// `BuildFunctionTree` skips items whose `Command ?? Id` matches an existing tag).
#[must_use]
pub fn function_tree(all: &[MenuItemDefinition], existing: &[&str]) -> Vec<Contribution> {
    contributions(all, MenuLocation::FunctionTree, None)
        .into_iter()
        .filter(|c| !existing.contains(&c.command.as_deref().unwrap_or(&c.id)))
        .map(|c| Contribution {
            separator_before: false,
            ..c
        })
        .collect()
}

/// The plugin module a route belongs to (C# module pages host `ModuleQuickAccess` flyouts).
#[must_use]
pub fn module_of(route: &Route) -> Option<&'static str> {
    match route {
        Route::OrderList | Route::OrderDetail { .. } => Some("OrderModule"),
        Route::ProductList | Route::ProductDetail { .. } => Some("ProductModule"),
        Route::CustomerList | Route::CustomerDetail { .. } => Some("CustomerModule"),
        _ => None,
    }
}

/// Segoe MDL2 code points used by the C# contributions → glyphs egui's fonts can draw.
#[must_use]
pub fn icon_glyph(icon: Option<&str>) -> &'static str {
    match icon.and_then(|s| s.chars().next()).map(u32::from) {
        Some(0xE80F) => "🏠",
        Some(0xE7C3 | 0xE8A5) => "📄",
        Some(0xE716) => "👤",
        Some(0xE719) => "📦",
        Some(0xE8FD) => "🔀",
        Some(0xE710) => "➕",
        Some(0xE8E5) => "📂",
        Some(0xE8F1) => "📋",
        _ => "•",
    }
}

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
        let quick: Vec<crate::pages::home::QuickAction> = self
            .contributed(MenuLocation::QuickAccess, None)
            .into_iter()
            .filter_map(|c| {
                Some(crate::pages::home::QuickAction {
                    icon: icon_glyph(c.icon.as_deref()).to_owned(),
                    title: c.title,
                    command: c.command?,
                })
            })
            .collect();
        let homes: Vec<super::TabId> = self
            .tabs
            .iter()
            .filter(|(_, t)| matches!(t.page, crate::pages::Page::Home(_)))
            .map(|(id, _)| *id)
            .collect();
        for id in homes {
            self.send(
                id,
                crate::pages::PageMsg::Home(crate::pages::home::Input::SetQuickActions(
                    quick.clone(),
                )),
            );
        }
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
        let rt = self.svc.plugins.clone();
        for req in rt.nav.drain() {
            match req {
                PluginNav::To {
                    route_id,
                    parameter,
                }
                | PluginNav::NewTab {
                    route_id,
                    parameter,
                } => {
                    if let Some(route) = route_for(&route_id, parameter.as_ref()) {
                        self.navigate(&NavRequest::new_tab(route), None);
                    } else {
                        self.notify(Notice::new(
                            NoticeLevel::Info,
                            Text::Key("common.developing"),
                        ));
                    }
                }
                PluginNav::WithinTab {
                    route_id,
                    parameter,
                    ..
                } => {
                    if let Some(route) = route_for(&route_id, parameter.as_ref()) {
                        self.navigate(&NavRequest::within_tab(route), None);
                    }
                }
                PluginNav::Back => {
                    if let Some(t) = self.focused_tab() {
                        self.back(t);
                    }
                }
                PluginNav::Forward => {}
                PluginNav::Close => {
                    if let Some(t) = self.focused_tab() {
                        self.close_tab(t, false);
                    }
                }
            }
        }
        for name in rt.take_shell_commands() {
            if let Some(cmd) = CommandRegistry::by_name(&name) {
                self.execute(cmd);
            }
        }
        for w in rt.window.drain() {
            let notice = match w {
                WindowRequest::Confirm {
                    title,
                    message,
                    reply_command,
                } => {
                    let d = super::Dialog::PluginConfirm {
                        title,
                        message,
                        reply_command,
                    };
                    if self.dialog.is_none() {
                        self.dialog = Some(d);
                    } else {
                        self.pending_confirms.push_back(d);
                    }
                    continue;
                }
                WindowRequest::StatusBar { id, text } => {
                    match text {
                        Some(t) => {
                            self.status_items.insert(id, t);
                        }
                        None => {
                            self.status_items.remove(&id);
                        }
                    }
                    continue;
                }
                WindowRequest::Message { level, message, .. } => Notice::new(
                    match level {
                        MessageLevel::Info => NoticeLevel::Info,
                        MessageLevel::Warning => NoticeLevel::Warning,
                        MessageLevel::Error => NoticeLevel::Error,
                    },
                    Text::Raw(message),
                ),
            };
            self.notify(notice);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, parent: Option<&str>, order: i32, loc: MenuLocation) -> MenuItemDefinition {
        let mut m = MenuItemDefinition::new(id, id, loc).with_order(order);
        m.parent_id = parent.map(str::to_string);
        m
    }

    #[test]
    fn tree_sorts_roots_and_children_by_order() {
        let all = vec![
            item("menu.help", None, 99, MenuLocation::MainMenu),
            item("menu.file", None, 1, MenuLocation::MainMenu),
            item(
                "menu.file.exit",
                Some("menu.file"),
                99,
                MenuLocation::FileMenu,
            ),
            item(
                "menu.file.new",
                Some("menu.file"),
                1,
                MenuLocation::FileMenu,
            ),
            item("quick.home", None, 1, MenuLocation::QuickAccess),
        ];
        let t = menu_tree(&all);
        assert_eq!(t.len(), 2);
        assert_eq!(t[0].item.id, "menu.file");
        assert_eq!(t[0].children[0].item.id, "menu.file.new");
        assert_eq!(t[1].item.id, "menu.help");
    }

    #[test]
    fn contributions_sort_filter_and_separate_groups() {
        let mut a = item("a", None, 2, MenuLocation::QuickAccess);
        a.group = Some("business".into());
        let mut b = item("b", None, 1, MenuLocation::QuickAccess);
        b.group = Some("navigation".into());
        let mut m = item("m", None, 1, MenuLocation::ModuleQuickAccess);
        m.module_id = Some("OrderModule".into());
        let all = vec![a, b, m, item("t", None, 1, MenuLocation::FunctionTree)];
        let q = contributions(&all, MenuLocation::QuickAccess, None);
        assert_eq!(
            q.iter().map(|c| c.id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert!(!q[0].separator_before && q[1].separator_before);
        assert_eq!(
            contributions(&all, MenuLocation::ModuleQuickAccess, Some("OrderModule")).len(),
            1
        );
        assert!(contributions(&all, MenuLocation::ModuleQuickAccess, Some("X")).is_empty());
        assert_eq!(function_tree(&all, &[]).len(), 1);
        assert!(function_tree(&all, &["t"]).is_empty());
        assert_eq!(icon_glyph(Some("\u{E8FD}")), "🔀");
        assert_eq!(icon_glyph(None), "•");
        assert_eq!(module_of(&Route::OrderList), Some("OrderModule"));
        assert_eq!(module_of(&Route::Home), None);
    }

    #[test]
    fn flowchart_route_carries_editor_args() {
        let p: Box<dyn std::any::Any + Send> = Box::new(EditorArgs::Sample);
        assert_eq!(
            route_for(routes::FLOWCHART, Some(&p)),
            Some(Route::FlowChart {
                mode: FlowMode::Sample
            })
        );
        assert_eq!(route_for("SettingsPage", None), Some(Route::Settings));
        assert_eq!(route_for("ProductCategoriesPage", None), None);
    }
}
