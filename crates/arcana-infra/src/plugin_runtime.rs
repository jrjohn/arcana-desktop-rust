//! Plugin runtime wiring (C# `AddPluginServices` + `App.InitializePluginsAsync`).
//!
//! Builds the shared plugin services, a [`PluginHost`] persisting to the main database,
//! registers the compiled-in built-ins (plus any the caller passes, e.g. `FlowChart` from the
//! UI layer), discovers installed third-party plugins and activates everything.

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use arcana_data::Db;
use arcana_plugin_api::{LocalizationService, MessageBusExt, Plugin};
use arcana_plugin_host::services::nav::QueuedNavGraph;
use arcana_plugin_host::services::window::QueuedWindowService;
use arcana_plugin_host::{HostPaths, PluginHost};

use crate::plugin_store::SqlitePluginVersionStore;
use crate::plugins::ShellCommand;

/// Route ids plugins may navigate to (C# `NavGraph.Routes` + plugin views).
pub const KNOWN_ROUTES: [&str; 12] = [
    "HomePage",
    "DashboardPage",
    "OrderListPage",
    "OrderDetailPage",
    "ProductListPage",
    "ProductDetailPage",
    "CustomerListPage",
    "CustomerDetailPage",
    "PluginManagerPage",
    "SettingsPage",
    "FlowChartEditorPage",
    "ProductCategoriesPage",
];

/// Everything the shell needs from the plugin system. Cheap to clone.
#[derive(Clone)]
pub struct PluginRuntime {
    pub host: Arc<PluginHost>,
    /// Navigation requests queued by plugins; the shell drains them each frame.
    pub nav: QueuedNavGraph,
    /// Messages / confirmations queued by plugins.
    pub window: QueuedWindowService,
    shell_commands: Arc<Mutex<Vec<String>>>,
}

impl std::fmt::Debug for PluginRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginRuntime")
            .field("host", &self.host)
            .finish_non_exhaustive()
    }
}

impl PluginRuntime {
    /// Wire, register, discover and activate. `root` holds `plugins/`, `plugin-data/` and
    /// backups; `extra` are built-ins from layers above infra.
    #[must_use]
    pub fn start(
        db: &Db,
        localization: Arc<dyn LocalizationService>,
        root: &Path,
        extra: Vec<Box<dyn Plugin>>,
    ) -> Self {
        let nav = QueuedNavGraph::new(KNOWN_ROUTES.iter().map(|r| (*r).to_string()));
        let window = QueuedWindowService::new();
        let services = crate::wiring::plugin_host_services(
            Arc::new(window.clone()),
            Arc::new(nav.clone()),
            localization,
        );
        let bus = services.message_bus.clone();
        let shell_commands: Arc<Mutex<Vec<String>>> = Arc::default();
        let sink = shell_commands.clone();
        // Lives as long as the bus; intentionally never disposed.
        let _subscription = bus.subscribe::<ShellCommand>(move |c| {
            sink.lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(c.0.clone());
        });
        let paths = HostPaths::under(root);
        for dir in [&paths.plugins, &paths.data] {
            if let Err(e) = std::fs::create_dir_all(dir) {
                tracing::warn!(path = %dir.display(), error = %e, "plugin directory");
            }
        }
        let host = PluginHost::new(
            services,
            paths,
            Arc::new(SqlitePluginVersionStore::new(db.clone())),
        );
        host.add_loader(Arc::new(arcana_plugin_host::wasm::WasmLoader::default()));
        host.register_compiled_builtins();
        for p in extra {
            host.register_builtin(p);
        }
        host.discover();
        host.activate_all();
        Self {
            host,
            nav,
            window,
            shell_commands,
        }
    }

    /// Shell commands plugins asked for since the last call.
    #[must_use]
    pub fn take_shell_commands(&self) -> Vec<String> {
        std::mem::take(
            &mut *self
                .shell_commands
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }
}
