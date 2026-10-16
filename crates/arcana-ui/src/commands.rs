//! The single command registry: menus, keyboard shortcuts, toolbar buttons and the command
//! search box all resolve to a [`CommandId`] and go through `Shell::execute`.
//!
//! Ids and shortcuts follow the C# `CoreMenuPlugin` / module plugins (`file.new` = Ctrl+N,
//! `view.refresh` = F5, `app.settings` = Ctrl+, …).

use crate::nav::{NavGraph, NavRequest};

/// A key a shortcut can be bound to — UI-agnostic; the view translates it to the toolkit's key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    N,
    O,
    S,
    W,
    Y,
    Z,
    K,
    Comma,
    ArrowLeft,
    F4,
    F5,
    F11,
}

/// Modifier keys held with a shortcut. `command` is Ctrl (Cmd on macOS).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub command: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Self = Self {
        command: false,
        alt: false,
    };
    pub const COMMAND: Self = Self {
        command: true,
        alt: false,
    };
    pub const ALT: Self = Self {
        command: false,
        alt: true,
    };
}

/// A keyboard shortcut in the view model's own terms (no GUI toolkit types).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyboardShortcut {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl KeyboardShortcut {
    #[must_use]
    pub const fn new(modifiers: Modifiers, key: Key) -> Self {
        Self { modifiers, key }
    }
}

/// Every command the shell understands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CommandId {
    // File
    New,
    Open,
    Save,
    Exit,
    // Edit
    Undo,
    Redo,
    Cut,
    Copy,
    Paste,
    // Business
    OrderList,
    NewOrder,
    CustomerList,
    NewCustomer,
    ProductList,
    NewProduct,
    // View
    Refresh,
    Fullscreen,
    ToggleSidebar,
    ToggleStatusBar,
    // Navigation / documents
    Home,
    Back,
    CloseTab,
    PopOut,
    CommandSearch,
    // Tools / System / Help
    Settings,
    PluginManager,
    Docs,
    About,
}

/// Registry entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandDef {
    pub id: CommandId,
    /// C# command string (`file.new`, `view.refresh`, …).
    pub name: &'static str,
    pub label_key: &'static str,
    pub shortcut: Option<KeyboardShortcut>,
}

#[allow(clippy::unnecessary_wraps)]
const fn sc(modifiers: Modifiers, key: Key) -> Option<KeyboardShortcut> {
    Some(KeyboardShortcut::new(modifiers, key))
}

const fn cmd(
    id: CommandId,
    name: &'static str,
    label_key: &'static str,
    shortcut: Option<KeyboardShortcut>,
) -> CommandDef {
    CommandDef {
        id,
        name,
        label_key,
        shortcut,
    }
}

const NONE: Modifiers = Modifiers::NONE;
const CTRL: Modifiers = Modifiers::COMMAND;

/// All commands, in menu order.
pub const COMMANDS: &[CommandDef] = &[
    cmd(
        CommandId::New,
        "file.new",
        "menu.file.new",
        sc(CTRL, Key::N),
    ),
    cmd(
        CommandId::Open,
        "file.open",
        "menu.file.open",
        sc(CTRL, Key::O),
    ),
    cmd(
        CommandId::Save,
        "file.save",
        "menu.file.save",
        sc(CTRL, Key::S),
    ),
    cmd(
        CommandId::Exit,
        "app.exit",
        "menu.file.exit",
        sc(Modifiers::ALT, Key::F4),
    ),
    cmd(
        CommandId::Undo,
        "edit.undo",
        "menu.edit.undo",
        sc(CTRL, Key::Z),
    ),
    cmd(
        CommandId::Redo,
        "edit.redo",
        "menu.edit.redo",
        sc(CTRL, Key::Y),
    ),
    cmd(CommandId::Cut, "edit.cut", "menu.edit.cut", None),
    cmd(CommandId::Copy, "edit.copy", "menu.edit.copy", None),
    cmd(CommandId::Paste, "edit.paste", "menu.edit.paste", None),
    cmd(
        CommandId::OrderList,
        "order.list",
        "menu.business.order.list",
        None,
    ),
    cmd(
        CommandId::NewOrder,
        "order.new",
        "menu.business.order.new",
        None,
    ),
    cmd(
        CommandId::CustomerList,
        "customer.list",
        "menu.business.customer.list",
        None,
    ),
    cmd(
        CommandId::NewCustomer,
        "customer.new",
        "menu.business.customer.new",
        None,
    ),
    cmd(
        CommandId::ProductList,
        "product.list",
        "menu.business.product.list",
        None,
    ),
    cmd(
        CommandId::NewProduct,
        "product.new",
        "menu.business.product.new",
        None,
    ),
    cmd(
        CommandId::Refresh,
        "view.refresh",
        "menu.view.refresh",
        sc(NONE, Key::F5),
    ),
    cmd(
        CommandId::Fullscreen,
        "view.fullscreen",
        "menu.view.fullscreen",
        sc(NONE, Key::F11),
    ),
    cmd(
        CommandId::ToggleSidebar,
        "view.sidebar",
        "menu.view.sidebar",
        None,
    ),
    cmd(
        CommandId::ToggleStatusBar,
        "view.statusbar",
        "menu.view.statusbar",
        None,
    ),
    cmd(CommandId::Home, "quick.home", "quick.home", None),
    cmd(
        CommandId::Back,
        "nav.back",
        "ui.nav.back",
        sc(Modifiers::ALT, Key::ArrowLeft),
    ),
    cmd(
        CommandId::CloseTab,
        "doc.close",
        "common.close",
        sc(CTRL, Key::W),
    ),
    cmd(CommandId::PopOut, "doc.popOut", "common.popOut", None),
    cmd(
        CommandId::CommandSearch,
        "app.search",
        "search.placeholder",
        sc(CTRL, Key::K),
    ),
    cmd(
        CommandId::Settings,
        "app.settings",
        "menu.tools.settings",
        sc(CTRL, Key::Comma),
    ),
    cmd(
        CommandId::PluginManager,
        "system.plugins",
        "menu.system.plugins",
        None,
    ),
    cmd(CommandId::Docs, "help.docs", "menu.help.docs", None),
    cmd(CommandId::About, "app.about", "menu.help.about", None),
];

