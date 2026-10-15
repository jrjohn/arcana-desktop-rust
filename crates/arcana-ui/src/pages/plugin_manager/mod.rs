//! Plugin manager (C# `PluginManagerViewModel` + `PluginManagerPage`), backed by the
//! `arcana-plugin-host` runtime.
//!
//! Lists everything the host knows (built-ins, `FlowChart`, installed third-party plugins),
//! and runs install / activate / deactivate / reload / upgrade / rollback / uninstall /
//! approve-permissions / health checks as async tasks against the host. A plugin whose
//! manifest declares `contributes.configuration` gets a generated settings form
//! (`arcana_plugin_api::schema::settings_form`), saved under `plugin.<id>.<key>`.

mod task;
mod view;

pub use task::run;
pub use view::show;

use arcana_plugin_api::schema::SettingField;
use arcana_plugin_api::{HealthState, PluginState};

use crate::effect::{Effect, Notice, NoticeLevel, Text};

/// One plugin row (C# `PluginItemViewModel`).
#[derive(Debug, Clone, PartialEq)]
pub struct PluginRow {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    /// C# `PluginType` name.
    pub kind: String,
    pub author: Option<String>,
    pub state: PluginState,
    pub health: HealthState,
    pub error_count: i32,
    pub built_in: bool,
    /// Requested but not yet approved permissions (C# flag names), empty when none.
    pub pending_permissions: String,
    /// Granted permissions (C# flag names).
    pub granted_permissions: String,
    /// Generated settings form fields with their current values.
    pub settings: Vec<(SettingField, serde_json::Value)>,
    /// Declarative UI a WASM plugin describes (`UiNode` JSON, `type`-tagged); host-drawn.
    pub ui_tree: Option<serde_json::Value>,
}

/// A host operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Op {
    Activate(String),
    Deactivate(String),
    Reload(String),
    Install(String),
    Upgrade(String, String),
    Rollback(String, String),
    Uninstall(String),
    Approve(String),
    CheckHealth(String),
    CheckAllHealth,
    /// A button in a plugin's declarative UI (`commands.execute(command)`).
    Command(String),
}

/// What the buttons ask for; the VM turns it into an [`Op`] with the selection and inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeAction {
    Install,
    Activate,
    Deactivate,
    Reload,
    Upgrade,
    Uninstall,
    Rollback,
    Approve,
}

