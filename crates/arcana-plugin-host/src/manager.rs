//! C# `PluginManager` + `PluginManagerService`: the instance-scoped plugin registry and
//! its state machine. Install / upgrade / rollback live in [`crate::lifecycle`].
//!
//! Nothing is static: create one [`PluginHost`] per application (or per test).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{
    HostServices, Plugin, PluginContext, PluginHealthStatus, PluginInfo, PluginManifest,
    PluginMetadata, PluginOperationResult, PluginPermission, PluginState, PluginStateChanged,
    PluginType, activation_events,
};
use chrono::{DateTime, Utc};

use crate::activation::ActivationEventService;
use crate::dependency::DependencyResolver;
use crate::health::{HealthTarget, PluginHealthMonitor};
use crate::lazy::LazyContributionService;
use crate::loader::PluginLoader;
use crate::manifest_service::ManifestService;
use crate::permission::PluginPermissionManager;
use crate::version_store::PluginVersionStore;

/// A plugin instance the host owns (C# `LoadedPlugin`).
pub(crate) struct Loaded {
    pub plugin: Box<dyn Plugin>,
    pub path: PathBuf,
    pub data_path: PathBuf,
    pub built_in: bool,
    pub manifest: Option<PluginManifest>,
    pub context: Option<PluginContext>,
    pub installed_at: DateTime<Utc>,
    pub last_activated_at: Option<DateTime<Utc>>,
}

/// A discovered, not yet loaded third-party plugin (C# `PendingPlugin`).
#[derive(Debug, Clone)]
pub(crate) struct Pending {
    pub manifest: PluginManifest,
    pub path: PathBuf,
    pub installed_at: DateTime<Utc>,
}

pub type StateChangedHandler = Arc<dyn Fn(&PluginStateChanged) + Send + Sync>;

#[derive(Default)]
pub(crate) struct Registry {
    pub loaded: BTreeMap<String, Loaded>,
    pub pending: BTreeMap<String, Pending>,
    /// Ids currently inside `activate`/`deactivate` (taken out of `loaded`).
    pub busy: BTreeMap<String, PluginMetadata>,
}

/// Directories the host works in.
#[derive(Debug, Clone)]
pub struct HostPaths {
    /// Installed third-party plugins, one sub-directory each.
    pub plugins: PathBuf,
    /// Per-plugin data directories.
    pub data: PathBuf,
    /// Version backups (`<backups>/<id>/<version>`).
    pub backups: PathBuf,
}

impl HostPaths {
    #[must_use]
    pub fn under(root: &Path) -> Self {
        Self {
            plugins: root.join("plugins"),
            data: root.join("plugin-data"),
            backups: root.join("plugins").join(".backups"),
        }
    }
}

/// The plugin runtime.
pub struct PluginHost {
    pub services: HostServices,
    pub manifests: Arc<ManifestService>,
    pub activation: Arc<ActivationEventService>,
    pub lazy: Arc<LazyContributionService>,
    pub permissions: Arc<PluginPermissionManager>,
    pub health: Arc<PluginHealthMonitor>,
    pub store: Arc<dyn PluginVersionStore>,
    pub paths: HostPaths,
    pub(crate) registry: Mutex<Registry>,
    pub(crate) loaders: Mutex<Vec<Arc<dyn PluginLoader>>>,
    handlers: Mutex<Vec<StateChangedHandler>>,
}

impl std::fmt::Debug for PluginHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = self.reg();
        f.debug_struct("PluginHost")
            .field("loaded", &r.loaded.keys().collect::<Vec<_>>())
            .field("pending", &r.pending.keys().collect::<Vec<_>>())
            .finish_non_exhaustive()
    }
}

/// `"id@range"` → `"id"`.
pub(crate) fn dependency_id(dep: &str) -> String {
    dep.split_once('@')
        .map_or(dep, |(id, _)| id)
        .trim()
        .to_string()
}

pub(crate) fn not_found(id: &str) -> AppError {
    AppError::Plugin {
        code: ErrorCode::PluginNotFound,
        message: format!("Plugin not found: {id}"),
        plugin_id: Some(id.to_string()),
    }
}

pub(crate) fn plugin_err(id: &str, code: ErrorCode, message: impl Into<String>) -> AppError {
    AppError::Plugin {
        code,
        message: message.into(),
        plugin_id: Some(id.to_string()),
    }
}

