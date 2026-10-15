//! `IPluginContext` — the services handed to [`crate::Plugin::activate`].

use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use crate::permission::PluginPermission;
use crate::services::{
    CommandService, EventAggregator, LocalizationService, MenuRegistry, MessageBus, NavGraph,
    SharedStateStore, Subscription, ViewRegistry, WindowService,
};

/// C# `IPluginContext`. `ILogger` is replaced by `tracing` (spans carry the plugin id);
/// `GetService<T>` has no equivalent — everything a plugin may use is a field.
#[derive(Clone)]
pub struct PluginContext {
    pub plugin_id: String,
    pub plugin_path: PathBuf,
    pub data_path: PathBuf,
    /// What the permission manager granted (built-ins: `FULL_ACCESS`).
    pub granted: PluginPermission,
    pub commands: Arc<dyn CommandService>,
    pub window: Arc<dyn WindowService>,
    pub message_bus: Arc<dyn MessageBus>,
    pub events: Arc<dyn EventAggregator>,
    pub shared_state: Arc<dyn SharedStateStore>,
    pub menus: Arc<dyn MenuRegistry>,
    pub views: Arc<dyn ViewRegistry>,
    pub nav_graph: Arc<dyn NavGraph>,
    pub localization: Arc<dyn LocalizationService>,
    subscriptions: Arc<Mutex<Vec<Subscription>>>,
}

/// The shared services, independent of any single plugin.
#[derive(Clone)]
pub struct HostServices {
    pub commands: Arc<dyn CommandService>,
    pub window: Arc<dyn WindowService>,
    pub message_bus: Arc<dyn MessageBus>,
    pub events: Arc<dyn EventAggregator>,
    pub shared_state: Arc<dyn SharedStateStore>,
    pub menus: Arc<dyn MenuRegistry>,
    pub views: Arc<dyn ViewRegistry>,
    pub nav_graph: Arc<dyn NavGraph>,
    pub localization: Arc<dyn LocalizationService>,
}

impl PluginContext {
    #[must_use]
    pub fn new(
        plugin_id: &str,
        plugin_path: PathBuf,
        data_path: PathBuf,
        granted: PluginPermission,
        services: &HostServices,
    ) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_path,
            data_path,
            granted,
            commands: services.commands.clone(),
            window: services.window.clone(),
            message_bus: services.message_bus.clone(),
            events: services.events.clone(),
            shared_state: services.shared_state.clone(),
            menus: services.menus.clone(),
            views: services.views.clone(),
            nav_graph: services.nav_graph.clone(),
            localization: services.localization.clone(),
            subscriptions: Arc::default(),
        }
    }

    /// C# `context.Subscriptions.Add(...)`.
    pub fn track(&self, subscription: Subscription) {
        self.subscriptions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(subscription);
    }

    #[must_use]
    pub fn subscription_count(&self) -> usize {
        self.subscriptions
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }

    /// Dispose every tracked subscription, newest first.
    pub fn dispose_subscriptions(&self) {
        let subs = std::mem::take(
            &mut *self
                .subscriptions
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        );
        subs.into_iter().rev().for_each(Subscription::dispose);
    }
}

impl std::fmt::Debug for PluginContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginContext")
            .field("plugin_id", &self.plugin_id)
            .field("granted", &self.granted)
            .finish_non_exhaustive()
    }
}