impl RuntimeAction {
    pub const ALL: [Self; 8] = [
        Self::Install,
        Self::Activate,
        Self::Deactivate,
        Self::Reload,
        Self::Upgrade,
        Self::Uninstall,
        Self::Rollback,
        Self::Approve,
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
            Self::Approve => "ui.plugins.approve",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct State {
    pub plugins: Vec<PluginRow>,
    pub search: String,
    pub selected: Option<String>,
    /// Package directory for install / upgrade.
    pub package_path: String,
    /// Versions of the selected plugin, newest first (C# `GetAvailableVersions`).
    pub versions: Vec<String>,
    pub rollback_to: Option<String>,
    /// Operations in flight.
    pub busy: usize,
}

impl State {
    /// Rows matching the search box (name, id or description).
    #[must_use]
    pub fn visible(&self) -> Vec<&PluginRow> {
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
    pub fn selected_plugin(&self) -> Option<&PluginRow> {
        self.selected
            .as_ref()
            .and_then(|id| self.plugins.iter().find(|p| &p.id == id))
    }

    /// Whether `action` makes sense for the current selection / inputs.
    #[must_use]
    pub fn can(&self, action: RuntimeAction) -> bool {
        let p = self.selected_plugin();
        let path = !self.package_path.trim().is_empty();
        match action {
            RuntimeAction::Install => path,
            RuntimeAction::Activate => p.is_some_and(|p| p.state != PluginState::Active),
            RuntimeAction::Deactivate => p.is_some_and(|p| p.state == PluginState::Active),
            RuntimeAction::Reload => p.is_some(),
            RuntimeAction::Upgrade => path && p.is_some_and(|p| !p.built_in),
            RuntimeAction::Uninstall => p.is_some_and(|p| !p.built_in),
            RuntimeAction::Rollback => self.rollback_to.is_some() && p.is_some_and(|p| !p.built_in),
            RuntimeAction::Approve => p.is_some_and(|p| !p.pending_permissions.is_empty()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Input {
    Load,
    Loaded(Vec<PluginRow>),
    SearchChanged(String),
    Select(String),
    Versions(Vec<String>),
    PackagePathChanged(String),
    /// Open the native folder picker for the package path (the text box stays as fallback
    /// where no dialog / portal exists).
    BrowsePackage,
    /// Picker result; `None` = cancelled / no dialog available (typed path is kept).
    PackagePicked(Option<String>),
    RollbackTarget(String),
    Refresh,
    CheckHealth(String),
    CheckAllHealth,
    Runtime(RuntimeAction),
    /// Result of an operation: `(success, message)`.
    Done(bool, String),
    SettingChanged(String, serde_json::Value),
    PluginCommand(String),
}

/// Async work against the plugin host.
#[derive(Debug, Clone, PartialEq)]
pub enum Task {
    Load,
    /// Native folder picker (`rfd`) for a plugin package directory.
    PickPackage,
    Versions(String),
    Op(Op),
    SaveSetting {
        plugin_id: String,
        key: String,
        value: serde_json::Value,
    },
}

#[derive(Debug, Default)]
pub struct ViewModel {
    pub state: State,
}

impl ViewModel {
    /// The page loads the plugin list as it opens.
    #[must_use]
    pub fn new() -> (Self, Vec<Effect<Task>>) {
        (Self::default(), vec![Effect::Run(Task::Load)])
    }

    fn op(&mut self, op: Op) -> Vec<Effect<Task>> {
        self.state.busy += 1;
        vec![Effect::Run(Task::Op(op))]
    }

    fn action(&mut self, a: RuntimeAction) -> Vec<Effect<Task>> {
        if !self.state.can(a) {
            return vec![];
        }
        let s = &self.state;
        let id = s.selected.clone().unwrap_or_default();
        let path = s.package_path.trim().to_string();
        let op = match a {
            RuntimeAction::Install => Op::Install(path),
            RuntimeAction::Activate => Op::Activate(id),
            RuntimeAction::Deactivate => Op::Deactivate(id),
            RuntimeAction::Reload => Op::Reload(id),
            RuntimeAction::Upgrade => Op::Upgrade(id, path),
            RuntimeAction::Uninstall => Op::Uninstall(id),
            RuntimeAction::Rollback => Op::Rollback(id, s.rollback_to.clone().unwrap_or_default()),
            RuntimeAction::Approve => Op::Approve(id),
        };
        self.op(op)
    }

    pub fn on(&mut self, input: Input) -> Vec<Effect<Task>> {
        let s = &mut self.state;
        match input {
            Input::Load | Input::Refresh => return vec![Effect::Run(Task::Load)],
            Input::Loaded(rows) => {
                s.plugins = rows;
                if s.selected_plugin().is_none() {
                    s.selected = None;
                    s.versions.clear();
                    s.rollback_to = None;
                }
                if let Some(id) = s.selected.clone() {
                    return vec![Effect::Run(Task::Versions(id))];
                }
            }
            Input::SearchChanged(q) => s.search = q,
            Input::Select(id) => {
                s.selected = Some(id.clone());
                s.versions.clear();
                s.rollback_to = None;
                return vec![Effect::Run(Task::Versions(id))];
            }
            Input::Versions(v) => {
                s.rollback_to = s.rollback_to.take().filter(|r| v.contains(r));
                s.versions = v;
            }
            Input::PackagePathChanged(p) => s.package_path = p,
            Input::BrowsePackage => return vec![Effect::Run(Task::PickPackage)],
            Input::PackagePicked(p) => {
                if let Some(p) = p {
                    s.package_path = p;
                }
            }
            Input::RollbackTarget(v) => s.rollback_to = Some(v),
            Input::CheckHealth(id) => return self.op(Op::CheckHealth(id)),
            Input::CheckAllHealth => return self.op(Op::CheckAllHealth),
            Input::Runtime(a) => return self.action(a),
            Input::PluginCommand(c) => return self.op(Op::Command(c)),
            Input::Done(ok, message) => {
                s.busy = s.busy.saturating_sub(1);
                let level = if ok {
                    NoticeLevel::Success
                } else {
                    NoticeLevel::Error
                };
                return vec![
                    Effect::Notify(Notice::new(level, Text::Raw(message))),
                    Effect::Run(Task::Load),
                ];
            }
            Input::SettingChanged(key, value) => {
                let Some(id) = s.selected.clone() else {
                    return vec![];
                };
                let Some(row) = s.plugins.iter_mut().find(|p| p.id == id) else {
                    return vec![];
                };
                let Some(slot) = row.settings.iter_mut().find(|(f, _)| f.key == key) else {
                    return vec![];
                };
                if let Err(e) = slot.0.validate(&value) {
                    return vec![Effect::Notify(Notice::error(e.message().to_string()))];
                }
                slot.1 = value.clone();
                return vec![Effect::Run(Task::SaveSetting {
                    plugin_id: id,
                    key,
                    value,
                })];
            }
        }
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana_plugin_api::schema::SettingKind;

    fn row(id: &str, state: PluginState, built_in: bool) -> PluginRow {
        PluginRow {
            id: id.into(),
            name: id.to_uppercase(),
            version: "1.0.0".into(),
            description: format!("{id} module"),
            kind: "Module".into(),
            author: None,
            state,
            health: HealthState::Unknown,
            error_count: 0,
            built_in,
            pending_permissions: String::new(),
            granted_permissions: String::new(),
            settings: vec![],
            ui_tree: None,
        }
    }

    fn loaded() -> ViewModel {
        let (mut vm, fx) = ViewModel::new();
        assert_eq!(fx, vec![Effect::Run(Task::Load)]);
        let _ = vm.on(Input::Loaded(vec![
            row("arcana.module.order", PluginState::Active, true),
            row("ext.sample", PluginState::Loaded, false),
        ]));
        vm
    }

    #[test]
    fn search_select_requests_versions() {
        let mut vm = loaded();
        let _ = vm.on(Input::SearchChanged("ORDER".into()));
        assert_eq!(vm.state.visible().len(), 1);
        let fx = vm.on(Input::Select("ext.sample".into()));
        assert_eq!(fx, vec![Effect::Run(Task::Versions("ext.sample".into()))]);
        let _ = vm.on(Input::Versions(vec!["2.0.0".into(), "1.0.0".into()]));
        assert_eq!(vm.state.versions.len(), 2);
    }

    #[test]
    fn actions_respect_state_and_built_in() {
        let mut vm = loaded();
        let _ = vm.on(Input::Select("arcana.module.order".into()));
        assert!(vm.state.can(RuntimeAction::Deactivate));
        assert!(!vm.state.can(RuntimeAction::Activate));
        assert!(!vm.state.can(RuntimeAction::Uninstall));
        assert!(vm.on(Input::Runtime(RuntimeAction::Uninstall)).is_empty());
        let fx = vm.on(Input::Runtime(RuntimeAction::Deactivate));
        assert_eq!(
            fx,
            vec![Effect::Run(Task::Op(Op::Deactivate(
                "arcana.module.order".into()
            )))]
        );
        assert_eq!(vm.state.busy, 1);
        let fx = vm.on(Input::Done(true, "ok".into()));
        assert_eq!(vm.state.busy, 0);
        assert!(matches!(&fx[0], Effect::Notify(n) if n.level == NoticeLevel::Success));
        assert_eq!(fx[1], Effect::Run(Task::Load));
    }

    #[test]
    fn install_upgrade_rollback_need_inputs() {
        let mut vm = loaded();
        assert!(!vm.state.can(RuntimeAction::Install));
        assert_eq!(
            vm.on(Input::BrowsePackage),
            vec![Effect::Run(Task::PickPackage)]
        );
        let _ = vm.on(Input::PackagePicked(Some("/picked".into())));
        assert_eq!(vm.state.package_path, "/picked");
        let _ = vm.on(Input::PackagePicked(None));
        assert_eq!(vm.state.package_path, "/picked");
        let _ = vm.on(Input::PackagePathChanged("/pkg".into()));
        assert_eq!(
            vm.on(Input::Runtime(RuntimeAction::Install)),
            vec![Effect::Run(Task::Op(Op::Install("/pkg".into())))]
        );
        let _ = vm.on(Input::Select("ext.sample".into()));
        assert!(vm.state.can(RuntimeAction::Upgrade));
        assert!(!vm.state.can(RuntimeAction::Rollback));
        let _ = vm.on(Input::Versions(vec!["1.0.0".into()]));
        let _ = vm.on(Input::RollbackTarget("1.0.0".into()));
        assert_eq!(
            vm.on(Input::Runtime(RuntimeAction::Rollback)),
            vec![Effect::Run(Task::Op(Op::Rollback(
                "ext.sample".into(),
                "1.0.0".into()
            )))]
        );
    }

    #[test]
    fn settings_are_validated_then_saved() {
        let mut vm = loaded();
        vm.state.plugins[1].settings = vec![(
            SettingField {
                key: "size".into(),
                kind: SettingKind::Integer {
                    min: Some(1.0),
                    max: Some(10.0),
                },
                default: serde_json::Value::from(5),
                description: None,
                description_key: None,
            },
            serde_json::Value::from(5),
        )];
        let _ = vm.on(Input::Select("ext.sample".into()));
        let bad = vm.on(Input::SettingChanged(
            "size".into(),
            serde_json::Value::from(99),
        ));
        assert!(matches!(&bad[0], Effect::Notify(n) if n.level == NoticeLevel::Error));
        let ok = vm.on(Input::SettingChanged(
            "size".into(),
            serde_json::Value::from(7),
        ));
        assert!(matches!(&ok[0], Effect::Run(Task::SaveSetting { .. })));
        assert_eq!(
            vm.state.plugins[1].settings[0].1,
            serde_json::Value::from(7)
        );
    }

    #[test]
    fn approve_only_with_pending_permissions() {
        let mut vm = loaded();
        let _ = vm.on(Input::Select("ext.sample".into()));
        assert!(!vm.state.can(RuntimeAction::Approve));
        vm.state.plugins[1].pending_permissions = "NetworkAccess".into();
        assert!(vm.state.can(RuntimeAction::Approve));
        for a in RuntimeAction::ALL {
            assert!(a.label_key().starts_with("ui.plugins."));
        }
    }
}
