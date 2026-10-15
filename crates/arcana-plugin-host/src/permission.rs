//! C# `PluginPermissionManager`, `PluginSandbox`, `PluginPermissionException`.
//!
//! Rust addition: **pending approvals**. A third-party (WASM) manifest *requests* permissions
//! with [`PluginPermissionManager::request_permissions`]; nothing is granted until
//! [`PluginPermissionManager::approve`] is called (the UI asks the user). C# granted a
//! manifest's required set automatically (`GrantDefaultPermissionsAsync`, kept as is).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use arcana_core::{AppError, ErrorCode};
use arcana_plugin_api::{
    PermissionCheckResult, PermissionRequest, PluginPermission, PluginPermissionManifest,
};
use chrono::{DateTime, Utc};

const FILE_NAME: &str = "plugin_permissions.json";

/// C# `PermissionDeniedEventArgs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionDenied {
    pub plugin_id: String,
    pub requested_permission: PluginPermission,
    pub resource: Option<String>,
    pub timestamp: DateTime<Utc>,
}

pub type PermissionDeniedHandler = Arc<dyn Fn(&PermissionDenied) + Send + Sync>;

/// The one call [`PluginSandbox`] needs (C# mocks `IPluginPermissionManager` for it).
pub trait PermissionChecker: Send + Sync {
    fn check_permission(&self, request: &PermissionRequest) -> PermissionCheckResult;
}

#[derive(Default)]
struct Inner {
    granted: BTreeMap<String, PluginPermission>,
    pending: BTreeMap<String, PluginPermission>,
    manifests: BTreeMap<String, PluginPermissionManifest>,
    handlers: Vec<PermissionDeniedHandler>,
}

/// C# `PluginPermissionManager`. Grants persist to `<data>/plugin_permissions.json` as
/// `{ pluginId: bits }` (same shape as C#); save/load failures are logged, not returned.
pub struct PluginPermissionManager {
    file: PathBuf,
    inner: Mutex<Inner>,
}

impl fmt::Debug for PluginPermissionManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PluginPermissionManager")
            .field("file", &self.file)
            .finish_non_exhaustive()
    }
}

