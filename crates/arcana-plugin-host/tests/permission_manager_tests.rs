//! C# `PluginPermissionManagerTests.cs` (43): `PluginPermissionManagerTests` (26),
//! `PluginSandboxTests` (8), `PluginPermissionExceptionTests` (2), `PermissionCheckResultTests`
//! (2), `PluginPermissionFlagsTests` (4) — plus Rust-only pending-approval tests.
#![allow(clippy::unwrap_used)]

use std::sync::{Arc, Mutex};

use arcana_plugin_api::{
    PermissionCheckResult, PermissionRequest, PluginPermission as P, PluginPermissionManifest,
};
use arcana_plugin_host::permission::{
    PermissionChecker, PermissionDenied, PluginPermissionError, PluginPermissionManager,
    PluginSandbox,
};

struct Fx {
    dir: tempfile::TempDir,
    m: PluginPermissionManager,
}

fn fx() -> Fx {
    let dir = tempfile::tempdir().unwrap();
    let m = PluginPermissionManager::new(dir.path());
    Fx { dir, m }
}

fn req(id: &str, p: P, resource: Option<&str>) -> PermissionRequest {
    PermissionRequest {
        plugin_id: id.into(),
        permission: p,
        resource: resource.map(Into::into),
        reason: None,
    }
}

// ---- PluginPermissionManagerTests ----

/// `HasPermission_PluginWithNoPermissions_ShouldReturnFalse`
#[test]
fn has_permission_plugin_with_no_permissions_should_return_false() {
    assert!(!fx().m.has_permission("unknown-plugin", P::NETWORK_ACCESS));
}

/// `HasPermission_GrantedPermission_ShouldReturnTrue`
#[test]
fn has_permission_granted_permission_should_return_true() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    assert!(f.m.has_permission("test-plugin", P::NETWORK_ACCESS));
}

/// `HasPermission_PartialPermission_ShouldReturnFalse`
#[test]
fn has_permission_partial_permission_should_return_false() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    assert!(!f.m.has_permission("test-plugin", P::NETWORK_ACCESS | P::WEB_SOCKET_ACCESS));
}

/// `HasPermission_MultiplePermissions_ShouldReturnTrue`
#[test]
fn has_permission_multiple_permissions_should_return_true() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    f.m.grant_permission("test-plugin", P::WEB_SOCKET_ACCESS);
    assert!(f.m.has_permission("test-plugin", P::NETWORK_ACCESS | P::WEB_SOCKET_ACCESS));
}

/// `CheckPermission_Granted_ShouldReturnAllow`
#[test]
fn check_permission_granted_should_return_allow() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::READ_LOCAL_DATA);
    let r = f.m.check_permission(&req(
        "test-plugin",
        P::READ_LOCAL_DATA,
        Some("/data/file.txt"),
    ));
    assert!(r.granted);
    assert_eq!(r.denied_permissions, P::NONE);
}

/// `CheckPermission_Denied_ShouldReturnDeny`
#[test]
fn check_permission_denied_should_return_deny() {
    let r = fx().m.check_permission(&req(
        "test-plugin",
        P::NETWORK_ACCESS,
        Some("https://example.com"),
    ));
    assert!(!r.granted);
    assert_eq!(r.denied_permissions, P::NETWORK_ACCESS);
    assert!(r.message.unwrap().contains("Permission denied"));
}

/// `CheckPermission_Denied_ShouldRaiseEvent`
#[test]
fn check_permission_denied_should_raise_event() {
    let f = fx();
    let captured: Arc<Mutex<Option<PermissionDenied>>> = Arc::default();
    let c = captured.clone();
    f.m.on_permission_denied(Arc::new(move |e| *c.lock().unwrap() = Some(e.clone())));
    let _ =
        f.m.check_permission(&req("test-plugin", P::EXECUTE_PROCESS, Some("cmd.exe")));
    let e = captured.lock().unwrap().clone().unwrap();
    assert_eq!(e.plugin_id, "test-plugin");
    assert_eq!(e.requested_permission, P::EXECUTE_PROCESS);
    assert_eq!(e.resource.as_deref(), Some("cmd.exe"));
}

/// `CheckPermission_PartiallyDenied_ShouldReturnOnlyDeniedPermissions`
#[test]
fn check_permission_partially_denied_should_return_only_denied_permissions() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    let r = f.m.check_permission(&req(
        "test-plugin",
        P::NETWORK_ACCESS | P::EXECUTE_PROCESS,
        None,
    ));
    assert!(!r.granted);
    assert_eq!(r.denied_permissions, P::EXECUTE_PROCESS);
}

