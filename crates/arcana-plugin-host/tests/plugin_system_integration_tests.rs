//! C# `PluginSystemIntegrationTests` (11). EF in-memory DB → `InMemoryPluginVersionStore`;
//! `PluginLoadContextManager` → `PluginHost` with a fake loader.

use std::any::Any;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use arcana_core::AppResult;
use arcana_plugin_api::{
    HealthState, PermissionRequest, Plugin, PluginHealthStatus, PluginManifest, PluginMetadata,
    PluginPermission, PluginPermissionManifest, PluginVersionInfo,
};
use arcana_plugin_host::dependency::DependencyResolver;
use arcana_plugin_host::loader::PluginLoader;
use arcana_plugin_host::permission::{PermissionChecker, PluginPermissionManager, PluginSandbox};
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBehavior};
use arcana_plugin_host::version::{PluginDependency, SemanticVersion};
use arcana_plugin_host::version_store::{InMemoryPluginVersionStore, PluginVersionStore};
use chrono::{Duration, Utc};

#[path = "support/host.rs"]
mod host;

fn v(s: &str) -> semver::Version {
    semver::Version::parse(s).unwrap_or_else(|e| panic!("{e}"))
}

fn info(id: &str, ver: &str, days_ago: i64, current: bool) -> PluginVersionInfo {
    PluginVersionInfo {
        plugin_id: id.into(),
        version: v(ver),
        installed_at: Utc::now() - Duration::days(days_ago),
        backup_path: format!("/backups/{id}/{ver}"),
        is_current: current,
        size_bytes: 0,
        release_notes: None,
    }
}

fn ok<T>(r: AppResult<T>) -> T {
    r.unwrap_or_else(|e| panic!("{e}"))
}

// PluginInstallation_ShouldSetPermissionsAndRecordVersion
#[tokio::test]
async fn plugin_installation_should_set_permissions_and_record_version() {
    let d = host::tmp();
    let pm = PluginPermissionManager::new(d.path());
    let store = InMemoryPluginVersionStore::new();
    pm.register_manifest(
        "test-plugin",
        PluginPermissionManifest {
            required_permissions: PluginPermission::NETWORK_ACCESS
                | PluginPermission::READ_DATABASE,
            optional_permissions: PluginPermission::WEB_SOCKET_ACCESS,
            ..PluginPermissionManifest::default()
        },
    );
    pm.grant_default_permissions("test-plugin");
    ok(store
        .add_version(&info("test-plugin", "1.0.0", 0, true))
        .await);
    assert_eq!(
        pm.permissions("test-plugin"),
        PluginPermission::NETWORK_ACCESS | PluginPermission::READ_DATABASE
    );
    let cur = ok(store.current_version("test-plugin").await);
    assert_eq!(cur.map(|c| c.version), Some(v("1.0.0")));
}

// PluginUpgrade_ShouldUpdateVersionAndPreservePermissions
#[tokio::test]
async fn plugin_upgrade_should_update_version_and_preserve_permissions() {
    let d = host::tmp();
    let pm = PluginPermissionManager::new(d.path());
    let store = InMemoryPluginVersionStore::new();
    let perms = PluginPermission::NETWORK_ACCESS | PluginPermission::READ_DATABASE;
    pm.set_permissions("test-plugin", perms);
    ok(store
        .add_version(&info("test-plugin", "1.0.0", 1, true))
        .await);
    ok(store
        .add_version(&info("test-plugin", "2.0.0", 0, true))
        .await);
    assert_eq!(pm.permissions("test-plugin"), perms);
    let cur = ok(store.current_version("test-plugin").await);
    assert_eq!(cur.map(|c| c.version), Some(v("2.0.0")));
    assert_eq!(ok(store.versions("test-plugin").await).len(), 2);
}

// PluginRollback_ShouldRestorePreviousVersion
#[tokio::test]
async fn plugin_rollback_should_restore_previous_version() {
    let store = InMemoryPluginVersionStore::new();
    ok(store.add_version(&info("p", "1.0.0", 2, false)).await);
    ok(store.add_version(&info("p", "2.0.0", 0, true)).await);
    ok(store.set_current_version("p", "1.0.0").await);
    assert_eq!(
        ok(store.current_version("p").await).map(|c| c.version),
        Some(v("1.0.0"))
    );
    assert_eq!(
        ok(store.version("p", "2.0.0").await).map(|c| c.is_current),
        Some(false)
    );
}

