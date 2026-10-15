//! C# `PluginBaseLocalizationTests` (13). Logger verifications become assertions on the
//! helpers' return values (number of files registered / success flag) — the log line is the
//! same event, observed through the API instead of a mock logger.

#[path = "support/host.rs"]
mod host;

use std::any::Any;
use std::collections::HashMap;

use arcana_plugin_api::{Plugin, PluginMetadata, PluginType};
use arcana_plugin_host::plugin_base::{BasePlugin, PluginBehavior};
use host::{Fixture, fixture, tmp, write};

struct TestPlugin;
impl PluginBehavior for TestPlugin {
    fn metadata(&self) -> PluginMetadata {
        PluginMetadata::new(
            "test.plugin",
            "Test Plugin",
            semver::Version::new(1, 0, 0),
            PluginType::Module,
        )
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn active(f: &Fixture, dir: &std::path::Path) -> BasePlugin<TestPlugin> {
    let mut p = BasePlugin::new(TestPlugin);
    assert!(p.activate(&f.context("test.plugin", dir)).is_ok());
    p
}

// LoadExternalLocalizationAsync_WithValidFiles_ShouldRegisterResources
#[test]
fn load_external_with_valid_files_should_register_resources() {
    let f = fixture();
    let d = tmp();
    let loc = d.path().join("locales");
    write(
        &loc.join("en-US.json"),
        r#"{"key1":"Value 1","key2":"Value 2"}"#,
    );
    write(&loc.join("zh-TW.json"), r#"{"key1":"值 1","key2":"值 2"}"#);
    let p = active(&f, d.path());
    assert_eq!(p.base().load_external_localization(Some(&loc)), 2);
    assert_eq!(p.base().l("key1"), "Value 1");
    let calls = f.l10n.calls();
    assert!(calls.contains(&("test.plugin".into(), "en-US".into(), 2)));
    assert!(calls.contains(&("test.plugin".into(), "zh-TW".into(), 2)));
}

// LoadExternalLocalizationAsync_DirectoryNotExists_ShouldLogWarning
#[test]
fn load_external_directory_not_exists_registers_nothing() {
    let f = fixture();
    let d = tmp();
    let p = active(&f, d.path());
    assert_eq!(
        p.base()
            .load_external_localization(Some(&d.path().join("non_existent_locales"))),
        0
    );
    assert!(f.l10n.calls().is_empty());
}

// LoadExternalLocalizationAsync_InvalidJson_ShouldLogError
#[test]
fn load_external_invalid_json_is_skipped() {
    let f = fixture();
    let d = tmp();
    let loc = d.path().join("locales");
    write(&loc.join("invalid.json"), "{ invalid json }");
    let p = active(&f, d.path());
    assert_eq!(p.base().load_external_localization(Some(&loc)), 0);
    assert!(f.l10n.calls().is_empty());
}

// LoadExternalLocalizationAsync_EmptyDirectory_ShouldNotRegisterAnything
#[test]
fn load_external_empty_directory_should_not_register_anything() {
    let f = fixture();
    let d = tmp();
    let loc = d.path().join("locales");
    std::fs::create_dir_all(&loc).unwrap_or_else(|e| panic!("{e}"));
    let p = active(&f, d.path());
    assert_eq!(p.base().load_external_localization(Some(&loc)), 0);
    assert!(f.l10n.calls().is_empty());
}

// LoadExternalLocalizationAsync_DefaultPath_ShouldUsePluginPathLocales
#[test]
fn load_external_default_path_should_use_plugin_path_locales() {
    let f = fixture();
    let d = tmp();
    write(
        &d.path().join("locales/en-US.json"),
        r#"{"key1":"Value 1"}"#,
    );
    let p = active(&f, d.path());
    assert_eq!(p.base().load_external_localization(None), 1);
    assert_eq!(
        f.l10n.calls(),
        vec![("test.plugin".into(), "en-US".into(), 1)]
    );
}

// LoadExternalLocalizationAsync_MultipleLanguages_ShouldLoadAll
#[test]
fn load_external_multiple_languages_should_load_all() {
    let f = fixture();
    let d = tmp();
    let loc = d.path().join("locales");
    let langs = ["en-US", "zh-TW", "ja-JP", "de-DE", "fr-FR"];
    for l in langs {
        write(
            &loc.join(format!("{l}.json")),
            &format!(r#"{{"greeting":"Hello in {l}"}}"#),
        );
    }
    let p = active(&f, d.path());
    assert_eq!(p.base().load_external_localization(Some(&loc)), 5);
    let calls = f.l10n.calls();
    for l in langs {
        assert_eq!(calls.iter().filter(|c| c.1 == l).count(), 1, "{l}");
    }
}

// LoadLocalizationFileAsync_ValidFile_ShouldRegisterResources
#[test]
fn load_file_valid_file_should_register_resources() {
    let f = fixture();
    let d = tmp();
    let file = d.path().join("custom.json");
    write(
        &file,
        r#"{"custom.key1":"Custom Value 1","custom.key2":"Custom Value 2"}"#,
    );
    let p = active(&f, d.path());
    assert!(p.base().load_localization_file("custom-culture", &file));
    assert_eq!(
        f.l10n.calls(),
        vec![("test.plugin".into(), "custom-culture".into(), 2)]
    );
}

// LoadLocalizationFileAsync_FileNotExists_ShouldLogWarning
#[test]
fn load_file_not_exists_returns_false() {
    let f = fixture();
    let d = tmp();
    let p = active(&f, d.path());
    assert!(
        !p.base()
            .load_localization_file("en-US", std::path::Path::new("/non/existent/file.json"))
    );
}

// LoadLocalizationFileAsync_InvalidJson_ShouldLogError
#[test]
fn load_file_invalid_json_returns_false() {
    let f = fixture();
    let d = tmp();
    let file = d.path().join("invalid.json");
    write(&file, "not valid json");
    let p = active(&f, d.path());
    assert!(!p.base().load_localization_file("en-US", &file));
    assert!(f.l10n.calls().is_empty());
}

// RegisterResources_ShouldCallLocalizationService
#[test]
fn register_resources_should_call_localization_service() {
    let f = fixture();
    let d = tmp();
    let p = active(&f, d.path());
    p.base().register_resources(
        "en-US",
        HashMap::from([("test.key".to_string(), "Test Value".to_string())]),
    );
    assert_eq!(
        f.l10n.calls(),
        vec![("test.plugin".into(), "en-US".into(), 1)]
    );
}

// L_ShouldReturnLocalizedString
#[test]
fn l_should_return_localized_string() {
    let f = fixture();
    let d = tmp();
    let p = active(&f, d.path());
    p.base().register_resources(
        "en-US",
        HashMap::from([("test.key".to_string(), "Localized Value".to_string())]),
    );
    assert_eq!(p.base().l("test.key"), "Localized Value");
}

// L_WithArgs_ShouldReturnFormattedString
#[test]
fn l_with_args_should_return_formatted_string() {
    let f = fixture();
    let d = tmp();
    let p = active(&f, d.path());
    p.base().register_resources(
        "en-US",
        HashMap::from([("test.key".to_string(), "Hello, {0}!".to_string())]),
    );
    assert_eq!(p.base().l_with("test.key", &[&"World"]), "Hello, World!");
}

// L_BeforeActivation_ShouldReturnKey
#[test]
fn l_before_activation_should_return_key() {
    let p = BasePlugin::new(TestPlugin);
    assert_eq!(p.base().l("test.key"), "test.key");
}