impl PluginHost {
    /// Build a host around shared services. The activation callback for lazy plugins is
    /// wired to [`PluginHost::activate`].
    pub fn new(
        services: HostServices,
        paths: HostPaths,
        store: Arc<dyn PluginVersionStore>,
    ) -> Arc<Self> {
        let activation = Arc::new(ActivationEventService::new());
        let lazy = Arc::new(LazyContributionService::new(
            services.menus.clone(),
            services.views.clone(),
            services.commands.clone(),
            activation.clone(),
            services.localization.clone(),
        ));
        let host = Arc::new(Self {
            services,
            manifests: Arc::new(ManifestService::new()),
            activation: activation.clone(),
            lazy,
            permissions: Arc::new(PluginPermissionManager::new(&paths.data)),
            health: Arc::new(PluginHealthMonitor::with_store(store.clone())),
            store,
            paths,
            registry: Mutex::default(),
            loaders: Mutex::default(),
            handlers: Mutex::default(),
        });
        let weak = Arc::downgrade(&host);
        activation.set_activation_callback(Arc::new(move |id| {
            weak.upgrade()
                .ok_or_else(|| not_found(id))
                .and_then(|h| h.activate(id))
        }));
        host
    }

    pub(crate) fn reg(&self) -> MutexGuard<'_, Registry> {
        self.registry.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn add_loader(&self, loader: Arc<dyn PluginLoader>) {
        self.loaders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(loader);
    }

    /// C# `PluginStateChanged += ...`.
    pub fn on_state_changed(&self, handler: StateChangedHandler) {
        self.handlers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(handler);
    }

    pub(crate) fn raise(&self, id: &str, old: PluginState, new: PluginState) {
        let e = PluginStateChanged {
            plugin_id: id.to_string(),
            old_state: old,
            new_state: new,
            message: None,
        };
        let handlers = self
            .handlers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for h in &handlers {
            h(&e);
        }
    }

    fn data_path(&self, id: &str) -> PathBuf {
        self.paths.data.join(id)
    }

    /// C# `RegisterBuiltInPlugin` + `LoadPluginAsync`: built-ins get every permission.
    pub fn register_builtin(&self, plugin: Box<dyn Plugin>) {
        let id = plugin.metadata().id.clone();
        self.permissions
            .set_permissions(&id, PluginPermission::FULL_ACCESS);
        let mut reg = self.reg();
        if reg.loaded.contains_key(&id) {
            return;
        }
        reg.loaded.insert(
            id.clone(),
            Loaded {
                plugin,
                path: PathBuf::new(),
                data_path: self.data_path(&id),
                built_in: true,
                manifest: None,
                context: None,
                installed_at: Utc::now(),
                last_activated_at: None,
            },
        );
    }

    /// Register every `inventory`-submitted built-in.
    pub fn register_compiled_builtins(&self) {
        for p in crate::builtin::builtin_plugins() {
            self.register_builtin(p);
        }
    }

    /// C# `DiscoverPluginsAsync` (lazy mode): read each installed manifest, register its
    /// lazy contributions and activation events. Already known plugins are skipped.
    pub fn discover(&self) {
        for manifest in self.manifests.discover_manifests(&self.paths.plugins) {
            self.add_pending(manifest);
        }
    }

    pub(crate) fn add_pending(&self, manifest: PluginManifest) {
        let id = manifest.id.clone();
        {
            let reg = self.reg();
            if reg.loaded.contains_key(&id) || reg.pending.contains_key(&id) {
                return;
            }
        }
        let Some(dir) = self.manifests.manifest_directory(&id) else {
            return;
        };
        if let Ok(requested) = manifest.requested_permissions() {
            let _ = self.permissions.request_permissions(&id, requested);
        }
        self.lazy.register_manifest_contributions(&manifest, &dir);
        self.activation
            .register_pending_plugin(&id, manifest.activation_events());
        self.reg().pending.insert(
            id,
            Pending {
                manifest,
                path: dir,
                installed_at: Utc::now(),
            },
        );
    }