impl PluginPermissionManager {
    /// C# constructor: loads previously saved grants from `data_path`.
    #[must_use]
    pub fn new(data_path: &Path) -> Self {
        let file = data_path.join(FILE_NAME);
        let mut inner = Inner::default();
        match std::fs::read_to_string(&file) {
            Ok(json) => match serde_json::from_str::<BTreeMap<String, u32>>(&json) {
                Ok(map) => {
                    inner.granted = map
                        .into_iter()
                        .map(|(k, v)| (k, PluginPermission(v)))
                        .collect();
                }
                Err(e) => tracing::warn!(error = %e, "failed to load plugin permissions"),
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => tracing::warn!(error = %e, "failed to load plugin permissions"),
        }
        Self {
            file,
            inner: Mutex::new(inner),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn save(&self, inner: &Inner) {
        let data: BTreeMap<&str, u32> = inner
            .granted
            .iter()
            .map(|(k, v)| (k.as_str(), v.0))
            .collect();
        let result = serde_json::to_string_pretty(&data)
            .map_err(std::io::Error::other)
            .and_then(|json| {
                if let Some(dir) = self.file.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(&self.file, json)
            });
        if let Err(e) = result {
            tracing::warn!(error = %e, "failed to save plugin permissions");
        }
    }

    /// C# `PermissionDenied += handler`.
    pub fn on_permission_denied(&self, handler: PermissionDeniedHandler) {
        self.lock().handlers.push(handler);
    }

    /// C# `HasPermission`.
    #[must_use]
    pub fn has_permission(&self, plugin_id: &str, permission: PluginPermission) -> bool {
        self.permissions(plugin_id).contains(permission)
    }

    /// C# `GetPermissions`.
    #[must_use]
    pub fn permissions(&self, plugin_id: &str) -> PluginPermission {
        self.lock()
            .granted
            .get(plugin_id)
            .copied()
            .unwrap_or_default()
    }

    /// C# `GrantPermissionAsync`.
    pub fn grant_permission(&self, plugin_id: &str, permission: PluginPermission) {
        let mut inner = self.lock();
        let entry = inner.granted.entry(plugin_id.to_string()).or_default();
        *entry |= permission;
        tracing::info!(plugin_id, %permission, "granted permission");
        self.save(&inner);
    }

    /// C# `RevokePermissionAsync`.
    pub fn revoke_permission(&self, plugin_id: &str, permission: PluginPermission) {
        let mut inner = self.lock();
        let entry = inner.granted.entry(plugin_id.to_string()).or_default();
        *entry = *entry & !permission;
        tracing::info!(plugin_id, %permission, "revoked permission");
        self.save(&inner);
    }

    /// C# `SetPermissionsAsync`.
    pub fn set_permissions(&self, plugin_id: &str, permissions: PluginPermission) {
        let mut inner = self.lock();
        inner.granted.insert(plugin_id.to_string(), permissions);
        tracing::info!(plugin_id, %permissions, "set permissions");
        self.save(&inner);
    }

    /// C# `GetManifest`.
    #[must_use]
    pub fn manifest(&self, plugin_id: &str) -> Option<PluginPermissionManifest> {
        self.lock().manifests.get(plugin_id).cloned()
    }

    /// C# `RegisterManifest`.
    pub fn register_manifest(&self, plugin_id: &str, manifest: PluginPermissionManifest) {
        self.lock()
            .manifests
            .insert(plugin_id.to_string(), manifest);
    }

    /// C# `RegisterManifestFromType` — the `[RequiresPermission]` / `[OptionalPermission]`
    /// attributes become explicit `(permission, reason)` lists.
    pub fn register_manifest_from_attributes(
        &self,
        plugin_id: &str,
        required: &[(PluginPermission, Option<&str>)],
        optional: &[(PluginPermission, Option<&str>)],
    ) {
        let mut m = PluginPermissionManifest::default();
        for (p, reason) in required {
            m.required_permissions |= *p;
            if let Some(r) = reason.filter(|r| !r.is_empty()) {
                m.permission_reasons.insert(p.to_string(), r.to_string());
            }
        }
        for (p, reason) in optional {
            m.optional_permissions |= *p;
            if let Some(r) = reason.filter(|r| !r.is_empty()) {
                m.permission_reasons.insert(p.to_string(), r.to_string());
            }
        }
        self.register_manifest(plugin_id, m);
    }

    /// C# `ValidateRequiredPermissions`.
    #[must_use]
    pub fn validate_required_permissions(&self, plugin_id: &str) -> PermissionCheckResult {
        let Some(m) = self.manifest(plugin_id) else {
            return PermissionCheckResult::allow();
        };
        let missing = m.required_permissions & !self.permissions(plugin_id);
        if missing.is_empty() {
            PermissionCheckResult::allow()
        } else {
            PermissionCheckResult::deny(
                missing,
                Some(format!("Missing required permissions: {missing}")),
            )
        }
    }

    /// C# `GrantBuiltInPermissionsAsync`.
    pub fn grant_built_in_permissions(&self, plugin_id: &str) {
        self.set_permissions(plugin_id, PluginPermission::FULL_ACCESS);
    }

    /// C# `GrantDefaultPermissionsAsync`.
    pub fn grant_default_permissions(&self, plugin_id: &str) {
        let perms = self
            .manifest(plugin_id)
            .map_or(PluginPermission::BASIC_PLUGIN, |m| m.required_permissions);
        self.set_permissions(plugin_id, perms);
    }

    /// Rust addition: record what a manifest asks for without granting it. Already-granted
    /// bits are not pending. Returns the pending set.
    pub fn request_permissions(
        &self,
        plugin_id: &str,
        requested: PluginPermission,
    ) -> PluginPermission {
        let mut inner = self.lock();
        let granted = inner.granted.get(plugin_id).copied().unwrap_or_default();
        let pending = requested & !granted;
        if pending.is_empty() {
            inner.pending.remove(plugin_id);
        } else {
            inner.pending.insert(plugin_id.to_string(), pending);
        }
        pending
    }

    /// Permissions requested but not yet approved.
    #[must_use]
    pub fn pending(&self, plugin_id: &str) -> PluginPermission {
        self.lock()
            .pending
            .get(plugin_id)
            .copied()
            .unwrap_or_default()
    }

    /// Approve (a subset of) the pending request. Bits that were never requested are ignored,
    /// so approval can never widen what the manifest declared. Returns what was granted.
    pub fn approve(&self, plugin_id: &str, permissions: PluginPermission) -> PluginPermission {
        let mut inner = self.lock();
        let pending = inner.pending.get(plugin_id).copied().unwrap_or_default();
        let approved = pending & permissions;
        let left = pending & !approved;
        if left.is_empty() {
            inner.pending.remove(plugin_id);
        } else {
            inner.pending.insert(plugin_id.to_string(), left);
        }
        if !approved.is_empty() {
            *inner.granted.entry(plugin_id.to_string()).or_default() |= approved;
            self.save(&inner);
        }
        approved
    }

    /// Drop the whole pending request.
    pub fn reject_pending(&self, plugin_id: &str) {
        self.lock().pending.remove(plugin_id);
    }

    /// Forget a plugin entirely (uninstall).
    pub fn remove_plugin(&self, plugin_id: &str) {
        let mut inner = self.lock();
        inner.granted.remove(plugin_id);
        inner.pending.remove(plugin_id);
        inner.manifests.remove(plugin_id);
        self.save(&inner);
    }
}

impl PermissionChecker for PluginPermissionManager {
    /// C# `CheckPermission`.
    fn check_permission(&self, request: &PermissionRequest) -> PermissionCheckResult {
        let granted = self.permissions(&request.plugin_id);
        if granted.contains(request.permission) {
            return PermissionCheckResult::allow();
        }
        let denied = request.permission & !granted;
        tracing::warn!(
            plugin_id = %request.plugin_id, %denied, resource = ?request.resource,
            "permission denied"
        );
        let event = PermissionDenied {
            plugin_id: request.plugin_id.clone(),
            requested_permission: request.permission,
            resource: request.resource.clone(),
            timestamp: Utc::now(),
        };
        let handlers = self.lock().handlers.clone();
        for h in &handlers {
            h(&event);
        }
        PermissionCheckResult::deny(denied, Some(format!("Permission denied: {denied}")))
    }
}

/// C# `PluginPermissionException`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginPermissionError {
    pub plugin_id: String,
    pub denied_permissions: PluginPermission,
    pub message: String,
}

impl PluginPermissionError {
    #[must_use]
    pub fn new(plugin_id: &str, denied: PluginPermission, message: Option<&str>) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            denied_permissions: denied,
            message: message.map_or_else(
                || format!("Plugin {plugin_id} does not have permission: {denied}"),
                str::to_string,
            ),
        }
    }
}

