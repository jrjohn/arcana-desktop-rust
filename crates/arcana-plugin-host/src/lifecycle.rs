//! C# `PluginManagerService` install / uninstall / upgrade / rollback / reload and the
//! version history (persisted through [`crate::version_store::PluginVersionStore`]).
//!
//! A package is a directory holding `plugin.json` and the `main` file (C# uses a `.zip`;
//! zip handling is not ported — see `PORTING_PLAN` §8).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{
    PluginManifest, PluginOperationResult, PluginPermission, PluginState, PluginVersionInfo,
};
use chrono::Utc;

use crate::manager::PluginHost;
use crate::manifest_service::ManifestService;

fn io(e: &std::io::Error, what: &str) -> AppError {
    AppError::data(ErrorCode::FileAccessDenied, format!("{what}: {e}"))
}

/// C# `CopyDirectory`.
pub(crate) fn copy_dir(from: &Path, to: &Path) -> AppResult<()> {
    std::fs::create_dir_all(to).map_err(|e| io(&e, "create directory"))?;
    for entry in std::fs::read_dir(from).map_err(|e| io(&e, "read directory"))? {
        let entry = entry.map_err(|e| io(&e, "read directory"))?;
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), &target).map_err(|e| io(&e, "copy file"))?;
        }
    }
    Ok(())
}

/// C# `GetDirectorySize`.
pub(crate) fn dir_size(path: &Path) -> i64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|e| {
            let p = e.path();
            if p.is_dir() {
                dir_size(&p)
            } else {
                e.metadata()
                    .map_or(0, |m| i64::try_from(m.len()).unwrap_or(i64::MAX))
            }
        })
        .sum()
}

fn remove_dir(path: &Path) -> AppResult<()> {
    if path.exists() {
        std::fs::remove_dir_all(path).map_err(|e| io(&e, "remove directory"))?;
    }
    Ok(())
}

fn read_package_manifest(package: &Path) -> AppResult<PluginManifest> {
    let path = ManifestService::manifest_path(package).ok_or_else(|| {
        AppError::validation("Invalid plugin package: missing plugin.json", Vec::new())
    })?;
    let json = std::fs::read_to_string(&path).map_err(|e| io(&e, "read manifest"))?;
    let m = PluginManifest::from_json(&json)?;
    if m.id.is_empty() {
        return Err(AppError::validation("Invalid plugin manifest", Vec::new()));
    }
    Ok(m)
}

fn op_err(id: Option<&str>, e: &AppError, code: &str) -> PluginOperationResult {
    PluginOperationResult::failed(id, e.message(), Some(code))
}

impl PluginHost {
    fn backup_dir(&self, id: &str, version: &semver::Version) -> PathBuf {
        self.paths.backups.join(id).join(version.to_string())
    }

    async fn record_version(&self, id: &str, version: &semver::Version, install_path: &Path) {
        let info = PluginVersionInfo {
            plugin_id: id.to_string(),
            version: version.clone(),
            installed_at: Utc::now(),
            backup_path: self.backup_dir(id, version).display().to_string(),
            is_current: true,
            size_bytes: dir_size(install_path),
            release_notes: None,
        };
        let r = match self.store.version(id, &version.to_string()).await {
            Ok(Some(_)) => self.store.update_version(&info).await,
            _ => self.store.add_version(&info).await,
        };
        let r = r.and(
            self.store
                .set_current_version(id, &version.to_string())
                .await,
        );
        if let Err(e) = r {
            tracing::warn!(plugin = %id, error = %e, "version history not stored");
        }
    }

