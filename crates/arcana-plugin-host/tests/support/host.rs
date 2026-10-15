//! Shared fixtures: real host services plus an in-memory localization fake (the real
//! `LocalizationServiceImpl` lives in `arcana-infra`, which the host must not depend on).
#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use arcana_plugin_api::{
    CultureChanged, CultureChangedHandler, HostServices, LocalizationService, PluginContext,
    PluginPermission, format_template,
};
use arcana_plugin_host::services::command::CommandServiceImpl;
use arcana_plugin_host::services::event_aggregator::EventAggregatorImpl;
use arcana_plugin_host::services::menu::MenuRegistryImpl;
use arcana_plugin_host::services::message_bus::MessageBusImpl;
use arcana_plugin_host::services::nav::QueuedNavGraph;
use arcana_plugin_host::services::shared_state::SharedStateStoreImpl;
use arcana_plugin_host::services::view::ViewRegistryImpl;
use arcana_plugin_host::services::window::QueuedWindowService;
use arcana_plugin_host::version_store::InMemoryPluginVersionStore;
use arcana_plugin_host::{HostPaths, PluginHost};

type Res = BTreeMap<String, BTreeMap<String, HashMap<String, String>>>;

/// culture -> plugin -> key -> text, with en-US fallback.
#[derive(Default)]
pub struct FakeL10n {
    culture: Mutex<String>,
    res: Mutex<Res>,
    /// Every `register_plugin_resources` call: (plugin, culture, count).
    pub calls: Mutex<Vec<(String, String, usize)>>,
    handlers: Mutex<Vec<CultureChangedHandler>>,
}

impl FakeL10n {
    pub fn new() -> Arc<Self> {
        let l = Self::default();
        *l.culture.lock().unwrap_or_else(PoisonError::into_inner) = "en-US".to_string();
        Arc::new(l)
    }
    pub fn calls(&self) -> Vec<(String, String, usize)> {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    fn lookup(&self, plugin: Option<&str>, key: &str) -> Option<String> {
        let res = self.res.lock().unwrap_or_else(PoisonError::into_inner);
        let cur = self.current_culture();
        [cur.as_str(), "en-US"].iter().find_map(|c| {
            let by_plugin = res.get(*c)?;
            match plugin {
                Some(p) => by_plugin.get(p)?.get(key).cloned(),
                None => by_plugin.values().find_map(|m| m.get(key).cloned()),
            }
        })
    }
}

impl LocalizationService for FakeL10n {
    fn current_culture(&self) -> String {
        self.culture
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
    fn available_cultures(&self) -> Vec<String> {
        vec!["en-US".into(), "zh-TW".into(), "ja-JP".into()]
    }
    fn get(&self, key: &str) -> String {
        self.lookup(None, key).unwrap_or_else(|| key.to_string())
    }
    fn get_with(&self, key: &str, args: &[&dyn Display]) -> String {
        let t = self.get(key);
        format_template(&t, args).unwrap_or(t)
    }
    fn get_for_plugin(&self, plugin_id: &str, key: &str) -> String {
        self.lookup(Some(plugin_id), key)
            .unwrap_or_else(|| key.to_string())
    }
    fn get_for_plugin_with(&self, plugin_id: &str, key: &str, args: &[&dyn Display]) -> String {
        let t = self.get_for_plugin(plugin_id, key);
        format_template(&t, args).unwrap_or(t)
    }
    fn set_culture(&self, culture_name: &str) {
        let old = self.current_culture();
        *self.culture.lock().unwrap_or_else(PoisonError::into_inner) = culture_name.to_string();
        let e = CultureChanged {
            old_culture: old,
            new_culture: culture_name.to_string(),
        };
        for h in self
            .handlers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
        {
            h(&e);
        }
    }
    fn register_plugin_resources(
        &self,
        plugin_id: &str,
        culture_name: &str,
        resources: HashMap<String, String>,
    ) {
        self.calls
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((plugin_id.into(), culture_name.into(), resources.len()));
        self.res
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(culture_name.to_string())
            .or_default()
            .entry(plugin_id.to_string())
            .or_default()
            .extend(resources);
    }
    fn get_from_any_plugin(&self, key: &str) -> String {
        self.get(key)
    }
    fn on_culture_changed(&self, handler: CultureChangedHandler) {
        self.handlers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(handler);
    }
}

pub struct Fixture {
    pub services: HostServices,
    pub l10n: Arc<FakeL10n>,
    pub commands: Arc<CommandServiceImpl>,
    pub menus: Arc<MenuRegistryImpl>,
    pub views: Arc<ViewRegistryImpl>,
    pub bus: Arc<MessageBusImpl>,
    pub events: Arc<EventAggregatorImpl>,
    pub nav: Arc<QueuedNavGraph>,
}

pub fn fixture() -> Fixture {
    let l10n = FakeL10n::new();
    let commands = Arc::new(CommandServiceImpl::new());
    let menus = Arc::new(MenuRegistryImpl::new());
    let views = Arc::new(ViewRegistryImpl::new());
    let bus = Arc::new(MessageBusImpl::new());
    let events = Arc::new(EventAggregatorImpl::new());
    let nav = Arc::new(QueuedNavGraph::new(Vec::<String>::new()));
    let services = HostServices {
        commands: commands.clone(),
        window: Arc::new(QueuedWindowService::new()),
        message_bus: bus.clone(),
        events: events.clone(),
        shared_state: Arc::new(SharedStateStoreImpl::new()),
        menus: menus.clone(),
        views: views.clone(),
        nav_graph: nav.clone(),
        localization: l10n.clone(),
    };
    Fixture {
        services,
        l10n,
        commands,
        menus,
        views,
        bus,
        events,
        nav,
    }
}

impl Fixture {
    pub fn context(&self, id: &str, path: &Path) -> PluginContext {
        PluginContext::new(
            id,
            path.to_path_buf(),
            path.join("data"),
            PluginPermission::FULL_ACCESS,
            &self.services,
        )
    }

    pub fn host(&self, root: &Path) -> Arc<PluginHost> {
        PluginHost::new(
            self.services.clone(),
            HostPaths::under(root),
            Arc::new(InMemoryPluginVersionStore::new()),
        )
    }
}

pub fn write(path: &Path, text: &str) {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).unwrap_or_else(|e| panic!("mkdir {}: {e}", p.display()));
    }
    std::fs::write(path, text).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
}

pub fn tmp() -> tempfile::TempDir {
    tempfile::tempdir().unwrap_or_else(|e| panic!("tempdir: {e}"))
}

pub fn pb(p: &Path, s: &str) -> PathBuf {
    p.join(s)
}