fn dep(s: &str) -> PluginDependency {
    ok(PluginDependency::parse(s))
}
fn sv(s: &str) -> SemanticVersion {
    ok(SemanticVersion::parse(s))
}

// DependencyResolver_ShouldResolveComplexDependencyGraph
#[test]
fn dependency_resolver_should_resolve_complex_dependency_graph() {
    let mut r = DependencyResolver::new();
    r.register("core", sv("1.0.0"), vec![]);
    r.register("themes", sv("1.0.0"), vec![dep("core@^1.0.0")]);
    r.register(
        "ui",
        sv("1.0.0"),
        vec![dep("core@^1.0.0"), dep("themes@^1.0.0")],
    );
    r.register(
        "app",
        sv("1.0.0"),
        vec![dep("ui@^1.0.0"), dep("core@^1.0.0")],
    );
    let res = r.resolve();
    assert!(res.success);
    let pos = |id: &str| res.resolved_order.iter().position(|x| x == id);
    assert_eq!(pos("core"), Some(0));
    assert!(pos("themes") > pos("core"));
    assert!(pos("ui") > pos("themes"));
    assert_eq!(pos("app"), Some(3));
}

// DependencyResolver_ShouldDetectVersionConflicts
#[test]
fn dependency_resolver_should_detect_version_conflicts() {
    let mut r = DependencyResolver::new();
    r.register("core", sv("1.5.0"), vec![]);
    r.register("plugin-a", sv("1.0.0"), vec![dep("core@^1.0.0")]);
    r.register("plugin-b", sv("1.0.0"), vec![dep("core@^2.0.0")]);
    let res = r.resolve();
    assert!(!res.success);
    assert!(!res.conflicts.is_empty());
}

struct P(String);
impl PluginBehavior for P {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            &self.0,
            &self.0,
            semver::Version::new(1, 0, 0),
            arcana_plugin_api::PluginType::Module,
        )
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
struct Loader(Mutex<Vec<String>>);
impl PluginLoader for Loader {
    fn can_load(&self, _: &PluginManifest) -> bool {
        true
    }
    fn load(
        &self,
        m: &PluginManifest,
        _: &Path,
        _: PluginPermission,
    ) -> AppResult<Box<dyn Plugin>> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(m.id.clone());
        Ok(BasePlugin::boxed(P(m.id.clone())))
    }
}