    /// C# `InstallPluginAsync`: copy the package into the plugins directory, discover it
    /// (lazy contributions + activation events) and record the version.
    pub async fn install(&self, package: &Path) -> PluginOperationResult {
        if !package.exists() {
            return PluginOperationResult::failed(
                None,
                &format!("Package file not found: {}", package.display()),
                Some("FILE_NOT_FOUND"),
            );
        }
        let manifest = match read_package_manifest(package) {
            Ok(m) => m,
            Err(e) => return op_err(None, &e, "INVALID_PACKAGE"),
        };
        let id = manifest.id.clone();
        if self.plugin(&id).is_some() {
            return PluginOperationResult::failed(
                Some(&id),
                &format!("Plugin {id} is already installed. Use upgrade instead."),
                Some("ALREADY_INSTALLED"),
            );
        }
        if let Err(e) = crate::compat::validate_abi(&manifest) {
            return op_err(Some(&id), &e, "INCOMPATIBLE");
        }
        let target = self.paths.plugins.join(&id);
        let staged = remove_dir(&target).and_then(|()| copy_dir(package, &target));
        if let Err(e) = staged {
            let _ = remove_dir(&target);
            return op_err(Some(&id), &e, "INSTALL_FAILED");
        }
        let Some(m) =
            ManifestService::manifest_path(&target).and_then(|p| self.manifests.load_manifest(&p))
        else {
            let _ = remove_dir(&target);
            return PluginOperationResult::failed(
                Some(&id),
                "Invalid plugin manifest",
                Some("INVALID_MANIFEST"),
            );
        };
        let version = crate::compat::parse_lenient(&m.version);
        self.add_pending(m);
        self.record_version(&id, &version, &target).await;
        PluginOperationResult::succeeded(&id, Some(&format!("Plugin {id} installed successfully")))
    }

    /// C# `UninstallPluginAsync`.
    pub async fn uninstall(&self, id: &str) -> PluginOperationResult {
        let Some(info) = self.plugin(id) else {
            return PluginOperationResult::failed(Some(id), "Plugin not found", Some("NOT_FOUND"));
        };
        if !info.can_uninstall {
            return PluginOperationResult::failed(
                Some(id),
                "Cannot uninstall built-in plugin",
                Some("BUILTIN_PLUGIN"),
            );
        }
        if info.state == PluginState::Active {
            let _ = self.deactivate(id);
        }
        self.lazy.on_plugin_activated(id); // drop lazy placeholders
        {
            let mut reg = self.reg();
            reg.loaded.remove(id);
            reg.pending.remove(id);
        }
        self.manifests.remove(id);
        self.activation.forget(id);
        let r = remove_dir(&self.paths.backups.join(id))
            .and_then(|()| remove_dir(Path::new(&info.install_path)));
        if let Err(e) = self.store.delete_all_versions(id).await {
            tracing::warn!(plugin = %id, error = %e, "version history not removed");
        }
        if let Err(e) = r {
            return op_err(Some(id), &e, "UNINSTALL_FAILED");
        }
        self.raise(id, PluginState::Loaded, PluginState::Uninstalled);
        PluginOperationResult::succeeded(id, Some("Plugin uninstalled successfully"))
    }

    /// Drop the instance and re-discover the installed files (C# `DiscoverPluginsAsync`
    /// after replacing a directory).
    fn rediscover(&self, id: &str) {
        self.lazy.on_plugin_activated(id);
        {
            let mut reg = self.reg();
            reg.loaded.remove(id);
            reg.pending.remove(id);
        }
        self.activation.forget(id);
        if let Some(p) = ManifestService::manifest_path(&self.paths.plugins.join(id))
            && let Some(m) = self.manifests.load_manifest(&p)
        {
            self.add_pending(m);
        }
    }

    fn hot_reload_state(&self, id: &str) -> AppResult<Option<BTreeMap<String, serde_json::Value>>> {
        self.with_plugin(id, |p| match p.as_hot_reload() {
            Some(h) => h.on_before_reload().map(|()| Some(h.state())),
            None => Ok(None),
        })
        .unwrap_or(Ok(None))
    }

    fn restore_hot_reload(&self, id: &str, state: Option<BTreeMap<String, serde_json::Value>>) {
        if let Some(state) = state {
            let _ = self.with_plugin(id, |p| {
                if let Some(h) = p.as_hot_reload() {
                    h.restore_state(state);
                    if let Err(e) = h.on_after_reload() {
                        tracing::warn!(plugin = %id, error = %e, "on_after_reload");
                    }
                }
            });
        }
    }

    /// Load the new instance so hooks (`IPluginUpgradeable`, `IPluginHotReload`) can run on
    /// it; C# gets this from re-discovery with lazy loading off.
    fn ensure_loaded(&self, id: &str) {
        if self.is_pending(id)
            && let Err(e) = self.activate(id)
        {
            tracing::warn!(plugin = %id, error = %e, "reload activation");
        }
    }