/// Lookup + shortcut matching over [`COMMANDS`].
#[derive(Debug, Clone, Copy, Default)]
pub struct CommandRegistry;

impl CommandRegistry {
    /// Definition of `id`.
    #[must_use]
    pub fn get(id: CommandId) -> &'static CommandDef {
        COMMANDS.iter().find(|c| c.id == id).unwrap_or(&COMMANDS[0])
    }

    /// By C# command string.
    #[must_use]
    pub fn by_name(name: &str) -> Option<CommandId> {
        COMMANDS.iter().find(|c| c.name == name).map(|c| c.id)
    }

    /// Commands whose (localized) label or name contains `query`, case-insensitively.
    #[must_use]
    pub fn search(query: &str, label: impl Fn(&CommandDef) -> String) -> Vec<CommandId> {
        let q = query.trim().to_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        COMMANDS
            .iter()
            .filter(|c| c.id != CommandId::CommandSearch)
            .filter(|c| label(c).to_lowercase().contains(&q) || c.name.contains(&q))
            .map(|c| c.id)
            .collect()
    }

    /// The navigation a command performs, if it is a pure navigation.
    #[must_use]
    pub fn navigation(id: CommandId) -> Option<NavRequest> {
        Some(match id {
            CommandId::OrderList => NavGraph::to_order_list(),
            CommandId::NewOrder | CommandId::New => NavGraph::to_new_order(),
            CommandId::CustomerList => NavGraph::to_customer_list(),
            CommandId::NewCustomer => NavGraph::to_new_customer(),
            CommandId::ProductList => NavGraph::to_product_list(),
            CommandId::NewProduct => NavGraph::to_new_product(),
            CommandId::Home => NavGraph::to_dashboard(),
            CommandId::Settings => NavGraph::to_settings(),
            CommandId::PluginManager => NavGraph::to_plugin_manager(),
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn ids_names_and_shortcuts_are_unique() {
        let ids: HashSet<_> = COMMANDS.iter().map(|c| c.id).collect();
        let names: HashSet<_> = COMMANDS.iter().map(|c| c.name).collect();
        let shortcuts: Vec<_> = COMMANDS.iter().filter_map(|c| c.shortcut).collect();
        let unique: HashSet<_> = shortcuts.iter().copied().collect();
        assert_eq!(ids.len(), COMMANDS.len());
        assert_eq!(names.len(), COMMANDS.len());
        assert_eq!(unique.len(), shortcuts.len());
    }

    #[test]
    fn csharp_shortcuts_are_kept() {
        let s = |id| CommandRegistry::get(id).shortcut.unwrap();
        assert_eq!(s(CommandId::New), KeyboardShortcut::new(CTRL, Key::N));
        assert_eq!(s(CommandId::Save), KeyboardShortcut::new(CTRL, Key::S));
        assert_eq!(s(CommandId::Refresh), KeyboardShortcut::new(NONE, Key::F5));
        assert_eq!(
            s(CommandId::Settings),
            KeyboardShortcut::new(CTRL, Key::Comma)
        );
        assert_eq!(
            CommandRegistry::by_name("view.refresh"),
            Some(CommandId::Refresh)
        );
        assert_eq!(CommandRegistry::by_name("nope"), None);
    }

    #[test]
    fn navigation_and_search() {
        assert_eq!(
            CommandRegistry::navigation(CommandId::OrderList),
            Some(NavGraph::to_order_list())
        );
        assert_eq!(CommandRegistry::navigation(CommandId::Save), None);
        for c in COMMANDS {
            let _ = CommandRegistry::navigation(c.id);
        }
        let hits = CommandRegistry::search("order", |c| c.label_key.to_owned());
        assert!(hits.contains(&CommandId::OrderList));
        assert!(hits.contains(&CommandId::NewOrder));
        assert!(CommandRegistry::search("  ", |_| String::new()).is_empty());
        assert!(
            !CommandRegistry::search("search", |c| c.label_key.to_owned())
                .contains(&CommandId::CommandSearch)
        );
    }
}
