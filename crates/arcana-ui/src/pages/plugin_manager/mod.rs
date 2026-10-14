//! Plugin manager (C# `PluginManagerViewModel` + `PluginManagerPage`).
//!
//! Lists the built-in plugins C# registers at startup (`App.InitializePluginsAsync`) with
//! their metadata, state and health. The plugin runtime (install, activate/deactivate,
//! reload, upgrade, rollback, uninstall, health monitor) arrives with milestone 5; those
//! actions are shown disabled with a note. `FlowChart` is an external plugin in C# and is
//! listed once the M5 host loads it.

mod view;

pub use view::show;

use crate::effect::{Effect, Notice, NoticeLevel, Text};

/// C# `PluginState` subset reachable for built-ins.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginState {
    Active,
    Loaded,
}

/// C# `PluginHealthStatus` subset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Unknown,
}

/// One plugin row (C# `PluginItemViewModel`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub version: &'static str,
    pub description: &'static str,
    /// C# `PluginType` name.
    pub kind: &'static str,
    pub author: Option<&'static str>,
    pub state: PluginState,
    pub health: Health,
    pub error_count: u32,
}

const fn builtin(
    id: &'static str,
    name: &'static str,
    description: &'static str,
    kind: &'static str,
    author: Option<&'static str>,
) -> PluginInfo {
    PluginInfo {
        id,
        name,
        version: "1.0.0",
        description,
        kind,
        author,
        state: PluginState::Active,
        health: Health::Unknown,
        error_count: 0,
    }
}

/// C# built-in plugins, in registration order.
pub const BUILT_IN: [PluginInfo; 5] = [
    builtin(
        "arcana.core.menu",
        "Core Menu",
        "Provides core application menus",
        "Menu",
        None,
    ),
    builtin(
        "arcana.system",
        "System",
        "System management and settings",
        "Module",
        Some("Arcana Team"),
    ),
    builtin(
        "arcana.module.order",
        "Order Module",
        "Order management module",
        "Module",
        None,
    ),
    builtin(
        "arcana.module.customer",
        "Customer Module",
        "Customer management module",
        "Module",
        None,
    ),
    builtin(
        "arcana.module.product",
        "Product Module",
        "Product management module",
        "Module",
        None,
    ),
];

/// Runtime-only actions (disabled until M5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAction {
    Install,
    Activate,
    Deactivate,
    Reload,
    Upgrade,
    Uninstall,
    Rollback,
}

impl RuntimeAction {
    pub const ALL: [Self; 7] = [
        Self::Install,
        Self::Activate,
        Self::Deactivate,
        Self::Reload,
        Self::Upgrade,
        Self::Uninstall,
        Self::Rollback,
    ];

    #[must_use]
    pub fn label_key(self) -> &'static str {
        match self {
            Self::Install => "ui.plugins.install",
            Self::Activate => "ui.plugins.activate",
            Self::Deactivate => "ui.plugins.deactivate",
            Self::Reload => "ui.plugins.reload",
            Self::Upgrade => "ui.plugins.upgrade",
            Self::Uninstall => "ui.plugins.uninstall",
            Self::Rollback => "ui.plugins.rollback",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
    pub plugins: Vec<PluginInfo>,
    pub search: String,
    pub selected: Option<&'static str>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            plugins: BUILT_IN.to_vec(),
            search: String::new(),
            selected: None,
        }
    }
}

impl State {
    /// Rows matching the search box (name, id or description).
    #[must_use]
    pub fn visible(&self) -> Vec<&PluginInfo> {
        let q = self.search.trim().to_lowercase();
        self.plugins
            .iter()
            .filter(|p| {
                q.is_empty()
                    || p.name.to_lowercase().contains(&q)
                    || p.id.contains(&q)
                    || p.description.to_lowercase().contains(&q)
            })
            .collect()
    }

    #[must_use]
    pub fn selected_plugin(&self) -> Option<&PluginInfo> {
        self.selected
            .and_then(|id| self.plugins.iter().find(|p| p.id == id))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    SearchChanged(String),
    Select(&'static str),
    Refresh,
    /// Built-ins are compiled in and running, so a check reports healthy.
    CheckHealth(&'static str),
    CheckAllHealth,
    Runtime(RuntimeAction),
}

/// No async work until the M5 plugin host exists.
pub type Task = std::convert::Infallible;

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

impl ViewModel {
    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        let s = &mut self.state;
        match input {
            Input::SearchChanged(q) => s.search = q,
            Input::Select(id) => s.selected = Some(id),
            Input::Refresh => {
                let selected = s.selected;
                *s = State {
                    selected,
                    ..State::default()
                };
            }
            Input::CheckHealth(id) => {
                if let Some(p) = s.plugins.iter_mut().find(|p| p.id == id) {
                    p.health = Health::Healthy;
                }
            }
            Input::CheckAllHealth => {
                for p in &mut s.plugins {
                    p.health = Health::Healthy;
                }
            }
            Input::Runtime(_) => {
                return vec![Effect::Notify(Notice::new(
                    NoticeLevel::Info,
                    Text::Key("ui.plugins.runtimeNote"),
                ))];
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_csharp_built_ins_active() {
        let vm = ViewModel::default();
        let ids: Vec<_> = vm.state.plugins.iter().map(|p| p.id).collect();
        assert_eq!(
            ids,
            [
                "arcana.core.menu",
                "arcana.system",
                "arcana.module.order",
                "arcana.module.customer",
                "arcana.module.product"
            ]
        );
        assert!(
            vm.state
                .plugins
                .iter()
                .all(|p| p.state == PluginState::Active && p.version == "1.0.0")
        );
        assert_eq!(vm.state.plugins[0].kind, "Menu");
        assert_eq!(vm.state.plugins[1].author, Some("Arcana Team"));
    }

    #[test]
    fn search_select_health_and_refresh() {
        let mut vm = ViewModel::default();
        let _ = vm.on(Input::SearchChanged("ORDER".into()));
        assert_eq!(vm.state.visible().len(), 1);
        let _ = vm.on(Input::SearchChanged("module".into()));
        assert_eq!(vm.state.visible().len(), 3);
        let _ = vm.on(Input::Select("arcana.system"));
        assert_eq!(vm.state.selected_plugin().map(|p| p.name), Some("System"));
        let _ = vm.on(Input::CheckHealth("arcana.system"));
        assert_eq!(vm.state.selected_plugin().unwrap().health, Health::Healthy);
        assert_eq!(vm.state.plugins[0].health, Health::Unknown);
        let _ = vm.on(Input::CheckAllHealth);
        assert!(vm.state.plugins.iter().all(|p| p.health == Health::Healthy));
        let _ = vm.on(Input::Refresh);
        assert!(vm.state.search.is_empty());
        assert_eq!(vm.state.selected, Some("arcana.system"));
        assert_eq!(vm.state.plugins[0].health, Health::Unknown);
    }

    #[test]
    fn runtime_actions_explain_m5() {
        let mut vm = ViewModel::default();
        for a in RuntimeAction::ALL {
            assert!(a.label_key().starts_with("ui.plugins."));
            let fx = vm.on(Input::Runtime(a));
            assert!(
                matches!(&fx[0], Effect::Notify(n) if n.text == Text::Key("ui.plugins.runtimeNote"))
            );
        }
        assert_eq!(PluginState::Loaded, PluginState::Loaded);
    }
}