    /// C# `UpgradePluginAsync`.
    pub async fn upgrade(&self, id: &str, package: &Path) -> PluginOperationResult {
        let Some(existing) = self.plugin(id) else {
            return PluginOperationResult::failed(Some(id), "Plugin not found", Some("NOT_FOUND"));
        };
        if !existing.can_upgrade {
            return PluginOperationResult::failed(
                Some(id),
                "Cannot upgrade built-in plugin",
                Some("BUILTIN_PLUGIN"),
            );
        }
        let new_manifest = match read_package_manifest(package) {
            Ok(m) if m.id == id => m,
            Ok(m) => {
                return PluginOperationResult::failed(
                    Some(id),
                    &format!("Package is for plugin {}", m.id),
                    Some("UPGRADE_FAILED"),
                );
            }
            Err(e) => return op_err(Some(id), &e, "UPGRADE_FAILED"),
        };
        if let Err(e) = crate::compat::validate_abi(&new_manifest) {
            return op_err(Some(id), &e, "INCOMPATIBLE");
        }
        let current = existing.version.clone();
        let new_version = crate::compat::parse_lenient(&new_manifest.version);
        let was_active = existing.state == PluginState::Active;
        let install = PathBuf::from(&existing.install_path);
        let backup = self.backup_dir(id, &current);
        if let Err(e) = remove_dir(&backup).and_then(|()| copy_dir(&install, &backup)) {
            return op_err(Some(id), &e, "UPGRADE_FAILED");
        }
        let saved = match self.hot_reload_state(id) {
            Ok(s) => s,
            Err(e) => return op_err(Some(id), &e, "UPGRADE_FAILED"),
        };
        let veto = self.with_plugin(id, |p| {
            p.as_upgradeable().map(|u| {
                if !u.can_upgrade_to(&new_version) {
                    Some((
                        "UPGRADE_NOT_SUPPORTED",
                        format!("Plugin cannot be upgraded to version {new_version}"),
                    ))
                } else if !u.on_before_upgrade(&current, &new_version) {
                    Some((
                        "UPGRADE_REJECTED",
                        "Plugin rejected the upgrade".to_string(),
                    ))
                } else {
                    None
                }
            })
        });
        if let Some(Some(Some((code, msg)))) = veto {
            return PluginOperationResult::failed(Some(id), &msg, Some(code));
        }
        if was_active {
            let _ = self.deactivate(id);
        }
        // Transactional swap: on failure the backup is restored.
        let swapped = remove_dir(&install).and_then(|()| copy_dir(package, &install));
        if let Err(e) = swapped {
            let _ = remove_dir(&install).and_then(|()| copy_dir(&backup, &install));
            self.rediscover(id);
            if was_active {
                let _ = self.activate(id);
            }
            return op_err(Some(id), &e, "UPGRADE_FAILED");
        }
        self.rediscover(id);
        self.ensure_loaded(id);
        self.record_version(id, &new_version, &install).await;
        let _ = self.with_plugin(id, |p| {
            if let Some(u) = p.as_upgradeable() {
                u.on_after_upgrade(&current, &new_version);
            }
        });
        self.restore_hot_reload(id, saved);
        if was_active {
            let _ = self.activate(id);
        }
        PluginOperationResult::succeeded(
            id,
            Some(&format!("Plugin upgraded from {current} to {new_version}")),
        )
    }

