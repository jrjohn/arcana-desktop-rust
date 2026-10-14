//! C# `LocalizationServiceTests` (24), using a temp resources directory like C#.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arcana_infra::{LocalizationService, LocalizationServiceImpl};
use tempfile::TempDir;

struct Ctx {
    dir: TempDir,
}

fn ctx() -> Ctx {
    Ctx {
        dir: tempfile::tempdir().unwrap(),
    }
}

impl Ctx {
    fn write(&self, culture: &str, pairs: &[(&str, &str)]) {
        let map: HashMap<_, _> = pairs.iter().copied().collect();
        std::fs::write(
            self.dir.path().join(format!("{culture}.json")),
            serde_json::to_string(&map).unwrap(),
        )
        .unwrap();
    }

    fn service(&self) -> LocalizationServiceImpl {
        LocalizationServiceImpl::with_resources_dir(self.dir.path())
    }

    fn en(&self) -> LocalizationServiceImpl {
        let s = self.service();
        s.set_culture("en-US");
        s
    }
}

fn map(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs
        .iter()
        .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
        .collect()
}

// Constructor_NoResourceFiles_ShouldNotThrow
#[test]
fn constructor_no_resource_files_should_not_throw() {
    let s = ctx().service();
    assert_eq!(s.core_count("en-US"), 0);
}

// CurrentCulture_Default_ShouldBeEnUs
#[test]
fn current_culture_default_should_be_en_us() {
    let s = ctx().service();
    assert!(["en-US", "zh-TW", "ja-JP"].contains(&s.current_culture().as_str()));
}

// AvailableCultures_ShouldContainEnUs
#[test]
fn available_cultures_should_contain_en_us() {
    assert!(
        ctx()
            .service()
            .available_cultures()
            .contains(&"en-US".to_owned())
    );
}

// AvailableCultures_ShouldHaveThreeCultures
#[test]
fn available_cultures_should_have_three_cultures() {
    assert_eq!(
        ctx().service().available_cultures(),
        ["en-US", "zh-TW", "ja-JP"]
    );
}

// Get_ExistingKey_ShouldReturnTranslation
#[test]
fn get_existing_key_should_return_translation() {
    let c = ctx();
    c.write("en-US", &[("greeting", "Hello")]);
    assert_eq!(c.en().get("greeting"), "Hello");
}

// Get_NonExistingKey_ShouldReturnKey
#[test]
fn get_non_existing_key_should_return_key() {
    assert_eq!(ctx().en().get("nonexistent.key"), "nonexistent.key");
}

// Get_NullOrEmptyKey_ShouldReturnEmpty (no null `&str` in Rust)
#[test]
fn get_null_or_empty_key_should_return_empty() {
    assert_eq!(ctx().service().get(""), "");
}

// Get_WithArgs_ShouldFormatString
#[test]
fn get_with_args_should_format_string() {
    let c = ctx();
    c.write("en-US", &[("welcome", "Hello, {0}!")]);
    assert_eq!(c.en().get_with("welcome", &[&"Alice"]), "Hello, Alice!");
}

// Get_WithArgs_InvalidFormat_ShouldReturnTemplate
#[test]
fn get_with_args_invalid_format_should_return_template() {
    let c = ctx();
    c.write("en-US", &[("broken", "Hello {0} {1} {999}")]);
    assert_eq!(
        c.en().get_with("broken", &[&"only-one-arg"]),
        "Hello {0} {1} {999}"
    );
}

// SetCulture_ValidCulture_ShouldChangeCulture
#[test]
fn set_culture_valid_culture_should_change_culture() {
    let s = ctx().service();
    s.set_culture("zh-TW");
    assert_eq!(s.current_culture(), "zh-TW");
}

// SetCulture_InvalidCulture_ShouldNotChange
#[test]
fn set_culture_invalid_culture_should_not_change() {
    let s = ctx().en();
    s.set_culture("xx-INVALID");
    assert_eq!(s.current_culture(), "en-US");
}

fn counter(s: &LocalizationServiceImpl) -> Arc<AtomicUsize> {
    let n = Arc::new(AtomicUsize::new(0));
    let m = n.clone();
    s.on_culture_changed(Box::new(move |_| {
        m.fetch_add(1, Ordering::SeqCst);
    }));
    n
}

// SetCulture_SameCulture_ShouldNotRaiseEvent
#[test]
fn set_culture_same_culture_should_not_raise_event() {
    let s = ctx().en();
    let n = counter(&s);
    s.set_culture("en-US");
    assert_eq!(n.load(Ordering::SeqCst), 0);
}