    /// C# `ActivateAllAsync`: fire `onStartup`, activate built-ins in dependency order, then
    /// pending plugins that want startup activation. Failures are logged, not returned.
    pub fn activate_all(&self) {
        self.activation.fire(activation_events::ON_STARTUP);
        let order = {
            let reg = self.reg();
            let metas: Vec<PluginMetadata> = reg
                .loaded
                .values()
                .map(|l| l.plugin.metadata().clone())
                .collect();
            let mut r = DependencyResolver::new();
            for m in &metas {
                if let Err(e) = r.register_metadata(m) {
                    tracing::warn!(plugin = %m.id, error = %e, "dependency metadata");
                }
            }
            let res = r.resolve();
            if res.success {
                Ok(res.resolved_order)
            } else {
                Err(AppError::Plugin {
                    code: ErrorCode::PluginIncompatible,
                    message: format!(
                        "missing: {:?}, conflicts: {}",
                        res.missing_dependencies,
                        res.conflicts.len()
                    ),
                    plugin_id: None,
                })
            }
        };
        let ids: Vec<String> = match order {
            Ok(ids) => ids,
            Err(e) => {
                tracing::error!(error = %e, "plugin dependency resolution failed");
                self.reg().loaded.keys().cloned().collect()
            }
        };
        for id in ids {
            let state = self.state(&id);
            if matches!(state, PluginState::NotLoaded | PluginState::Loaded)
                && let Err(e) = self.activate(&id)
            {
                tracing::error!(plugin = %id, error = %e, "activation failed");
            }
        }
        let startup: Vec<String> = self
            .reg()
            .pending
            .keys()
            .filter(|id| self.manifests.should_activate_on_startup(id))
            .cloned()
            .collect();
        for id in startup {
            if let Err(e) = self.activate(&id) {
                tracing::error!(plugin = %id, error = %e, "startup activation failed");
            }
        }
    }

    /// Current state; pending plugins report `Loaded` (C# `PluginInfo.FromManifest`).
    #[must_use]
    pub fn state(&self, id: &str) -> PluginState {
        let reg = self.reg();
        if let Some(l) = reg.loaded.get(id) {
            l.plugin.state()
        } else if reg.busy.contains_key(id) {
            PluginState::Activating
        } else if reg.pending.contains_key(id) {
            PluginState::Loaded
        } else {
            PluginState::Unknown
        }
    }

    #[must_use]
    pub fn is_pending(&self, id: &str) -> bool {
        self.reg().pending.contains_key(id)
    }

    fn load_pending(&self, pending: &Pending) -> AppResult<()> {
        let id = &pending.manifest.id;
        let loader = self
            .loaders
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .iter()
            .find(|l| l.can_load(&pending.manifest))
            .cloned()
            .ok_or_else(|| {
                plugin_err(
                    id,
                    ErrorCode::PluginLoadFailed,
                    format!("No loader for plugin main {:?}", pending.manifest.main),
                )
            })?;
        let granted = self.permissions.permissions(id);
        let plugin = loader.load(&pending.manifest, &pending.path, granted)?;
        let mut reg = self.reg();
        reg.pending.remove(id);
        reg.loaded.insert(
            id.clone(),
            Loaded {
                plugin,
                path: pending.path.clone(),
                data_path: self.data_path(id),
                built_in: false,
                manifest: Some(pending.manifest.clone()),
                context: None,
                installed_at: pending.installed_at,
                last_activated_at: None,
            },
        );
        Ok(())
    }