/// `GrantPermissionAsync_ShouldAddPermission`
#[test]
fn grant_permission_should_add_permission() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    assert_eq!(f.m.permissions("test-plugin"), P::NETWORK_ACCESS);
}

/// `GrantPermissionAsync_Multiple_ShouldCombine`
#[test]
fn grant_permission_multiple_should_combine() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    f.m.grant_permission("test-plugin", P::WEB_SOCKET_ACCESS);
    assert_eq!(
        f.m.permissions("test-plugin"),
        P::NETWORK_ACCESS | P::WEB_SOCKET_ACCESS
    );
}

/// `GrantPermissionAsync_SamePermissionTwice_ShouldNotDuplicate`
#[test]
fn grant_permission_same_permission_twice_should_not_duplicate() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    assert_eq!(f.m.permissions("test-plugin"), P::NETWORK_ACCESS);
}

/// `GrantPermissionAsync_ShouldPersistToFile`
#[test]
fn grant_permission_should_persist_to_file() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    let path = f.dir.path().join("plugin_permissions.json");
    assert!(path.exists());
    assert!(
        std::fs::read_to_string(path)
            .unwrap()
            .contains("test-plugin")
    );
}

/// `RevokePermissionAsync_ShouldRemovePermission`
#[test]
fn revoke_permission_should_remove_permission() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS | P::WEB_SOCKET_ACCESS);
    f.m.revoke_permission("test-plugin", P::NETWORK_ACCESS);
    assert_eq!(f.m.permissions("test-plugin"), P::WEB_SOCKET_ACCESS);
}

/// `RevokePermissionAsync_NonExistingPermission_ShouldNotThrow`
#[test]
fn revoke_permission_non_existing_permission_should_not_throw() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    f.m.revoke_permission("test-plugin", P::EXECUTE_PROCESS);
    assert_eq!(f.m.permissions("test-plugin"), P::NETWORK_ACCESS);
}

/// `RevokePermissionAsync_AllPermissions_ShouldResultInNone`
#[test]
fn revoke_permission_all_permissions_should_result_in_none() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    f.m.revoke_permission("test-plugin", P::NETWORK_ACCESS);
    assert_eq!(f.m.permissions("test-plugin"), P::NONE);
}

/// `SetPermissionsAsync_ShouldReplaceAllPermissions`
#[test]
fn set_permissions_should_replace_all_permissions() {
    let f = fx();
    f.m.grant_permission("test-plugin", P::NETWORK_ACCESS);
    f.m.set_permissions("test-plugin", P::READ_DATABASE);
    let p = f.m.permissions("test-plugin");
    assert_eq!(p, P::READ_DATABASE);
    assert!(!p.contains(P::NETWORK_ACCESS));
}

/// `SetPermissionsAsync_BasicPlugin_ShouldSetMultiple`
#[test]
fn set_permissions_basic_plugin_should_set_multiple() {
    let f = fx();
    f.m.set_permissions("test-plugin", P::BASIC_PLUGIN);
    let p = f.m.permissions("test-plugin");
    assert!(p.contains(P::READ_LOCAL_DATA));
    assert!(p.contains(P::WRITE_LOCAL_DATA));
    assert!(p.contains(P::CREATE_DIALOGS));
}

/// `GetManifest_Unregistered_ShouldReturnNull`
#[test]
fn get_manifest_unregistered_should_return_null() {
    assert!(fx().m.manifest("unknown-plugin").is_none());
}

/// `RegisterManifest_ShouldStoreManifest`
#[test]
fn register_manifest_should_store_manifest() {
    let f = fx();
    let mut m = PluginPermissionManifest {
        required_permissions: P::NETWORK_ACCESS,
        optional_permissions: P::WEB_SOCKET_ACCESS,
        ..Default::default()
    };
    m.permission_reasons
        .insert("NetworkAccess".into(), "Required for API calls".into());
    f.m.register_manifest("test-plugin", m);
    let got = f.m.manifest("test-plugin").unwrap();
    assert_eq!(got.required_permissions, P::NETWORK_ACCESS);
    assert_eq!(got.optional_permissions, P::WEB_SOCKET_ACCESS);
}

