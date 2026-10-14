//! Settings (C# `Arcana.Infrastructure.Settings.SettingsService` and
//! `Arcana.App.Services.AppSettingsService`).
//!
//! C# declares the key/value `SettingsService` interface without an implementation (its DI
//! registration maps the interface to itself). [`JsonSettingsService`] implements it over a
//! JSON file. [`AppSettingsService`] ports the typed app settings that the C# App project
//! actually uses. Both take the file path, so tests can point them at a temp directory;
//! [`default_settings_path`] is the production location.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use arcana_core::{AppError, AppResult, ErrorCode};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::locale;

/// Well-known keys (C# `SettingKeys`).
pub mod setting_keys {
    pub const THEME: &str = "app.theme";
    pub const LANGUAGE: &str = "app.language";
    pub const SYNC_ENABLED: &str = "sync.enabled";
    pub const SYNC_INTERVAL: &str = "sync.interval";
    pub const LAST_SYNC_TIME: &str = "sync.lastTime";
    pub const WINDOW_WIDTH: &str = "window.width";
    pub const WINDOW_HEIGHT: &str = "window.height";
    pub const WINDOW_X: &str = "window.x";
    pub const WINDOW_Y: &str = "window.y";
    pub const IS_MAXIMIZED: &str = "window.maximized";
    pub const SIDEBAR_WIDTH: &str = "sidebar.width";
    pub const RECENT_FILES: &str = "recent.files";
}

/// `<local app data>/Arcana/<file>` (C# `Environment.SpecialFolder.LocalApplicationData`).
#[must_use]
pub fn app_data_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("Arcana")
}

/// The C# settings file location: `<local app data>/Arcana/settings.json`.
#[must_use]
pub fn default_settings_path() -> PathBuf {
    app_data_dir().join("settings.json")
}

fn file_error(e: &dyn std::fmt::Display) -> AppError {
    AppError::data(ErrorCode::FileAccessDenied, e.to_string())
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> AppResult<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| file_error(&e))?;
    }
    let json = serde_json::to_string_pretty(value).map_err(|e| file_error(&e))?;
    std::fs::write(path, json).map_err(|e| file_error(&e))
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    let text = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str(&text) {
        Ok(v) => Some(v),
        Err(e) => {
            tracing::warn!("ignoring unreadable settings file {}: {e}", path.display());
            None
        }
    }
}

fn guard<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Payload of a setting change (C# `SettingChangedEventArgs`).
#[derive(Debug, Clone, PartialEq)]
pub struct SettingChanged {
    pub key: String,
    pub old_value: Option<Value>,
    pub new_value: Option<Value>,
}

pub type SettingChangedHandler = Box<dyn Fn(&SettingChanged) + Send + Sync>;

/// C# `SettingsService`. Values are JSON; [`SettingsServiceExt`] adds the typed `Get<T>` /
/// `SetAsync<T>`. Writes are synchronous (a small local file).
pub trait SettingsService: Send + Sync {
    fn get_value(&self, key: &str) -> Option<Value>;
    /// # Errors
    /// `FileAccessDenied` when the file cannot be written.
    fn set_value(&self, key: &str, value: Value) -> AppResult<()>;
    /// # Errors
    /// `FileAccessDenied` when the file cannot be written.
    fn remove(&self, key: &str) -> AppResult<()>;
    fn contains(&self, key: &str) -> bool;
    fn get_all(&self) -> BTreeMap<String, Value>;
    fn on_setting_changed(&self, handler: SettingChangedHandler);
}

/// Typed access on any [`SettingsService`].
pub trait SettingsServiceExt: SettingsService {
    /// The value as `T`, or `default` when missing or of another type.
    fn get<T: DeserializeOwned>(&self, key: &str, default: T) -> T {
        self.get_value(key)
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or(default)
    }

    /// Store `value`.
    ///
    /// # Errors
    /// `InvalidFormat` when `value` cannot be represented as JSON; otherwise as
    /// [`SettingsService::set_value`].
    fn set<T: Serialize>(&self, key: &str, value: &T) -> AppResult<()> {
        let v = serde_json::to_value(value).map_err(|e| AppError::Validation {
            code: ErrorCode::InvalidFormat,
            message: e.to_string(),
            errors: vec![],
        })?;
        self.set_value(key, v)
    }
}

impl<S: SettingsService + ?Sized> SettingsServiceExt for S {}

/// [`SettingsService`] persisted as one JSON object.
pub struct JsonSettingsService {
    path: PathBuf,
    values: Mutex<BTreeMap<String, Value>>,
    handlers: Mutex<Vec<SettingChangedHandler>>,
}

impl std::fmt::Debug for JsonSettingsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JsonSettingsService")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl JsonSettingsService {
    /// Load `path` (missing or unreadable → empty).
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let values = read_json(&path).unwrap_or_default();
        Self {
            path,
            values: Mutex::new(values),
            handlers: Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn change(&self, key: &str, new_value: Option<Value>) -> AppResult<()> {
        let old_value = {
            let mut values = guard(&self.values);
            let old = match &new_value {
                Some(v) => values.insert(key.to_owned(), v.clone()),
                None => values.remove(key),
            };
            write_json(&self.path, &*values)?;
            old
        };
        if old_value != new_value {
            let e = SettingChanged {
                key: key.to_owned(),
                old_value,
                new_value,
            };
            for h in guard(&self.handlers).iter() {
                h(&e);
            }
        }
        Ok(())
    }
}

impl SettingsService for JsonSettingsService {
    fn get_value(&self, key: &str) -> Option<Value> {
        guard(&self.values).get(key).cloned()
    }

    fn set_value(&self, key: &str, value: Value) -> AppResult<()> {
        self.change(key, Some(value))
    }

    fn remove(&self, key: &str) -> AppResult<()> {
        if self.contains(key) {
            self.change(key, None)
        } else {
            Ok(())
        }
    }

    fn contains(&self, key: &str) -> bool {
        guard(&self.values).contains_key(key)
    }

    fn get_all(&self) -> BTreeMap<String, Value> {
        guard(&self.values).clone()
    }

    fn on_setting_changed(&self, handler: SettingChangedHandler) {
        guard(&self.handlers).push(handler);
    }
}

/// Typed application settings (C# `AppSettings`), stored with the C# property names.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase", default)]
pub struct AppSettings {
    pub theme_id: String,
    pub language_code: String,
    pub auto_sync_enabled: bool,
    pub sync_frequency_minutes: i32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            theme_id: "System".into(),
            language_code: locale::best_supported(locale::system_locale().as_deref()).into(),
            auto_sync_enabled: true,
            sync_frequency_minutes: 5,
        }
    }
}

