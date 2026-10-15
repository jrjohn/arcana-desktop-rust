//! Third-party plugins as WASM components ([`arcana_plugin_wasm`]).
//!
//! The manifest's approved permissions become sandbox [`Grants`]; nothing else reaches the
//! guest. Contributed commands run the guest's `run-command` export; the guest's `render`
//! output (a declarative UI tree) is exposed through [`Plugin::ui_tree`].

use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{
    Plugin, PluginContext, PluginManifest, PluginMetadata, PluginPermission, PluginState,
};
use arcana_plugin_wasm::{Grants, Limits, WasmPlugin, WasmSandbox};

use crate::loader::PluginLoader;

/// Permission → grant mapping. Network needs a host HTTP client; none is wired, so even an
/// approved `NetworkAccess` is denied by the sandbox (reported to the guest as an error).
#[must_use]
pub fn grants_for(granted: PluginPermission, data_dir: &Path) -> Grants {
    Grants {
        log: true,
        fs_read: (granted.contains(PluginPermission::READ_LOCAL_DATA)
            || granted.contains(PluginPermission::READ_FILE_SYSTEM))
        .then(|| data_dir.to_path_buf()),
        net: None,
    }
}

/// [`PluginLoader`] for `main: "*.wasm"`.
#[derive(Debug, Clone, Default)]
pub struct WasmLoader {
    pub limits: Limits,
}

impl PluginLoader for WasmLoader {
    fn can_load(&self, manifest: &PluginManifest) -> bool {
        manifest
            .main
            .as_deref()
            .is_some_and(|m| m.to_ascii_lowercase().ends_with(".wasm"))
    }

    fn load(
        &self,
        manifest: &PluginManifest,
        plugin_dir: &Path,
        granted: PluginPermission,
    ) -> AppResult<Box<dyn Plugin>> {
        crate::compat::validate_abi(manifest)?;
        let main = manifest.main.as_deref().unwrap_or_default();
        let bytes = std::fs::read(plugin_dir.join(main))
            .map_err(|e| AppError::data(ErrorCode::FileNotFound, format!("{main}: {e}")))?;
        let inner = WasmSandbox::new(self.limits).load(&bytes, grants_for(granted, plugin_dir))?;
        let mut metadata = PluginMetadata::new(
            &manifest.id,
            &manifest.name,
            crate::compat::parse_lenient(&manifest.version),
            manifest.parsed_type(),
        );
        metadata.description.clone_from(&manifest.description);
        metadata.author.clone_from(&manifest.author);
        metadata.dependencies = manifest.dependencies.clone().unwrap_or_default();
        metadata.activation_events = manifest.activation_events().to_vec();
        let commands = manifest
            .contributes
            .as_ref()
            .and_then(|c| c.commands.as_ref())
            .map(|c| c.iter().map(|c| c.id.clone()).collect())
            .unwrap_or_default();
        Ok(Box::new(WasmHostedPlugin {
            metadata,
            state: PluginState::Loaded,
            inner: Arc::new(Mutex::new(inner)),
            commands,
            context: None,
        }))
    }
}

/// A sandboxed plugin seen through the [`Plugin`] trait.
pub struct WasmHostedPlugin {
    metadata: PluginMetadata,
    state: PluginState,
    inner: Arc<Mutex<WasmPlugin>>,
    commands: Vec<String>,
    context: Option<PluginContext>,
}

impl std::fmt::Debug for WasmHostedPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmHostedPlugin")
            .field("id", &self.metadata.id)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl Plugin for WasmHostedPlugin {
    fn metadata(&self) -> &PluginMetadata {
        &self.metadata
    }

    fn state(&self) -> PluginState {
        self.state
    }

    fn activate(&mut self, context: &PluginContext) -> AppResult<()> {
        self.state = PluginState::Activating;
        let result = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .activate()
            .and_then(|()| {
                for cmd in &self.commands {
                    let inner = self.inner.clone();
                    let name = cmd.clone();
                    let sub = context.commands.register_command(
                        cmd,
                        Arc::new(move |_| {
                            inner
                                .lock()
                                .unwrap_or_else(PoisonError::into_inner)
                                .run_command(&name, "")
                                .map(|_| ())
                        }),
                    )?;
                    context.track(sub);
                }
                Ok(())
            });
        self.context = Some(context.clone());
        self.state = if result.is_ok() {
            PluginState::Active
        } else {
            context.dispose_subscriptions();
            PluginState::Error
        };
        result
    }

    fn deactivate(&mut self) -> AppResult<()> {
        self.state = PluginState::Deactivating;
        if let Some(c) = &self.context {
            c.dispose_subscriptions();
        }
        let r = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .deactivate();
        self.state = if r.is_ok() {
            PluginState::Deactivated
        } else {
            PluginState::Error
        };
        r
    }

    fn ui_tree(&mut self) -> Option<serde_json::Value> {
        let node = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .render()
            .ok()?;
        serde_json::to_value(node).ok()
    }
}