    /// C# `PluginManager.ActivatePluginAsync`: load a pending plugin if needed, activate its
    /// dependencies first, then the plugin. Already active → no-op.
    ///
    /// # Errors
    /// Unknown plugin, missing dependency, load failure or the plugin's own error (the
    /// plugin is then in [`PluginState::Error`] and the error is recorded by health).
    pub fn activate(&self, id: &str) -> AppResult<()> {
        let pending = self.reg().pending.get(id).cloned();
        if let Some(p) = pending {
            for dep in p.manifest.dependencies.iter().flatten() {
                self.activate(&crate::manager::dependency_id(dep))?;
            }
            self.load_pending(&p)?;
            self.lazy.on_plugin_activated(id);
            self.activation.mark_activated(id);
        }
        let (mut loaded, old) = {
            let mut reg = self.reg();
            if reg.busy.contains_key(id) {
                return Ok(()); // re-entrant activation through an activation event
            }
            let Some(l) = reg.loaded.remove(id) else {
                return Err(not_found(id));
            };
            let state = l.plugin.state();
            if state == PluginState::Active {
                reg.loaded.insert(id.to_string(), l);
                return Ok(());
            }
            reg.busy.insert(id.to_string(), l.plugin.metadata().clone());
            (l, state)
        };
        let deps = loaded.plugin.metadata().dependencies.clone();
        let result = deps
            .iter()
            .try_for_each(|d| self.activate(&crate::manager::dependency_id(d)))
            .and_then(|()| {
                let ctx = PluginContext::new(
                    id,
                    loaded.path.clone(),
                    loaded.data_path.clone(),
                    self.permissions.permissions(id),
                    &self.services,
                );
                self.register_resources(&*loaded.plugin);
                let r = loaded.plugin.activate(&ctx);
                loaded.context = Some(ctx);
                r
            });
        if result.is_ok() {
            loaded.last_activated_at = Some(Utc::now());
        }
        let new = loaded.plugin.state();
        {
            let mut reg = self.reg();
            reg.busy.remove(id);
            reg.loaded.insert(id.to_string(), loaded);
        }
        self.activation.mark_activated(id);
        self.lazy.on_plugin_activated(id);
        match &result {
            Ok(()) => self.health.record_success(id),
            Err(e) => self.health.record_error(id, &e.to_string()),
        }
        if old != new {
            self.raise(id, old, new);
        }
        result
    }

    fn register_resources(&self, plugin: &dyn Plugin) {
        let id = &plugin.metadata().id;
        for (culture, res) in plugin.resources() {
            self.services.localization.register_plugin_resources(
                id,
                &culture,
                res.into_iter().collect(),
            );
        }
    }

    /// C# `PluginManager.DeactivatePluginAsync`: only an active plugin is deactivated.
    ///
    /// # Errors
    /// The plugin's own error.
    pub fn deactivate(&self, id: &str) -> AppResult<()> {
        let mut loaded = {
            let mut reg = self.reg();
            match reg.loaded.get(id) {
                Some(l) if l.plugin.state() == PluginState::Active => {}
                _ => return Ok(()),
            }
            let Some(l) = reg.loaded.remove(id) else {
                return Ok(());
            };
            reg.busy.insert(id.to_string(), l.plugin.metadata().clone());
            l
        };
        let result = loaded.plugin.deactivate();
        if let Some(ctx) = &loaded.context {
            ctx.dispose_subscriptions();
        }
        let new = loaded.plugin.state();
        {
            let mut reg = self.reg();
            reg.busy.remove(id);
            reg.loaded.insert(id.to_string(), loaded);
        }
        match &result {
            Ok(()) => self.health.record_success(id),
            Err(e) => self.health.record_error(id, &e.to_string()),
        }
        self.raise(id, PluginState::Active, new);
        result
    }

    /// C# `PluginManagerService.ActivatePluginAsync`.
    pub fn activate_op(&self, id: &str) -> PluginOperationResult {
        match self.activate(id) {
            Ok(()) => PluginOperationResult::succeeded(id, Some("Plugin activated successfully")),
            Err(e) => {
                PluginOperationResult::failed(Some(id), e.message(), Some("ACTIVATION_FAILED"))
            }
        }
    }

    /// C# `PluginManagerService.DeactivatePluginAsync`.
    pub fn deactivate_op(&self, id: &str) -> PluginOperationResult {
        match self.deactivate(id) {
            Ok(()) => PluginOperationResult::succeeded(id, Some("Plugin deactivated successfully")),
            Err(e) => {
                PluginOperationResult::failed(Some(id), e.message(), Some("DEACTIVATION_FAILED"))
            }
        }
    }

    /// Remove a loaded plugin from the registry (C# `UnloadPluginAsync`).
    pub fn unload(&self, id: &str) -> bool {
        let _ = self.deactivate(id);
        self.reg().loaded.remove(id).is_some()
    }

