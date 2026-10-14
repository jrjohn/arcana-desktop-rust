//! New (no C# counterpart): settings files, locale detection, string formatting, embedded
//! resources, logging setup, audit writer and the wired service graph end to end.

use std::sync::{Arc, Mutex};

use arcana_core::{ErrorCode, PageRequest, Repository};
use arcana_data::{Db, SqliteRepository};
use arcana_domain::{AuditEventType, Customer, User};
use arcana_infra::locale::{best_supported, normalize, system_locale};
use arcana_infra::logging::{RETAINED_FILES, default_log_dir, init_logging, rolling_appender};
use arcana_infra::security::{AuditEntry, AuditLogger, TokenServiceOptions};
use arcana_infra::settings::{app_data_dir, default_settings_path};
use arcana_infra::wiring::{ArcanaConfig, ArcanaServices};
use arcana_infra::{
    AppSettings, AppSettingsService, JsonSettingsService, LocalizationService,
    LocalizationServiceImpl, SettingChanged, SettingsService, SettingsServiceExt, format_template,
    setting_keys,
};
use serde_json::json;

#[test]
fn json_settings_round_trip_events_and_persistence() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/settings.json");
    let s = JsonSettingsService::new(&path);
    assert_eq!(s.path(), path);
    assert!(!s.contains(setting_keys::THEME));
    assert_eq!(s.get(setting_keys::WINDOW_WIDTH, 800), 800);
    let seen = Arc::new(Mutex::new(Vec::<SettingChanged>::new()));
    let sink = seen.clone();
    s.on_setting_changed(Box::new(move |e| sink.lock().unwrap().push(e.clone())));

    s.set(setting_keys::THEME, &"Dark").unwrap();
    s.set(setting_keys::WINDOW_WIDTH, &1024).unwrap();
    s.set(setting_keys::THEME, &"Dark").unwrap(); // unchanged → no event
    assert_eq!(s.get(setting_keys::THEME, String::new()), "Dark");
    assert_eq!(
        s.get(setting_keys::THEME, 0),
        0,
        "wrong type falls back to default"
    );
    assert_eq!(s.get_all().len(), 2);

    let reloaded = JsonSettingsService::new(&path);
    assert_eq!(
        reloaded.get_value(setting_keys::WINDOW_WIDTH),
        Some(json!(1024))
    );

    s.remove(setting_keys::THEME).unwrap();
    s.remove("missing").unwrap();
    assert!(!s.contains(setting_keys::THEME));
    let events = seen.lock().unwrap();
    assert_eq!(events.len(), 3);
    assert_eq!(events[2].old_value, Some(json!("Dark")));
    assert_eq!(events[2].new_value, None);
    assert!(format!("{s:?}").contains("settings.json"));
}

/// A value serde refuses to serialise.
struct NotJson;

impl serde::Serialize for NotJson {
    fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
        Err(serde::ser::Error::custom("nope"))
    }
}

#[test]
fn json_settings_ignores_corrupt_file_and_reports_write_errors() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("s.json");
    std::fs::write(&path, "not json").unwrap();
    let s = JsonSettingsService::new(&path);
    assert!(s.get_all().is_empty());
    // A directory where the file should be cannot be written.
    let bad = JsonSettingsService::new(dir.path());
    assert_eq!(
        bad.set_value("k", json!(1)).unwrap_err().code(),
        ErrorCode::FileAccessDenied
    );
    assert_eq!(
        s.set("k", &NotJson).unwrap_err().code(),
        ErrorCode::InvalidFormat
    );
}

#[test]
fn app_settings_lazy_load_save_on_change_and_csharp_shape() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(
        &path,
        r#"{"ThemeId":"Dark","LanguageCode":"ja-JP","AutoSyncEnabled":false,"SyncFrequencyMinutes":15}"#,
    )
    .unwrap();
    let s = AppSettingsService::new(&path);
    assert_eq!(s.path(), path);
    assert_eq!(s.theme_id(), "Dark");
    assert_eq!(s.language_code(), "ja-JP");
    assert!(!s.auto_sync_enabled());
    assert_eq!(s.sync_frequency_minutes(), 15);

    let changes = Arc::new(Mutex::new(0));
    let c = changes.clone();
    s.on_settings_changed(Box::new(move |_| *c.lock().unwrap() += 1));
    assert!(!s.set_theme_id("Dark"), "unchanged value is not saved");
    assert!(s.set_theme_id("Light"));
    assert!(s.set_language_code("zh-TW"));
    assert!(s.set_auto_sync_enabled(true));
    assert!(s.set_sync_frequency_minutes(30));
    assert_eq!(*changes.lock().unwrap(), 4);

    let on_disk: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(on_disk["ThemeId"], "Light");
    assert_eq!(on_disk["SyncFrequencyMinutes"], 30);
    assert_eq!(AppSettingsService::new(&path).settings(), s.settings());
    assert!(format!("{s:?}").contains("settings.json"));
}

