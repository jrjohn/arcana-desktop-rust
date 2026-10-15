//! How a third-party plugin's `main` file becomes a [`Plugin`] (C# `PluginLoadContext` +
//! `Activator.CreateInstance`). The host knows no file format itself; loaders are injected.

use std::path::Path;

use arcana_core::AppResult;
use arcana_plugin_api::{Plugin, PluginManifest, PluginPermission};

/// Turns an installed package into a plugin instance.
pub trait PluginLoader: Send + Sync {
    /// Whether this loader handles the manifest's `main` (e.g. `*.wasm`).
    fn can_load(&self, manifest: &PluginManifest) -> bool;
    /// # Errors
    /// The package cannot be loaded (missing file, bad ABI, …).
    fn load(
        &self,
        manifest: &PluginManifest,
        plugin_dir: &Path,
        granted: PluginPermission,
    ) -> AppResult<Box<dyn Plugin>>;
}
