//! C# `ManifestServiceTests` (35).

use std::path::Path;

use arcana_plugin_api::{
    ActivationEventType, ManifestContributions, ManifestMenuDefinition, ManifestViewDefinition,
    PluginManifest, activation_events,
};
use arcana_plugin_host::manifest_service::{ManifestService, ManifestValidationResult};

fn m(id: &str, name: &str, version: &str) -> PluginManifest {
    PluginManifest {
        id: id.into(),
        name: name.into(),
        version: version.into(),
        plugin_type: "Module".into(),
        ..PluginManifest::default()
    }
}
fn def() -> PluginManifest {
    m("test.plugin", "Test Plugin", "1.0.0")
}
fn with_events(ev: &[&str]) -> PluginManifest {
    let mut x = def();
    x.activation_events = Some(ev.iter().map(|s| (*s).to_string()).collect());
    x
}

// RegisterManifest_ThenGetManifest_ShouldReturnSame
#[test]
fn register_then_get() {
    let s = ManifestService::new();
    s.register_manifest(def(), None);
    assert_eq!(
        s.manifest("test.plugin").map(|m| m.id),
        Some("test.plugin".into())
    );
}

// GetManifest_NotRegistered_ShouldReturnNull
#[test]
fn get_not_registered() {
    assert!(ManifestService::new().manifest("nonexistent").is_none());
}

// RegisterManifest_OverwritesExisting
#[test]
fn register_overwrites() {
    let s = ManifestService::new();
    s.register_manifest(m("test.plugin", "T", "1.0.0"), None);
    s.register_manifest(m("test.plugin", "T", "2.0.0"), None);
    assert_eq!(
        s.manifest("test.plugin").map(|m| m.version),
        Some("2.0.0".into())
    );
}

// RegisterManifest_WithBasePath_ShouldReturnDirectory
#[test]
fn register_with_base_path() {
    let s = ManifestService::new();
    s.register_manifest(def(), Some(Path::new("/plugins/test")));
    assert_eq!(
        s.manifest_directory("test.plugin").as_deref(),
        Some(Path::new("/plugins/test"))
    );
}

// RegisterManifest_WithoutBasePath_DirectoryShouldBeNull
#[test]
fn register_without_base_path() {
    let s = ManifestService::new();
    s.register_manifest(def(), None);
    assert!(s.manifest_directory("test.plugin").is_none());
}

// GetAllManifests_NoManifests_ShouldReturnEmpty
#[test]
fn all_empty() {
    assert!(ManifestService::new().all_manifests().is_empty());
}

// GetAllManifests_MultipleRegistered_ShouldReturnAll
#[test]
fn all_multiple() {
    let s = ManifestService::new();
    for id in ["plugin.a", "plugin.b", "plugin.c"] {
        s.register_manifest(m(id, "X", "1.0.0"), None);
    }
    let ids: Vec<_> = s.all_manifests().into_iter().map(|m| m.id).collect();
    assert_eq!(ids, ["plugin.a", "plugin.b", "plugin.c"]);
}

// GetManifestDirectory_NotRegistered_ShouldReturnNull
#[test]
fn dir_not_registered() {
    assert!(
        ManifestService::new()
            .manifest_directory("unknown")
            .is_none()
    );
}

// ShouldActivateOnStartup_UnknownPlugin_ShouldReturnTrue
#[test]
fn startup_unknown() {
    assert!(ManifestService::new().should_activate_on_startup("nonexistent"));
}

fn startup(x: PluginManifest) -> bool {
    let s = ManifestService::new();
    s.register_manifest(x, None);
    s.should_activate_on_startup("test.plugin")
}

// ShouldActivateOnStartup_NullActivationEvents_ShouldReturnTrue
#[test]
fn startup_null_events() {
    assert!(startup(def()));
}

// ShouldActivateOnStartup_EmptyActivationEvents_ShouldReturnTrue
#[test]
fn startup_empty_events() {
    assert!(startup(with_events(&[])));
}

// ShouldActivateOnStartup_WithOnStartupEvent_ShouldReturnTrue
#[test]
fn startup_on_startup() {
    assert!(startup(with_events(&[activation_events::ON_STARTUP])));
}

// ShouldActivateOnStartup_WithStarEvent_ShouldReturnTrue
#[test]
fn startup_star() {
    assert!(startup(with_events(&[activation_events::STAR])));
}

// ShouldActivateOnStartup_OnlyCommandEvent_ShouldReturnFalse
#[test]
fn startup_command_only() {
    assert!(!startup(with_events(&["onCommand:order.new"])));
}

// GetActivationEvents_UnknownPlugin_ShouldReturnEmpty
#[test]
fn events_unknown() {
    assert!(
        ManifestService::new()
            .activation_events("nonexistent")
            .is_empty()
    );
}

// GetActivationEvents_NullEvents_ShouldReturnEmpty
#[test]
fn events_null() {
    let s = ManifestService::new();
    s.register_manifest(def(), None);
    assert!(s.activation_events("test.plugin").is_empty());
}

// GetActivationEvents_WithEvents_ShouldReturnThem
#[test]
fn events_some() {
    let s = ManifestService::new();
    s.register_manifest(with_events(&["onStartup", "onCommand:test"]), None);
    assert_eq!(
        s.activation_events("test.plugin"),
        ["onStartup", "onCommand:test"]
    );
}

fn for_event(x: PluginManifest, arg: Option<&str>) -> Vec<String> {
    let s = ManifestService::new();
    s.register_manifest(x, None);
    s.plugins_for_activation_event(ActivationEventType::OnCommand, arg)
}

// GetPluginsForActivationEvent_NoMatches_ShouldReturnEmpty
#[test]
fn for_event_none() {
    assert!(for_event(def(), Some("test")).is_empty());
}