async fn host_with(
    ids: &[&str],
) -> (
    host::Fixture,
    tempfile::TempDir,
    Arc<arcana_plugin_host::PluginHost>,
    Arc<Loader>,
) {
    let f = host::fixture();
    let d = host::tmp();
    let h = f.host(d.path());
    let l = Arc::new(Loader::default());
    h.add_loader(l.clone());
    for id in ids {
        let pkg = d.path().join("pkg").join(id);
        host::write(
            &pkg.join("plugin.json"),
            &format!(r#"{{"id":"{id}","name":"{id}","version":"1.0.0","main":"x.bin"}}"#),
        );
        assert!(h.install(&pkg).await.success);
    }
    (f, d, h, l)
}

// PluginLoadContextManager_ShouldManageMultipleContexts
#[tokio::test]
async fn host_should_manage_multiple_plugins() {
    let (_f, _d, h, l) = host_with(&["plugin1", "plugin2"]).await;
    assert!(h.activate("plugin1").is_ok() && h.activate("plugin2").is_ok());
    assert_eq!(
        *l.0.lock().unwrap_or_else(PoisonError::into_inner),
        ["plugin1", "plugin2"]
    );
    assert_eq!(h.all_plugins().len(), 2);
}

// PluginLoadContextManager_ShouldReplaceExistingContext — reload drops the old instance and
// loads a new one under the same id.
#[tokio::test]
async fn host_should_replace_existing_instance_on_reload() {
    let (_f, _d, h, l) = host_with(&["plugin1"]).await;
    assert!(h.activate("plugin1").is_ok());
    assert!(h.reload("plugin1").success);
    assert_eq!(l.0.lock().unwrap_or_else(PoisonError::into_inner).len(), 2);
    assert_eq!(h.all_plugins().len(), 1);
}

// PluginSandbox_ShouldEnforcePermissions
#[test]
fn plugin_sandbox_should_enforce_permissions() {
    let d = host::tmp();
    let pm = Arc::new(PluginPermissionManager::new(d.path()));
    pm.set_permissions("sandboxed-plugin", PluginPermission::BASIC_PLUGIN);
    let sb = PluginSandbox::new("sandboxed-plugin", pm);
    assert!(
        sb.require_permission(PluginPermission::READ_LOCAL_DATA, None)
            .is_ok()
    );
    assert!(
        sb.require_permission(PluginPermission::NETWORK_ACCESS, None)
            .is_err()
    );
}

// PluginPermission_ShouldTrackDeniedAccess
#[test]
fn plugin_permission_should_track_denied_access() {
    let d = host::tmp();
    let pm = PluginPermissionManager::new(d.path());
    let seen = Arc::new(Mutex::new(Vec::new()));
    let s2 = seen.clone();
    pm.on_permission_denied(Arc::new(move |e| {
        s2.lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(e.clone());
    }));
    let _ = pm.check_permission(&PermissionRequest {
        plugin_id: "tracked-plugin".into(),
        permission: PluginPermission::EXECUTE_PROCESS,
        resource: Some("cmd.exe".into()),
        reason: None,
    });
    let seen = seen.lock().unwrap_or_else(PoisonError::into_inner);
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].plugin_id, "tracked-plugin");
    assert_eq!(
        seen[0].requested_permission,
        PluginPermission::EXECUTE_PROCESS
    );
    assert_eq!(seen[0].resource.as_deref(), Some("cmd.exe"));
}

// HealthHistory_ShouldTrackOverTime
#[tokio::test]
async fn health_history_should_track_over_time() {
    let store = InMemoryPluginVersionStore::new();
    let base = Utc::now();
    let states = [
        HealthState::Healthy,
        HealthState::Healthy,
        HealthState::Degraded,
        HealthState::Unhealthy,
        HealthState::Healthy,
    ];
    for (i, s) in states.iter().enumerate() {
        let mut st = PluginHealthStatus::new("m", "Monitored Plugin", *s);
        st.checked_at = base + Duration::minutes(i64::try_from(i).unwrap_or(0));
        ok(store.record_health_snapshot("m", &st).await);
    }
    let h = ok(store
        .health_history(
            "m",
            base - Duration::minutes(1),
            base + Duration::minutes(10),
        )
        .await);
    assert_eq!(h.len(), 5);
    assert_eq!(h[0].state, HealthState::Healthy);
    assert_eq!(h[1].state, HealthState::Unhealthy);
    assert_eq!(h[2].state, HealthState::Degraded);
}

// CompletePluginLifecycle_InstallActivateDeactivateUninstall
#[tokio::test]
async fn complete_plugin_lifecycle_install_activate_deactivate_uninstall() {
    let (_f, _d, h, _l) = host_with(&["lifecycle-plugin"]).await;
    let id = "lifecycle-plugin";
    h.permissions.register_manifest(
        id,
        PluginPermissionManifest {
            required_permissions: PluginPermission::BASIC_PLUGIN,
            optional_permissions: PluginPermission::NETWORK_ACCESS,
            ..PluginPermissionManifest::default()
        },
    );
    h.permissions.grant_default_permissions(id);
    assert!(ok(h.store.current_version(id).await).is_some());
    assert_eq!(
        h.permissions.permissions(id),
        PluginPermission::BASIC_PLUGIN
    );
    assert!(h.activate(id).is_ok());
    assert_eq!(h.check_all_health().await.len(), 1);
    assert_eq!(ok(h.store.health_snapshot_count(id).await), 1);
    assert!(h.deactivate(id).is_ok());
    assert!(h.uninstall(id).await.success);
    assert!(ok(h.store.versions(id).await).is_empty());
    h.permissions.set_permissions(id, PluginPermission::NONE);
    assert_eq!(h.permissions.permissions(id), PluginPermission::NONE);
}
