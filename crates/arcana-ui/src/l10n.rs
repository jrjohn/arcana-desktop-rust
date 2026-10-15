//! UI strings on top of the M3 localization service.
//!
//! Core strings (C# `Resources/Strings/*.json`) are untouched. Plugin locale files (menus,
//! module titles) are registered by the plugins themselves when they activate (M5); the
//! strings this port adds (product/customer forms, plugin manager, notices) are registered
//! here as plugin resources. Every lookup goes through `get_from_any_plugin` (core first, then
//! plugins) — the same path C# uses for menus.

use std::collections::HashMap;

use arcana_infra::LocalizationService;
use arcana_infra::locale::SUPPORTED_CULTURES;

use crate::effect::Text;

/// Plugin id for strings added by this port.
pub const UI_PLUGIN: &str = "arcana.ui";

const UI: [(&str, &str); 3] = [
    ("en-US", include_str!("../resources/strings/en-US.json")),
    ("zh-TW", include_str!("../resources/strings/zh-TW.json")),
    ("ja-JP", include_str!("../resources/strings/ja-JP.json")),
];

fn parse(json: &str) -> HashMap<String, String> {
    serde_json::from_str(json).unwrap_or_else(|e| {
        tracing::error!("bad embedded UI strings: {e}");
        HashMap::new()
    })
}

/// Register the menu and UI string tables for every supported culture.
pub fn register(l10n: &dyn LocalizationService) {
    // The menu strings are registered by the CoreMenu plugin itself (M5).
    for (plugin, table) in [(UI_PLUGIN, UI)] {
        for (culture, json) in table {
            debug_assert!(SUPPORTED_CULTURES.contains(&culture));
            l10n.register_plugin_resources(plugin, culture, parse(json));
        }
    }
}

/// `key` in the current culture.
#[must_use]
pub fn t(l10n: &dyn LocalizationService, key: &str) -> String {
    l10n.get_from_any_plugin(key)
}

/// `key` with `{0}`… replaced by `args`.
#[must_use]
pub fn t_with(l10n: &dyn LocalizationService, key: &str, args: &[String]) -> String {
    let template = t(l10n, key);
    let refs: Vec<&dyn std::fmt::Display> =
        args.iter().map(|a| a as &dyn std::fmt::Display).collect();
    arcana_infra::format_template(&template, &refs).unwrap_or(template)
}

/// Resolve a [`Text`].
#[must_use]
pub fn text(l10n: &dyn LocalizationService, text: &Text) -> String {
    match text {
        Text::Key(k) => t(l10n, k),
        Text::KeyWith(k, args) => t_with(l10n, k, args),
        Text::Raw(s) => s.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arcana_infra::LocalizationServiceImpl;

    #[test]
    fn every_ui_key_exists_in_every_culture() {
        let en = parse(UI[0].1);
        for (culture, json) in &UI {
            let table = parse(json);
            assert!(!table.is_empty(), "{culture}");
            for k in en.keys() {
                assert!(table.contains_key(k), "{culture} missing {k}");
            }
        }
    }

    #[test]
    fn lookups_resolve_core_menu_and_ui_strings_per_culture() {
        let l = LocalizationServiceImpl::new();
        register(&l);
        l.set_culture("en-US");
        assert_eq!(t(&l, "nav.home"), "Home");
        assert_eq!(t(&l, "ui.product.code"), "Code");
        assert_eq!(
            t_with(
                &l,
                "common.showingFormat",
                &["1".into(), "10".into(), "10".into()]
            ),
            "Showing 1-10 of 10"
        );
        assert_eq!(text(&l, &Text::Raw("x".into())), "x");
        assert_eq!(text(&l, &Text::Key("nav.home")), "Home");
        l.set_culture("zh-TW");
        assert_eq!(t(&l, "nav.home"), "首頁");
        // Menu strings now come from the CoreMenu plugin, not from here.
        assert_eq!(t(&l, "menu.file"), "menu.file");
        l.set_culture("ja-JP");
        assert_ne!(t(&l, "ui.product.code"), "ui.product.code");
        assert_eq!(t(&l, "missing.key"), "missing.key");
        assert_eq!(parse("not json").len(), 0);
    }
}