impl fmt::Display for PluginPermissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for PluginPermissionError {}

impl From<PluginPermissionError> for AppError {
    fn from(e: PluginPermissionError) -> Self {
        Self::Plugin {
            code: ErrorCode::Forbidden,
            message: e.message,
            plugin_id: Some(e.plugin_id),
        }
    }
}

/// C# `PluginSandbox` — permission gate in front of an action.
pub struct PluginSandbox {
    plugin_id: String,
    checker: Arc<dyn PermissionChecker>,
}

impl PluginSandbox {
    #[must_use]
    pub fn new(plugin_id: &str, checker: Arc<dyn PermissionChecker>) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            checker,
        }
    }

    /// C# `RequirePermission`.
    ///
    /// # Errors
    /// [`PluginPermissionError`] with the denied bits.
    pub fn require_permission(
        &self,
        permission: PluginPermission,
        resource: Option<&str>,
    ) -> Result<(), PluginPermissionError> {
        let r = self.checker.check_permission(&PermissionRequest {
            plugin_id: self.plugin_id.clone(),
            permission,
            resource: resource.map(str::to_string),
            reason: None,
        });
        if r.granted {
            Ok(())
        } else {
            Err(PluginPermissionError::new(
                &self.plugin_id,
                r.denied_permissions,
                r.message.as_deref(),
            ))
        }
    }

    /// C# `ExecuteWithPermission` (the async overload is the same call in Rust).
    ///
    /// # Errors
    /// Denied permission; `action` is not run.
    pub fn execute_with_permission<T>(
        &self,
        permission: PluginPermission,
        action: impl FnOnce() -> T,
        resource: Option<&str>,
    ) -> Result<T, PluginPermissionError> {
        self.require_permission(permission, resource)?;
        Ok(action())
    }

    /// C# `ExecuteWithPermissionAsync`.
    ///
    /// # Errors
    /// Denied permission; `action` is not started.
    pub async fn execute_with_permission_async<T, F>(
        &self,
        permission: PluginPermission,
        action: impl FnOnce() -> F,
        resource: Option<&str>,
    ) -> Result<T, PluginPermissionError>
    where
        F: std::future::Future<Output = T>,
    {
        self.require_permission(permission, resource)?;
        Ok(action().await)
    }
}