// GetPluginsForActivationEvent_StarMatch_ShouldReturnAll
#[test]
fn for_event_star() {
    assert_eq!(for_event(with_events(&["*"]), Some("any")), ["test.plugin"]);
}

// GetPluginsForActivationEvent_CommandMatch_ShouldReturn
#[test]
fn for_event_command() {
    assert_eq!(
        for_event(with_events(&["onCommand:order.new"]), Some("order.new")),
        ["test.plugin"]
    );
}

// GetPluginsForActivationEvent_CommandNoArgMatch_ShouldReturn
#[test]
fn for_event_null_arg() {
    assert_eq!(
        for_event(with_events(&["onCommand:order.new"]), None),
        ["test.plugin"]
    );
}

fn validate(x: &PluginManifest) -> ManifestValidationResult {
    ManifestService::new().validate_manifest(x)
}

// ValidateManifest_ValidManifest_ShouldBeValid
#[test]
fn validate_valid() {
    let r = validate(&m("test", "Test Plugin", "1.0.0"));
    assert!(r.is_valid && r.errors.is_empty());
}

// ValidateManifest_MissingId_ShouldHaveError
#[test]
fn validate_missing_id() {
    let r = validate(&m("", "Test", "1.0.0"));
    assert!(!r.is_valid && r.errors.iter().any(|e| e.contains("id")));
}

// ValidateManifest_MissingName_ShouldHaveError
#[test]
fn validate_missing_name() {
    let r = validate(&m("test", "", "1.0.0"));
    assert!(!r.is_valid && r.errors.iter().any(|e| e.contains("name")));
}

// ValidateManifest_MissingVersion_ShouldHaveError
#[test]
fn validate_missing_version() {
    let r = validate(&m("test", "Test", ""));
    assert!(!r.is_valid && r.errors.iter().any(|e| e.contains("version")));
}

// ValidateManifest_InvalidVersion_ShouldHaveWarning
#[test]
fn validate_invalid_version() {
    let r = validate(&m("test.plugin", "T", "not-a-version"));
    assert!(r.is_valid && r.warnings.iter().any(|w| w.contains("version")));
}

// ValidateManifest_UnknownActivationEvent_ShouldHaveWarning
#[test]
fn validate_unknown_event() {
    let r = validate(&with_events(&["unknownEvent"]));
    assert!(
        r.warnings
            .iter()
            .any(|w| w.contains("Unknown activation event"))
    );
}

// ValidateManifest_ViewMissingId_ShouldHaveError
#[test]
fn validate_view_missing_id() {
    let mut x = def();
    x.contributes = Some(ManifestContributions {
        views: Some(vec![ManifestViewDefinition {
            title_key: "view.title".into(),
            ..ManifestViewDefinition::default()
        }]),
        ..ManifestContributions::default()
    });
    let r = validate(&x);
    assert!(!r.is_valid && r.errors.iter().any(|e| e.contains("View")));
}

// ValidateManifest_MenuMissingLocation_ShouldHaveError
#[test]
fn validate_menu_missing_location() {
    let mut x = def();
    x.contributes = Some(ManifestContributions {
        menus: Some(vec![ManifestMenuDefinition {
            id: "test.menu".into(),
            ..ManifestMenuDefinition::default()
        }]),
        ..ManifestContributions::default()
    });
    let r = validate(&x);
    assert!(!r.is_valid && r.errors.iter().any(|e| e.contains("location")));
}

// LoadManifestAsync_FileNotFound_ShouldReturnNull
#[test]
fn load_missing_file() {
    assert!(
        ManifestService::new()
            .load_manifest(Path::new("/tmp/nonexistent-arcana.json"))
            .is_none()
    );
}

fn load_text(json: &str) -> Option<PluginManifest> {
    let dir = tempfile::tempdir().ok()?;
    let p = dir.path().join("plugin.json");
    std::fs::write(&p, json).ok()?;
    ManifestService::new().load_manifest(&p)
}

// LoadManifestAsync_ValidJsonFile_ShouldReturnManifest
#[test]
fn load_valid() {
    let r = load_text(r#"{"id":"file.plugin","name":"File Plugin","version":"1.0.0"}"#);
    assert_eq!(
        r.map(|m| (m.id, m.name)),
        Some(("file.plugin".into(), "File Plugin".into()))
    );
}

// LoadManifestAsync_InvalidJson_ShouldReturnNull
#[test]
fn load_invalid_json() {
    assert!(load_text("{ this is not valid json }").is_none());
}

// LoadManifestAsync_MissingRequiredId_ShouldReturnNull
#[test]
fn load_missing_id() {
    assert!(load_text(r#"{"name":"No ID Plugin","version":"1.0.0"}"#).is_none());
}

// DiscoverManifestsAsync_DirectoryNotFound_ShouldReturnEmpty
#[test]
fn discover_missing_dir() {
    assert!(
        ManifestService::new()
            .discover_manifests(Path::new("/tmp/nonexistent-plugins-dir-xyz"))
            .is_empty()
    );
}

// DiscoverManifestsAsync_DirectoryWithManifests_ShouldLoadAll
#[test]
fn discover_loads_all() -> std::io::Result<()> {
    let dir = tempfile::tempdir()?;
    for i in 1..=2 {
        let d = dir.path().join(format!("plugin{i}"));
        std::fs::create_dir(&d)?;
        std::fs::write(
            d.join("plugin.manifest.json"),
            format!(r#"{{"id":"plugin{i}","name":"Plugin {i}","version":"1.0.0"}}"#),
        )?;
    }
    assert_eq!(
        ManifestService::new().discover_manifests(dir.path()).len(),
        2
    );
    Ok(())
}
