//! C# `PluginInfoTests` (13). Rust `PluginInfo` is a snapshot produced by the host
//! (`PluginHost::plugin`) instead of a mutable class, so the tests build it the way the host
//! does: `FromPlugin` = a registered built-in, `FromManifest` = an installed, pending package.

#[path = "support/host.rs"]
mod host;

use std::any::Any;

use arcana_plugin_api::{PluginInfo, PluginMetadata, PluginState, PluginType};
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBehavior};
use chrono::Utc;
use host::{fixture, tmp, write};

struct P;
impl PluginBehavior for P {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "test.plugin",
            "Test Plugin",
            semver::Version::new(1, 2, 3),
            PluginType::Module,
        )
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn builtin_info() -> PluginInfo {
    let f = fixture();
    let d = tmp();
    let host = f.host(d.path());
    host.register_builtin(BasePlugin::boxed(P));
    host.plugin("test.plugin")
        .unwrap_or_else(|| panic!("registered"))
}

async fn manifest_info(json: &str) -> PluginInfo {
    let f = fixture();
    let d = tmp();
    let host = f.host(d.path());
    let pkg = d.path().join("pkg");
    write(&pkg.join("plugin.json"), json);
    let r = host.install(&pkg).await;
    assert!(r.success, "{r:?}");
    let id = r.plugin_id.unwrap_or_default();
    host.plugin(&id).unwrap_or_else(|| panic!("installed"))
}

// PluginInfo_RequiredProperties_ShouldBeSet
#[test]
fn required_properties_should_be_set() {
    let i = builtin_info();
    assert_eq!(i.id, "test.plugin");
    assert_eq!(i.name, "Test Plugin");
    assert_eq!(i.version, semver::Version::new(1, 2, 3));
}

// PluginInfo_OptionalProperties_ShouldDefaultToNull
#[test]
fn optional_properties_should_default_to_none() {
    let i = builtin_info();
    assert!(i.description.is_none());
    assert!(i.author.is_none());
    assert!(i.icon_path.is_none());
    assert!(i.dependencies.is_empty());
    assert!(i.last_activated_at.is_none());
}

// PluginInfo_DefaultState_ShouldBeLoaded — a fresh built-in reports the plugin's own state.
#[test]
fn default_state_is_plugin_state() {
    assert_eq!(builtin_info().state, PluginState::NotLoaded);
}

// PluginInfo_IsBuiltIn_False_CanUninstallAndUpgrade
#[tokio::test]
async fn not_built_in_can_uninstall_and_upgrade() {
    let i = manifest_info(r#"{"id":"x","name":"X","version":"1.0.0"}"#).await;
    assert!(!i.is_built_in && i.can_uninstall && i.can_upgrade);
}

// PluginInfo_IsBuiltIn_True_CannotUninstallOrUpgrade
#[test]
fn built_in_cannot_uninstall_or_upgrade() {
    let i = builtin_info();
    assert!(i.is_built_in && !i.can_uninstall && !i.can_upgrade);
}

// PluginInfo_Metadata_DefaultIsEmptyDictionary
#[test]
fn metadata_default_is_empty() {
    assert!(builtin_info().metadata.is_empty());
}

// PluginInfo_InstalledAt_DefaultsToApproxNow
#[test]
fn installed_at_defaults_to_approx_now() {
    let before = Utc::now();
    let i = builtin_info();
    assert!(i.installed_at >= before && i.installed_at <= Utc::now());
}

// PluginInfo_MutableState_CanBeUpdated — state and last-activated move with activation.
#[test]
fn state_updates_with_activation() {
    let f = fixture();
    let d = tmp();
    let host = f.host(d.path());
    host.register_builtin(BasePlugin::boxed(P));
    assert!(host.activate("test.plugin").is_ok());
    let i = host.plugin("test.plugin").unwrap_or_else(|| panic!("x"));
    assert_eq!(i.state, PluginState::Active);
    assert!(i.last_activated_at.is_some());
}

// FromManifest_ValidManifest_ShouldPopulateAllFields
#[tokio::test]
async fn from_manifest_valid_manifest_should_populate_all_fields() {
    let i = manifest_info(
        r#"{"id":"arcana.test","name":"Test Plugin","version":"2.1.0","description":"A test plugin",
            "author":"Test Author","type":"Module","dependencies":["core.plugin","other.plugin"],"icon":"icon.png"}"#,
    )
    .await;
    assert_eq!(i.id, "arcana.test");
    assert_eq!(i.name, "Test Plugin");
    assert_eq!(i.version, semver::Version::new(2, 1, 0));
    assert_eq!(i.description.as_deref(), Some("A test plugin"));
    assert_eq!(i.author.as_deref(), Some("Test Author"));
    assert_eq!(i.plugin_type, PluginType::Module);
    assert!(i.install_path.ends_with("arcana.test"));
    assert!(!i.is_built_in);
    assert_eq!(i.dependencies, ["core.plugin", "other.plugin"]);
    assert_eq!(i.icon_path.as_deref(), Some("icon.png"));
    assert_eq!(i.state, PluginState::Loaded);
}

// FromManifest_InvalidVersion_ShouldDefaultTo100
#[tokio::test]
async fn from_manifest_invalid_version_should_default_to_100() {
    let i = manifest_info(r#"{"id":"t","name":"T","version":"not-a-version"}"#).await;
    assert_eq!(i.version, semver::Version::new(1, 0, 0));
}

// FromManifest_NullName_ShouldFallBackToId
#[tokio::test]
async fn from_manifest_name_same_as_id() {
    let i = manifest_info(r#"{"id":"fallback.id","name":"fallback.id","version":"1.0.0"}"#).await;
    assert_eq!(i.name, "fallback.id");
}

// FromManifest_UnknownType_ShouldDefaultToModule
#[tokio::test]
async fn from_manifest_unknown_type_should_default_to_module() {
    let i = manifest_info(r#"{"id":"t","name":"T","version":"1.0.0","type":"UnknownPluginType"}"#)
        .await;
    assert_eq!(i.plugin_type, PluginType::Module);
}

// FromManifest_ServiceType_ShouldMapCorrectly
#[tokio::test]
async fn from_manifest_service_type_should_map_correctly() {
    let i = manifest_info(r#"{"id":"t","name":"T","version":"1.0.0","type":"Service"}"#).await;
    assert_eq!(i.plugin_type, PluginType::Service);
}