/// `RegisterManifestFromType_ShouldExtractAttributes` — attributes become explicit lists.
#[test]
fn register_manifest_from_type_should_extract_attributes() {
    let f = fx();
    f.m.register_manifest_from_attributes(
        "test-plugin",
        &[
            (P::NETWORK_ACCESS, Some("Required for API calls")),
            (P::READ_DATABASE, Some("Required for data access")),
        ],
        &[(P::WEB_SOCKET_ACCESS, Some("Enables real-time updates"))],
    );
    let m = f.m.manifest("test-plugin").unwrap();
    assert!(m.required_permissions.contains(P::NETWORK_ACCESS));
    assert!(m.required_permissions.contains(P::READ_DATABASE));
    assert!(m.optional_permissions.contains(P::WEB_SOCKET_ACCESS));
    assert_eq!(m.permission_reasons.len(), 3);
}

/// `ValidateRequiredPermissions_NoManifest_ShouldReturnAllow`
#[test]
fn validate_required_permissions_no_manifest_should_return_allow() {
    assert!(
        fx().m
            .validate_required_permissions("unregistered-plugin")
            .granted
    );
}

fn net_db_manifest(f: &Fx) {
    f.m.register_manifest(
        "test-plugin",
        PluginPermissionManifest {
            required_permissions: P::NETWORK_ACCESS | P::READ_DATABASE,
            ..Default::default()
        },
    );
}

/// `ValidateRequiredPermissions_AllRequired_ShouldReturnAllow`
#[test]
fn validate_required_permissions_all_required_should_return_allow() {
    let f = fx();
    net_db_manifest(&f);
    f.m.set_permissions("test-plugin", P::NETWORK_ACCESS | P::READ_DATABASE);
    assert!(f.m.validate_required_permissions("test-plugin").granted);
}

/// `ValidateRequiredPermissions_MissingRequired_ShouldReturnDeny`
#[test]
fn validate_required_permissions_missing_required_should_return_deny() {
    let f = fx();
    net_db_manifest(&f);
    f.m.set_permissions("test-plugin", P::NETWORK_ACCESS);
    let r = f.m.validate_required_permissions("test-plugin");
    assert!(!r.granted);
    assert_eq!(r.denied_permissions, P::READ_DATABASE);
    assert!(r.message.unwrap().contains("Missing required permissions"));
}

/// `GrantBuiltInPermissionsAsync_ShouldGrantFullAccess`
#[test]
fn grant_built_in_permissions_should_grant_full_access() {
    let f = fx();
    f.m.grant_built_in_permissions("builtin-plugin");
    assert_eq!(f.m.permissions("builtin-plugin"), P::FULL_ACCESS);
}

/// `GrantDefaultPermissionsAsync_WithManifest_ShouldGrantRequired`
#[test]
fn grant_default_permissions_with_manifest_should_grant_required() {
    let f = fx();
    f.m.register_manifest(
        "test-plugin",
        PluginPermissionManifest {
            required_permissions: P::NETWORK_ACCESS | P::READ_DATABASE,
            optional_permissions: P::WEB_SOCKET_ACCESS,
            ..Default::default()
        },
    );
    f.m.grant_default_permissions("test-plugin");
    let p = f.m.permissions("test-plugin");
    assert_eq!(p, P::NETWORK_ACCESS | P::READ_DATABASE);
    assert!(!p.contains(P::WEB_SOCKET_ACCESS));
}

/// `GrantDefaultPermissionsAsync_NoManifest_ShouldGrantBasic`
#[test]
fn grant_default_permissions_no_manifest_should_grant_basic() {
    let f = fx();
    f.m.grant_default_permissions("test-plugin");
    assert_eq!(f.m.permissions("test-plugin"), P::BASIC_PLUGIN);
}

/// `LoadPermissions_ShouldRestoreFromFile`
#[test]
fn load_permissions_should_restore_from_file() {
    let f = fx();
    f.m.grant_permission("plugin1", P::NETWORK_ACCESS);
    f.m.grant_permission("plugin2", P::READ_DATABASE | P::WRITE_DATABASE);
    let again = PluginPermissionManager::new(f.dir.path());
    assert_eq!(again.permissions("plugin1"), P::NETWORK_ACCESS);
    assert_eq!(
        again.permissions("plugin2"),
        P::READ_DATABASE | P::WRITE_DATABASE
    );
}

// ---- PluginSandboxTests (Moq → a scripted checker) ----

struct Fake {
    result: PermissionCheckResult,
    seen: Mutex<Option<PermissionRequest>>,
}

fn sandbox(result: PermissionCheckResult) -> (PluginSandbox, Arc<Fake>) {
    let fake = Arc::new(Fake {
        result,
        seen: Mutex::new(None),
    });
    (PluginSandbox::new("test-plugin", fake.clone()), fake)
}