    fn info_of(l: &Loaded) -> PluginInfo {
        let m = l.plugin.metadata();
        PluginInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
            author: m.author.clone(),
            plugin_type: m.plugin_type,
            state: l.plugin.state(),
            icon_path: m.icon_path.clone(),
            install_path: l.path.display().to_string(),
            installed_at: l.installed_at,
            last_activated_at: l.last_activated_at,
            is_built_in: l.built_in,
            can_uninstall: !l.built_in,
            can_upgrade: !l.built_in,
            dependencies: m.dependencies.clone(),
            metadata: BTreeMap::new(),
        }
    }

    /// C# `PluginInfo.FromManifest`.
    pub(crate) fn info_of_manifest(
        m: &PluginManifest,
        path: &Path,
        at: DateTime<Utc>,
    ) -> PluginInfo {
        PluginInfo {
            id: m.id.clone(),
            name: if m.name.is_empty() {
                m.id.clone()
            } else {
                m.name.clone()
            },
            version: crate::compat::parse_lenient(&m.version),
            description: m.description.clone(),
            author: m.author.clone(),
            plugin_type: PluginType::parse(&m.plugin_type).unwrap_or_default(),
            state: PluginState::Loaded,
            icon_path: m.icon.clone(),
            install_path: path.display().to_string(),
            installed_at: at,
            last_activated_at: None,
            is_built_in: false,
            can_uninstall: true,
            can_upgrade: true,
            dependencies: m.dependencies.clone().unwrap_or_default(),
            metadata: BTreeMap::new(),
        }
    }

    /// C# `GetAllPlugins`: loaded first (by id), then pending.
    #[must_use]
    pub fn all_plugins(&self) -> Vec<PluginInfo> {
        let reg = self.reg();
        reg.loaded
            .values()
            .map(Self::info_of)
            .chain(
                reg.pending
                    .values()
                    .map(|p| Self::info_of_manifest(&p.manifest, &p.path, p.installed_at)),
            )
            .collect()
    }

    /// C# `GetPlugin`.
    #[must_use]
    pub fn plugin(&self, id: &str) -> Option<PluginInfo> {
        let reg = self.reg();
        reg.loaded.get(id).map(Self::info_of).or_else(|| {
            reg.pending
                .get(id)
                .map(|p| Self::info_of_manifest(&p.manifest, &p.path, p.installed_at))
        })
    }

    /// The manifest of a third-party plugin (loaded or pending).
    #[must_use]
    pub fn manifest(&self, id: &str) -> Option<PluginManifest> {
        let reg = self.reg();
        reg.loaded
            .get(id)
            .and_then(|l| l.manifest.clone())
            .or_else(|| reg.pending.get(id).map(|p| p.manifest.clone()))
    }

    /// Run `f` against a loaded plugin instance.
    pub fn with_plugin<R>(&self, id: &str, f: impl FnOnce(&mut dyn Plugin) -> R) -> Option<R> {
        let mut reg = self.reg();
        reg.loaded.get_mut(id).map(|l| f(&mut *l.plugin))
    }

    /// C# `GetHealthStatus`.
    #[must_use]
    pub fn health_status(&self, id: &str) -> PluginHealthStatus {
        if let Some(h) = self.health.cached_health(id) {
            return h;
        }
        match self.plugin(id) {
            None => {
                let mut s =
                    PluginHealthStatus::new(id, "Unknown", arcana_plugin_api::HealthState::Unknown);
                s.message = Some("Plugin not found".to_string());
                s
            }
            Some(p) => {
                let state = if p.state == PluginState::Active {
                    arcana_plugin_api::HealthState::Healthy
                } else {
                    arcana_plugin_api::HealthState::Unknown
                };
                let mut s = PluginHealthStatus::new(id, &p.name, state);
                s.message = Some(format!("Plugin state: {:?}", p.state));
                s
            }
        }
    }

    /// C# `CheckAllHealthAsync`: check every loaded plugin and persist a snapshot each.
    pub async fn check_all_health(&self) -> Vec<PluginHealthStatus> {
        let results: Vec<PluginHealthStatus> = {
            let reg = self.reg();
            reg.loaded
                .values()
                .map(|l| {
                    let m = l.plugin.metadata();
                    let target = HealthTarget::new(&m.id, &m.name, l.plugin.state());
                    self.health.evaluate(&target, l.plugin.as_health_check())
                })
                .collect()
        };
        for s in &results {
            self.health.persist(s).await;
        }
        results
    }
}
