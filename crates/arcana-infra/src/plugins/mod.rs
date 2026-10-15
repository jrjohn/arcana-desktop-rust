//! The built-in plugins C# registers in `App.InitializePluginsAsync` (`CoreMenu`, System,
//! Order, Product, Customer), now real `arcana-plugin-host` plugins registered at compile
//! time with `inventory`.
//!
//! Each one is a table: locale files, menu items, views and commands copied from the C#
//! `RegisterContributions`. A command either navigates (through the plugin's `NavGraph`) or
//! asks the shell to run one of its own commands by publishing [`ShellCommand`] on the
//! message bus (C# calls `Application.Current.Exit()` etc. directly; plugins here cannot see
//! the shell).

mod tables;

use arcana_core::AppResult;
use arcana_plugin_api::{
    MenuItemDefinition, MessageBusExt, PluginMetadata, PluginType, ViewDefinition,
};
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBase, PluginBehavior};
use arcana_plugin_host::submit_builtin;

pub use tables::{BUILTIN_IDS, CORE_MENU, CUSTOMER, ORDER, PRODUCT, SYSTEM};

/// Message a built-in plugin publishes to run a shell command (`app.exit`, `edit.undo`, …).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommand(pub String);

/// What a plugin command does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// C# `Navigation.NavigateToAsync(route)`.
    To(&'static str),
    /// C# `Navigation.NavigateToNewTabAsync(route)`.
    NewTab(&'static str),
    /// C# `Navigation.NavigateWithinTabAsync(parent, route)`.
    Within(&'static str, &'static str),
    /// Forward to the shell.
    Shell,
    /// C# placeholder handler (`() => Task.CompletedTask`).
    Nothing,
}

/// One C# menu item; `title_key` is looked up with the plugin's `L()`.
#[derive(Debug, Clone, Copy)]
pub struct Menu {
    pub id: &'static str,
    pub title_key: &'static str,
    pub location: arcana_plugin_api::MenuLocation,
    pub parent: Option<&'static str>,
    pub icon: Option<&'static str>,
    pub shortcut: Option<&'static str>,
    pub order: i32,
    pub command: Option<&'static str>,
    pub group: Option<&'static str>,
    pub module: Option<&'static str>,
    pub separator: bool,
}

/// One C# view.
#[derive(Debug, Clone, Copy)]
pub struct View {
    pub id: &'static str,
    pub title_key: &'static str,
    pub icon: &'static str,
    pub category_key: &'static str,
    pub module: Option<&'static str>,
    pub default_tab: bool,
    pub multiple: bool,
}

/// A built-in plugin as data.
#[derive(Debug)]
pub struct Table {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub author: Option<&'static str>,
    pub plugin_type: PluginType,
    pub locales: [(&'static str, &'static str); 3],
    pub menus: &'static [Menu],
    pub views: &'static [View],
    pub commands: &'static [(&'static str, Action)],
}

/// [`PluginBehavior`] over a [`Table`].
#[derive(Debug)]
pub struct TablePlugin(pub &'static Table);

impl TablePlugin {
    fn menu(base: &PluginBase, m: &Menu) -> MenuItemDefinition {
        let mut item = MenuItemDefinition::new(
            m.id,
            &if m.separator {
                String::new()
            } else {
                base.l(m.title_key)
            },
            m.location,
        )
        .with_order(m.order);
        item.parent_id = m.parent.map(str::to_string);
        item.icon = m.icon.map(str::to_string);
        item.shortcut = m.shortcut.map(str::to_string);
        item.command = m.command.map(str::to_string);
        item.group = m.group.map(str::to_string);
        item.module_id = m.module.map(str::to_string);
        item.is_separator = m.separator;
        item
    }

    fn view(base: &PluginBase, v: &View) -> ViewDefinition {
        ViewDefinition {
            id: v.id.to_string(),
            title: base.l(v.title_key),
            title_key: Some(v.title_key.to_string()),
            icon: Some(v.icon.to_string()),
            view_class_name: Some(v.id.to_string()),
            category: Some(base.l(v.category_key)),
            module_id: v.module.map(str::to_string),
            is_module_default_tab: v.default_tab,
            can_have_multiple_instances: v.multiple,
            ..ViewDefinition::default()
        }
    }
}

impl PluginBehavior for TablePlugin {
    fn metadata(&self) -> PluginMetadata {
        let mut m = PluginMetadata::new(
            self.0.id,
            self.0.name,
            semver::Version::new(1, 0, 0),
            self.0.plugin_type,
        );
        m.description = Some(self.0.description.to_string());
        m.author = self.0.author.map(str::to_string);
        m
    }

    fn embedded_locales(&self) -> Vec<(&'static str, &'static str)> {
        self.0.locales.to_vec()
    }

    fn register_contributions(&mut self, base: &mut PluginBase) -> AppResult<()> {
        for v in self.0.views {
            base.register_view(Self::view(base, v))?;
        }
        let menus = self.0.menus.iter().map(|m| Self::menu(base, m)).collect();
        base.register_menu_items(menus)?;
        let Some(ctx) = base.context().cloned() else {
            return Ok(());
        };
        for &(name, action) in self.0.commands {
            let ctx = ctx.clone();
            base.register_command(name, move |_| {
                match action {
                    Action::To(r) => {
                        ctx.nav_graph.to(r, None);
                    }
                    Action::NewTab(r) => {
                        ctx.nav_graph.to_new_tab(r, None);
                    }
                    Action::Within(p, r) => {
                        ctx.nav_graph.to_within_tab(p, r, None);
                    }
                    Action::Shell => ctx.message_bus.publish(&ShellCommand(name.to_string())),
                    Action::Nothing => {}
                }
                Ok(())
            })?;
        }
        tracing::info!(plugin = self.0.id, "built-in plugin activated");
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

submit_builtin!(|| BasePlugin::boxed(TablePlugin(&CORE_MENU)), 0);
submit_builtin!(|| BasePlugin::boxed(TablePlugin(&SYSTEM)), 1);
submit_builtin!(|| BasePlugin::boxed(TablePlugin(&ORDER)), 2);
submit_builtin!(|| BasePlugin::boxed(TablePlugin(&CUSTOMER)), 3);
submit_builtin!(|| BasePlugin::boxed(TablePlugin(&PRODUCT)), 4);