impl PermissionChecker for Fake {
    fn check_permission(&self, request: &PermissionRequest) -> PermissionCheckResult {
        *self.seen.lock().unwrap() = Some(request.clone());
        self.result.clone()
    }
}

/// `RequirePermission_Granted_ShouldNotThrow`
#[test]
fn require_permission_granted_should_not_throw() {
    let (s, _) = sandbox(PermissionCheckResult::allow());
    assert!(s.require_permission(P::NETWORK_ACCESS, None).is_ok());
}

/// `RequirePermission_Denied_ShouldThrowPluginPermissionException`
#[test]
fn require_permission_denied_should_throw_plugin_permission_exception() {
    let (s, _) = sandbox(PermissionCheckResult::deny(
        P::NETWORK_ACCESS,
        Some("Permission denied".into()),
    ));
    let e = s.require_permission(P::NETWORK_ACCESS, None).unwrap_err();
    assert_eq!(e.plugin_id, "test-plugin");
    assert_eq!(e.denied_permissions, P::NETWORK_ACCESS);
}

/// `RequirePermission_WithResource_ShouldPassResourceToManager`
#[test]
fn require_permission_with_resource_should_pass_resource_to_manager() {
    let (s, fake) = sandbox(PermissionCheckResult::allow());
    s.require_permission(P::READ_FILE_SYSTEM, Some("/path/to/file"))
        .unwrap();
    let seen = fake.seen.lock().unwrap().clone().unwrap();
    assert_eq!(seen.resource.as_deref(), Some("/path/to/file"));
}

/// `ExecuteWithPermission_Granted_ShouldExecuteAction`
#[test]
fn execute_with_permission_granted_should_execute_action() {
    let (s, _) = sandbox(PermissionCheckResult::allow());
    let mut executed = false;
    s.execute_with_permission(
        P::NETWORK_ACCESS,
        || {
            executed = true;
            42
        },
        None,
    )
    .unwrap();
    assert!(executed);
}

/// `ExecuteWithPermission_Granted_ShouldReturnResult`
#[test]
fn execute_with_permission_granted_should_return_result() {
    let (s, _) = sandbox(PermissionCheckResult::allow());
    assert_eq!(
        s.execute_with_permission(P::NETWORK_ACCESS, || 42, None)
            .unwrap(),
        42
    );
}

/// `ExecuteWithPermission_Denied_ShouldNotExecuteAction`
#[test]
fn execute_with_permission_denied_should_not_execute_action() {
    let (s, _) = sandbox(PermissionCheckResult::deny(P::NETWORK_ACCESS, None));
    let mut executed = false;
    let r = s.execute_with_permission(
        P::NETWORK_ACCESS,
        || {
            executed = true;
            42
        },
        None,
    );
    assert!(r.is_err());
    assert!(!executed);
}

/// `ExecuteWithPermissionAsync_Granted_ShouldExecuteAction`
#[tokio::test]
async fn execute_with_permission_async_granted_should_execute_action() {
    let (s, _) = sandbox(PermissionCheckResult::allow());
    let r = s
        .execute_with_permission_async(
            P::NETWORK_ACCESS,
            || async {
                tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                "result"
            },
            None,
        )
        .await
        .unwrap();
    assert_eq!(r, "result");
}

/// `ExecuteWithPermissionAsync_Denied_ShouldThrow`
#[tokio::test]
async fn execute_with_permission_async_denied_should_throw() {
    let (s, _) = sandbox(PermissionCheckResult::deny(P::NETWORK_ACCESS, None));
    let r = s
        .execute_with_permission_async(P::NETWORK_ACCESS, || async { "result" }, None)
        .await;
    assert!(r.is_err());
}

// ---- PluginPermissionExceptionTests ----

/// `Constructor_ShouldSetProperties`
#[test]
fn constructor_should_set_properties() {
    let e = PluginPermissionError::new(
        "test-plugin",
        P::NETWORK_ACCESS | P::EXECUTE_PROCESS,
        Some("Custom message"),
    );
    assert_eq!(e.plugin_id, "test-plugin");
    assert_eq!(e.denied_permissions, P::NETWORK_ACCESS | P::EXECUTE_PROCESS);
    assert_eq!(e.to_string(), "Custom message");
}

/// `Constructor_WithoutMessage_ShouldGenerateDefaultMessage`
#[test]
fn constructor_without_message_should_generate_default_message() {
    let e = PluginPermissionError::new("test-plugin", P::NETWORK_ACCESS, None);
    assert!(e.message.contains("test-plugin"));
    assert!(e.message.contains("NetworkAccess"));
}

// ---- PermissionCheckResultTests ----