pub type AppSettingsHandler = Box<dyn Fn(&AppSettings) + Send + Sync>;

/// C# `AppSettingsService`: loads lazily on first access, saves on every real change and
/// raises `SettingsChanged` after a successful save. Like C#, save errors are logged, not
/// returned (the setters are property setters there).
pub struct AppSettingsService {
    path: PathBuf,
    state: Mutex<Option<AppSettings>>,
    handlers: Mutex<Vec<AppSettingsHandler>>,
}

impl std::fmt::Debug for AppSettingsService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppSettingsService")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

impl AppSettingsService {
    #[must_use]
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            state: Mutex::new(None),
            handlers: Mutex::new(Vec::new()),
        }
    }

    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    fn with<R>(&self, f: impl FnOnce(&mut AppSettings) -> R) -> R {
        let mut state = guard(&self.state);
        let settings = state.get_or_insert_with(|| read_json(&self.path).unwrap_or_default());
        f(settings)
    }

    /// A snapshot of every setting (C# `Settings`).
    #[must_use]
    pub fn settings(&self) -> AppSettings {
        self.with(|s| s.clone())
    }

    pub fn on_settings_changed(&self, handler: AppSettingsHandler) {
        guard(&self.handlers).push(handler);
    }

    /// Apply `f`; save and notify when it changed something. Returns whether it saved.
    fn update(&self, f: impl FnOnce(&mut AppSettings)) -> bool {
        let saved = self.with(|s| {
            let before = s.clone();
            f(s);
            if *s == before {
                return None;
            }
            match write_json(&self.path, s) {
                Ok(()) => Some(s.clone()),
                Err(e) => {
                    tracing::error!("Error saving settings: {e}");
                    None
                }
            }
        });
        match saved {
            Some(s) => {
                for h in guard(&self.handlers).iter() {
                    h(&s);
                }
                true
            }
            None => false,
        }
    }

    #[must_use]
    pub fn theme_id(&self) -> String {
        self.with(|s| s.theme_id.clone())
    }

    pub fn set_theme_id(&self, value: &str) -> bool {
        self.update(|s| value.clone_into(&mut s.theme_id))
    }

    #[must_use]
    pub fn language_code(&self) -> String {
        self.with(|s| s.language_code.clone())
    }

    pub fn set_language_code(&self, value: &str) -> bool {
        self.update(|s| value.clone_into(&mut s.language_code))
    }

    #[must_use]
    pub fn auto_sync_enabled(&self) -> bool {
        self.with(|s| s.auto_sync_enabled)
    }

    pub fn set_auto_sync_enabled(&self, value: bool) -> bool {
        self.update(|s| s.auto_sync_enabled = value)
    }

    #[must_use]
    pub fn sync_frequency_minutes(&self) -> i32 {
        self.with(|s| s.sync_frequency_minutes)
    }

    pub fn set_sync_frequency_minutes(&self, value: i32) -> bool {
        self.update(|s| s.sync_frequency_minutes = value)
    }
}