// SetCulture_DifferentCulture_ShouldRaiseEvent
#[test]
fn set_culture_different_culture_should_raise_event() {
    let s = ctx().en();
    let n = counter(&s);
    s.set_culture("zh-TW");
    assert_eq!(n.load(Ordering::SeqCst), 1);
}

// Get_CultureFallbackToEnUs_WhenKeyMissingInCurrentCulture
#[test]
fn get_culture_fallback_to_en_us_when_key_missing_in_current_culture() {
    let c = ctx();
    c.write("en-US", &[("fallback.key", "Fallback Value")]);
    let s = c.service();
    s.set_culture("zh-TW");
    assert_eq!(s.get("fallback.key"), "Fallback Value");
}

// RegisterPluginResources_ShouldMakeKeysAvailable
#[test]
fn register_plugin_resources_should_make_keys_available() {
    let s = ctx().en();
    s.register_plugin_resources(
        "myplugin",
        "en-US",
        map(&[("plugin.hello", "Plugin Hello")]),
    );
    assert_eq!(s.get_for_plugin("myplugin", "plugin.hello"), "Plugin Hello");
}

// GetForPlugin_NonExistingKey_ShouldFallbackToCore
#[test]
fn get_for_plugin_non_existing_key_should_fallback_to_core() {
    let c = ctx();
    c.write("en-US", &[("core.key", "Core Value")]);
    assert_eq!(c.en().get_for_plugin("myplugin", "core.key"), "Core Value");
}

// GetForPlugin_EmptyKey_ShouldReturnEmpty
#[test]
fn get_for_plugin_empty_key_should_return_empty() {
    assert_eq!(ctx().service().get_for_plugin("myplugin", ""), "");
}

// GetForPlugin_WithArgs_ShouldFormatString
#[test]
fn get_for_plugin_with_args_should_format_string() {
    let s = ctx().en();
    s.register_plugin_resources("myplugin", "en-US", map(&[("plugin.msg", "Welcome {0}!")]));
    assert_eq!(
        s.get_for_plugin_with("myplugin", "plugin.msg", &[&"World"]),
        "Welcome World!"
    );
}

// GetFromAnyPlugin_KeyInCoreResources_ShouldReturnCore
#[test]
fn get_from_any_plugin_key_in_core_resources_should_return_core() {
    let c = ctx();
    c.write("en-US", &[("core.key", "Core")]);
    assert_eq!(c.en().get_from_any_plugin("core.key"), "Core");
}

// GetFromAnyPlugin_KeyInPlugin_ShouldReturnPluginValue
#[test]
fn get_from_any_plugin_key_in_plugin_should_return_plugin_value() {
    let s = ctx().en();
    s.register_plugin_resources(
        "plugin1",
        "en-US",
        map(&[("exclusive.key", "Plugin Exclusive")]),
    );
    assert_eq!(s.get_from_any_plugin("exclusive.key"), "Plugin Exclusive");
}

// GetFromAnyPlugin_NotFound_ShouldReturnKey
#[test]
fn get_from_any_plugin_not_found_should_return_key() {
    assert_eq!(
        ctx().en().get_from_any_plugin("totally.missing"),
        "totally.missing"
    );
}

// GetFromAnyPlugin_EmptyKey_ShouldReturnEmpty
#[test]
fn get_from_any_plugin_empty_key_should_return_empty() {
    assert_eq!(ctx().service().get_from_any_plugin(""), "");
}

// RegisterPluginResources_PluginFallbackToEnUs_WhenKeyMissingInCurrentCulture
#[test]
fn register_plugin_resources_plugin_fallback_to_en_us_when_key_missing_in_current_culture() {
    let s = ctx().service();
    s.register_plugin_resources("plugin1", "en-US", map(&[("fallback.msg", "Fallback EN")]));
    s.set_culture("zh-TW");
    assert_eq!(s.get_for_plugin("plugin1", "fallback.msg"), "Fallback EN");
}

// Constructor_CorruptJsonFile_ShouldNotThrow
#[test]
fn constructor_corrupt_json_file_should_not_throw() {
    let c = ctx();
    std::fs::write(
        c.dir.path().join("en-US.json"),
        "{ this is not valid json }",
    )
    .unwrap();
    let s = c.en();
    assert_eq!(s.core_count("en-US"), 0);
    assert_eq!(s.get("x"), "x");
}