/// `Allow_ShouldReturnGrantedResult`
#[test]
fn allow_should_return_granted_result() {
    let r = PermissionCheckResult::allow();
    assert!(r.granted);
    assert_eq!(r.denied_permissions, P::NONE);
    assert!(r.message.is_none());
}

/// `Deny_ShouldReturnDeniedResult`
#[test]
fn deny_should_return_denied_result() {
    let r = PermissionCheckResult::deny(P::NETWORK_ACCESS, Some("Access denied".into()));
    assert!(!r.granted);
    assert_eq!(r.denied_permissions, P::NETWORK_ACCESS);
    assert_eq!(r.message.as_deref(), Some("Access denied"));
}

// ---- PluginPermissionFlagsTests ----

/// `BasicPlugin_ShouldIncludeExpectedPermissions`
#[test]
fn basic_plugin_should_include_expected_permissions() {
    for p in [
        P::READ_LOCAL_DATA,
        P::WRITE_LOCAL_DATA,
        P::CREATE_DIALOGS,
        P::INTER_PLUGIN_COMM,
        P::REGISTER_MENUS,
        P::REGISTER_VIEWS,
    ] {
        assert!(P::BASIC_PLUGIN.contains(p));
    }
}

/// `BasicPlugin_ShouldNotIncludeSensitivePermissions`
#[test]
fn basic_plugin_should_not_include_sensitive_permissions() {
    for p in [P::NETWORK_ACCESS, P::EXECUTE_PROCESS, P::ACCESS_CREDENTIALS] {
        assert!(!P::BASIC_PLUGIN.contains(p));
    }
}

/// `NetworkPlugin_ShouldIncludeBasicPlusNetwork`
#[test]
fn network_plugin_should_include_basic_plus_network() {
    assert!(P::NETWORK_PLUGIN.contains(P::READ_LOCAL_DATA));
    assert!(P::NETWORK_PLUGIN.contains(P::NETWORK_ACCESS));
}

/// `FullAccess_ShouldIncludeAllPermissions`
#[test]
fn full_access_should_include_all_permissions() {
    for p in [
        P::READ_LOCAL_DATA,
        P::NETWORK_ACCESS,
        P::EXECUTE_PROCESS,
        P::ACCESS_CREDENTIALS,
        P::ACCESS_HARDWARE,
    ] {
        assert!(P::FULL_ACCESS.contains(p));
    }
}

// ---- Rust additions: pending approval (WASM manifests) ----

/// Requested permissions are pending, not granted, until approved.
#[test]
fn requested_permissions_are_pending_until_approved() {
    let f = fx();
    let pending =
        f.m.request_permissions("wasm", P::NETWORK_ACCESS | P::READ_FILE_SYSTEM);
    assert_eq!(pending, P::NETWORK_ACCESS | P::READ_FILE_SYSTEM);
    assert_eq!(f.m.permissions("wasm"), P::NONE);
    assert_eq!(f.m.approve("wasm", P::NETWORK_ACCESS), P::NETWORK_ACCESS);
    assert_eq!(f.m.permissions("wasm"), P::NETWORK_ACCESS);
    assert_eq!(f.m.pending("wasm"), P::READ_FILE_SYSTEM);
}

/// Approval cannot widen beyond what was requested.
#[test]
fn approve_cannot_grant_unrequested_permissions() {
    let f = fx();
    let _ = f.m.request_permissions("wasm", P::NETWORK_ACCESS);
    assert_eq!(f.m.approve("wasm", P::FULL_ACCESS), P::NETWORK_ACCESS);
    assert_eq!(f.m.permissions("wasm"), P::NETWORK_ACCESS);
    assert_eq!(f.m.approve("other", P::FULL_ACCESS), P::NONE);
}

/// Rejecting clears the request; removing forgets grants.
#[test]
fn reject_and_remove_clear_state() {
    let f = fx();
    let _ = f.m.request_permissions("wasm", P::NETWORK_ACCESS);
    f.m.reject_pending("wasm");
    assert_eq!(f.m.pending("wasm"), P::NONE);
    f.m.grant_permission("wasm", P::READ_LOCAL_DATA);
    f.m.remove_plugin("wasm");
    assert_eq!(f.m.permissions("wasm"), P::NONE);
}

/// Permission errors map to `AppError::Plugin` / `Forbidden`.
#[test]
fn permission_error_converts_to_app_error() {
    let e: arcana_core::AppError = PluginPermissionError::new("p", P::NETWORK_ACCESS, None).into();
    assert_eq!(e.code(), arcana_core::ErrorCode::Forbidden);
}