#[test]
fn app_settings_defaults_and_failed_save() {
    let d = AppSettings::default();
    assert_eq!(
        (
            d.theme_id.as_str(),
            d.auto_sync_enabled,
            d.sync_frequency_minutes
        ),
        ("System", true, 5)
    );
    assert!(["en-US", "zh-TW", "ja-JP"].contains(&d.language_code.as_str()));
    let dir = tempfile::tempdir().unwrap();
    // Path is a directory: load falls back to defaults, save fails and reports `false`.
    let s = AppSettingsService::new(dir.path());
    assert_eq!(s.theme_id(), "System");
    assert!(!s.set_theme_id("Dark"));
    // Partial file: missing fields take defaults.
    let p = dir.path().join("partial.json");
    std::fs::write(&p, r#"{"ThemeId":"Dark"}"#).unwrap();
    assert_eq!(AppSettingsService::new(&p).sync_frequency_minutes(), 5);
}

#[test]
fn locale_helpers() {
    assert_eq!(normalize("zh_TW.UTF-8").as_deref(), Some("zh-TW"));
    assert_eq!(normalize("ja_JP@euro").as_deref(), Some("ja-JP"));
    assert_eq!(normalize("C"), None);
    assert_eq!(normalize(""), None);
    assert_eq!(best_supported(Some("zh-tw")), "zh-TW");
    assert_eq!(best_supported(Some("ja")), "ja-JP");
    assert_eq!(best_supported(Some("zh-CN")), "zh-TW");
    assert_eq!(best_supported(Some("fr-FR")), "en-US");
    assert_eq!(best_supported(None), "en-US");
    let _ = system_locale();
    assert!(app_data_dir().ends_with("Arcana"));
    assert!(default_settings_path().ends_with("Arcana/settings.json"));
    assert!(default_log_dir().ends_with("Arcana/logs"));
}

#[test]
fn format_template_follows_dotnet_rules() {
    assert_eq!(
        format_template("{0} + {1}", &[&1, &"two"]).as_deref(),
        Some("1 + two")
    );
    assert_eq!(
        format_template("{{0}} {0}", &[&"x"]).as_deref(),
        Some("{0} x")
    );
    assert_eq!(
        format_template("{0,5} {0:N2}", &[&7]).as_deref(),
        Some("7 7")
    );
    assert_eq!(format_template("{1}", &[&0]), None);
    assert_eq!(format_template("{x}", &[&0]), None);
    assert_eq!(format_template("open {0", &[&0]), None);
    assert_eq!(format_template("close }", &[]), None);
    assert_eq!(format_template("plain", &[]).as_deref(), Some("plain"));
}

#[test]
fn embedded_resources_match_csharp_strings() {
    let s = LocalizationServiceImpl::default();
    assert_eq!(s.core_count("en-US"), 190);
    assert_eq!(s.core_count("zh-TW"), 190);
    assert_eq!(s.core_count("ja-JP"), 182);
    s.set_culture("en-US");
    assert_eq!(s.get("nav.home"), "Home");
    assert_eq!(s.get_with("common.pageFormat", &[&1, &3]), "Page 1 / 3");
    s.set_culture("zh-TW");
    assert_ne!(s.get("nav.home"), "Home");
    s.set_culture("ja-JP");
    assert_ne!(s.get("nav.home"), "Home");
    assert!(format!("{s:?}").contains("ja-JP"));
    // Every ja-JP / zh-TW key exists in en-US (the fallback).
    let en: std::collections::HashMap<String, String> =
        serde_json::from_str(include_str!("../resources/strings/en-US.json")).unwrap();
    for json in [
        include_str!("../resources/strings/zh-TW.json"),
        include_str!("../resources/strings/ja-JP.json"),
    ] {
        let other: std::collections::HashMap<String, String> = serde_json::from_str(json).unwrap();
        assert!(other.keys().all(|k| en.contains_key(k)));
    }
}

#[test]
fn rolling_appender_and_global_logging() {
    let dir = tempfile::tempdir().unwrap();
    assert!(rolling_appender(dir.path()).is_ok());
    assert_eq!(RETAINED_FILES, 7);
    let guard = init_logging(dir.path()).unwrap();
    tracing::info!("hello from test");
    drop(guard);
    assert!(
        std::fs::read_dir(dir.path())
            .unwrap()
            .any(|e| { e.unwrap().file_name().to_string_lossy().starts_with("app") })
    );
    // A second global subscriber is refused.
    assert_eq!(
        init_logging(dir.path()).unwrap_err().code(),
        ErrorCode::Unknown
    );
    let file = tempfile::NamedTempFile::new().unwrap();
    assert!(rolling_appender(&file.path().join("x")).is_err());
}

#[tokio::test]
async fn audit_logger_defaults_action_to_event_name() {
    let db = Db::in_memory().await.unwrap();
    let a = AuditLogger::new(db);
    let row = a
        .write(AuditEntry {
            event_type: AuditEventType::ConfigurationChanged,
            is_success: true,
            ..AuditEntry::default()
        })
        .await
        .unwrap();
    assert_eq!(row.action, "ConfigurationChanged");
    a.write(AuditEntry {
        action: Some("custom".into()),
        ..AuditEntry::default()
    })
    .await
    .unwrap();
    let all = a.entries().await.unwrap();
    assert_eq!(all.len(), 2);
    assert_eq!(all[1].action, "custom");
    assert_eq!(format!("{a:?}"), "AuditLogger");
}

fn config(dir: &std::path::Path) -> ArcanaConfig {
    ArcanaConfig {
        token_key: b"wiring-test-key-not-a-secret".to_vec(),
        token_options: TokenServiceOptions::default(),
        settings_path: dir.join("kv.json"),
        app_settings_path: dir.join("settings.json"),
    }
}

#[tokio::test]
async fn wired_services_work_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::in_memory().await.unwrap();
    let svc = ArcanaServices::new(db.clone(), config(dir.path())).unwrap();
    assert!(!format!("{:?}", config(dir.path())).contains("wiring-test"));
    assert_eq!(format!("{svc:?}"), "ArcanaServices");

    // A user whose hash uses the legacy iteration count is rehashed on login.
    let legacy = "1:10000:AAECAwQFBgcICQoLDA0ODw==:ytt5+rfAYyyO52xFMVtWIdr45UW1wpd59zviwcrOZ6M=";
    let users = SqliteRepository::<User>::new(db.clone());
    let u = users
        .add(User {
            username: "alice".into(),
            display_name: "Alice".into(),
            password_hash: legacy.into(),
            is_active: true,
            ..User::default()
        })
        .await
        .unwrap();
    let r = svc.auth.authenticate("alice", "legacy").await.unwrap();
    assert!(
        svc.token_service
            .validate_access_token(&r.access_token)
            .is_valid
    );
    assert_eq!(
        svc.auth
            .validate_token(&r.access_token)
            .await
            .unwrap()
            .username,
        "alice"
    );
    let stored = users.get_by_id(u.base.id).await.unwrap().unwrap();
    assert!(!svc.password_hasher.needs_rehash(&stored.password_hash));
    assert!(
        svc.password_hasher
            .verify_password("legacy", &stored.password_hash)
            .unwrap()
    );
    assert_eq!(svc.current_user.username().as_deref(), Some("alice"));

    // Refresh with the real refresh token, then reset/lock with an actor in the audit trail.
    let r2 = svc.auth.refresh_token(&r.refresh_token).await.unwrap();
    assert_ne!(r2.refresh_token, r.refresh_token);
    let temp = svc.auth.reset_password(u.base.id).await.unwrap();
    svc.auth.lock_account(u.base.id, None).await.unwrap();
    // C# quirk kept: a lock without an end date is treated as expired at the next login.
    let r3 = svc.auth.authenticate("alice", &temp).await.unwrap();
    assert!(r3.must_change_password);
    assert!(
        svc.auth
            .change_password(u.base.id, &temp, "brand-new-pass")
            .await
            .unwrap()
    );
    assert!(svc.auth.logout(u.base.id).await.unwrap());
    let audit = svc.audit.entries().await.unwrap();
    assert!(
        audit
            .iter()
            .any(|a| a.details.as_deref() == Some("Reset by alice"))
    );
    assert!(
        !svc.authorization
            .has_permission(u.base.id, "orders.view")
            .await
            .unwrap()
    );
}