    /// C# `RollbackPluginAsync`.
    pub async fn rollback(&self, id: &str, target: &str) -> PluginOperationResult {
        let Some(info) = self.plugin(id) else {
            return PluginOperationResult::failed(Some(id), "Plugin not found", Some("NOT_FOUND"));
        };
        let versions = self.available_versions(id).await;
        let Some(target_info) = versions.iter().find(|v| v.version.to_string() == target) else {
            return PluginOperationResult::failed(
                Some(id),
                &format!("Version {target} not found in backup history"),
                Some("VERSION_NOT_FOUND"),
            );
        };
        let backup = PathBuf::from(&target_info.backup_path);
        if !backup.is_dir() {
            return PluginOperationResult::failed(
                Some(id),
                "Backup files not found",
                Some("BACKUP_NOT_FOUND"),
            );
        }
        let current = info.version.clone();
        let target_v = target_info.version.clone();
        let veto = self.with_plugin(id, |p| {
            p.as_upgradeable().map(|u| {
                if !u.can_rollback_to(&target_v) {
                    Some((
                        "ROLLBACK_NOT_SUPPORTED",
                        format!("Plugin cannot be rolled back to version {target}"),
                    ))
                } else if !u.on_before_rollback(&current, &target_v) {
                    Some((
                        "ROLLBACK_REJECTED",
                        "Plugin rejected the rollback".to_string(),
                    ))
                } else {
                    None
                }
            })
        });
        if let Some(Some(Some((code, msg)))) = veto {
            return PluginOperationResult::failed(Some(id), &msg, Some(code));
        }
        let was_active = info.state == PluginState::Active;
        if was_active {
            let _ = self.deactivate(id);
        }
        let install = PathBuf::from(&info.install_path);
        // Keep the version being replaced so a later rollforward is possible.
        let current_backup = self.backup_dir(id, &current);
        if !current_backup.exists() {
            let _ = copy_dir(&install, &current_backup);
        }
        if let Err(e) = remove_dir(&install).and_then(|()| copy_dir(&backup, &install)) {
            let _ = remove_dir(&install).and_then(|()| copy_dir(&current_backup, &install));
            self.rediscover(id);
            return op_err(Some(id), &e, "ROLLBACK_FAILED");
        }
        self.rediscover(id);
        self.ensure_loaded(id);
        if let Err(e) = self.store.set_current_version(id, target).await {
            tracing::warn!(plugin = %id, error = %e, "current version not stored");
        }
        let _ = self.with_plugin(id, |p| {
            if let Some(u) = p.as_upgradeable() {
                u.on_after_rollback(&current, &target_v);
            }
        });
        if was_active {
            let _ = self.activate(id);
        }
        PluginOperationResult::succeeded(
            id,
            Some(&format!("Plugin rolled back from {current} to {target}")),
        )
    }

    /// C# `GetAvailableVersions`: newest first, `is_current` from the running version.
    pub async fn available_versions(&self, id: &str) -> Vec<PluginVersionInfo> {
        let current = self.plugin(id).map(|p| p.version);
        let mut versions = self.store.versions(id).await.unwrap_or_default();
        for v in &mut versions {
            v.is_current = current.as_ref() == Some(&v.version);
        }
        versions.sort_by(|a, b| b.version.cmp(&a.version));
        versions
    }

    /// C# `ReloadPluginAsync`.
    pub fn reload(&self, id: &str) -> PluginOperationResult {
        let Some(info) = self.plugin(id) else {
            return PluginOperationResult::failed(Some(id), "Plugin not found", Some("NOT_FOUND"));
        };
        let was_active = info.state == PluginState::Active;
        let saved = match self.hot_reload_state(id) {
            Ok(s) => s,
            Err(e) => return op_err(Some(id), &e, "RELOAD_FAILED"),
        };
        if was_active && let Err(e) = self.deactivate(id) {
            return op_err(Some(id), &e, "RELOAD_FAILED");
        }
        if !info.is_built_in {
            self.rediscover(id);
            self.ensure_loaded(id);
        }
        self.restore_hot_reload(id, saved);
        if was_active && let Err(e) = self.activate(id) {
            return op_err(Some(id), &e, "RELOAD_FAILED");
        }
        PluginOperationResult::succeeded(id, Some("Plugin reloaded successfully"))
    }

    /// Approve (grant) permissions a third-party plugin requested; takes effect on the next
    /// activation.
    pub fn approve_permissions(&self, id: &str) -> PluginOperationResult {
        if self.plugin(id).is_none() {
            return PluginOperationResult::failed(Some(id), "Plugin not found", Some("NOT_FOUND"));
        }
        self.permissions.approve(id, PluginPermission::FULL_ACCESS);
        PluginOperationResult::succeeded(id, Some("Permissions approved"))
    }
}