#[tokio::test]
async fn wired_domain_services_settings_and_localization() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::in_memory().await.unwrap();
    let svc = ArcanaServices::new(db.clone(), config(dir.path())).unwrap();
    let c = svc
        .customers
        .create_customer(Customer {
            code: "C1".into(),
            name: "Acme".into(),
            ..Customer::default()
        })
        .await
        .unwrap();
    assert_eq!(
        svc.customers
            .get_customer_by_id(c.base.id)
            .await
            .unwrap()
            .name,
        "Acme"
    );
    assert_eq!(
        svc.products
            .get_products(&PageRequest::default())
            .await
            .unwrap()
            .total_count,
        0
    );
    assert!(
        svc.orders
            .generate_order_number()
            .await
            .unwrap()
            .starts_with("ORD")
    );
    svc.settings.set_value("k", json!(true)).unwrap();
    assert!(svc.app_settings.set_theme_id("Dark"));
    assert_eq!(svc.localization.available_cultures().len(), 3);

    let mut bad = config(dir.path());
    bad.token_key.clear();
    assert!(ArcanaServices::new(db, bad).is_err());
}

#[tokio::test]
async fn blank_stored_hash_surfaces_as_validation_error() {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::in_memory().await.unwrap();
    let svc = ArcanaServices::new(db.clone(), config(dir.path())).unwrap();
    SqliteRepository::<User>::new(db)
        .add(User {
            username: "bob".into(),
            display_name: "Bob".into(),
            password_hash: " ".into(),
            is_active: true,
            ..User::default()
        })
        .await
        .unwrap();
    assert_eq!(
        svc.auth.authenticate("bob", "pw").await.unwrap_err().code(),
        ErrorCode::ValidationFailed
    );
}
