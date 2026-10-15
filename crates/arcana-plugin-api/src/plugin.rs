//! `IPlugin`, `PluginMetadata`, `PluginType`, `PluginState` and the specialised plugin
//! interfaces (`IAuthenticationPlugin`, `IMfaPlugin`).

use std::collections::BTreeMap;

use arcana_core::AppResult;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::context::PluginContext;
use crate::health::{PluginHealthCheck, PluginHotReload, PluginUpgradeable};
use crate::services::ViewDefinition;

/// The 18 plugin categories (C# `PluginType`, same order).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PluginType {
    Menu,
    FunctionTree,
    View,
    Widget,
    Theme,
    #[default]
    Module,
    Service,
    DataSource,
    Export,
    Import,
    Print,
    Auth,
    Sync,
    Analytics,
    Notification,
    EntityExtension,
    ViewExtension,
    Workflow,
}

impl PluginType {
    pub const ALL: [Self; 18] = [
        Self::Menu,
        Self::FunctionTree,
        Self::View,
        Self::Widget,
        Self::Theme,
        Self::Module,
        Self::Service,
        Self::DataSource,
        Self::Export,
        Self::Import,
        Self::Print,
        Self::Auth,
        Self::Sync,
        Self::Analytics,
        Self::Notification,
        Self::EntityExtension,
        Self::ViewExtension,
        Self::Workflow,
    ];

    /// `Enum.TryParse(..., ignoreCase: true)`.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|t| t.name().eq_ignore_ascii_case(s.trim()))
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Menu => "Menu",
            Self::FunctionTree => "FunctionTree",
            Self::View => "View",
            Self::Widget => "Widget",
            Self::Theme => "Theme",
            Self::Module => "Module",
            Self::Service => "Service",
            Self::DataSource => "DataSource",
            Self::Export => "Export",
            Self::Import => "Import",
            Self::Print => "Print",
            Self::Auth => "Auth",
            Self::Sync => "Sync",
            Self::Analytics => "Analytics",
            Self::Notification => "Notification",
            Self::EntityExtension => "EntityExtension",
            Self::ViewExtension => "ViewExtension",
            Self::Workflow => "Workflow",
        }
    }
}

/// C# `PluginState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum PluginState {
    #[default]
    Unknown,
    NotLoaded,
    Loaded,
    Activating,
    Active,
    Deactivating,
    Deactivated,
    Uninstalled,
    Error,
}

/// C# `PluginMetadata`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginMetadata {
    pub id: String,
    pub name: String,
    pub version: semver::Version,
    pub description: Option<String>,
    pub author: Option<String>,
    pub plugin_type: PluginType,
    pub icon_path: Option<String>,
    /// `"id"` or `"id@range"` (C# `DependencyResolver.ParseDependency`).
    pub dependencies: Vec<String>,
    pub activation_events: Vec<String>,
}

impl PluginMetadata {
    #[must_use]
    pub fn new(id: &str, name: &str, version: semver::Version, plugin_type: PluginType) -> Self {
        Self {
            id: id.to_string(),
            name: name.to_string(),
            version,
            description: None,
            author: None,
            plugin_type,
            icon_path: None,
            dependencies: Vec::new(),
            activation_events: Vec::new(),
        }
    }
}

/// C# `IPlugin`. `ConfigureServices` has no Rust equivalent (no DI container); plugins
/// register everything through the [`PluginContext`] in [`Plugin::activate`].
///
/// The `as_*` methods replace C# `plugin is IPluginHealthCheck` style type tests.
pub trait Plugin: Send {
    fn metadata(&self) -> &PluginMetadata;
    fn state(&self) -> PluginState;
    /// # Errors
    /// Whatever the plugin reports; the host moves it to [`PluginState::Error`].
    fn activate(&mut self, context: &PluginContext) -> AppResult<()>;
    /// # Errors
    /// Whatever the plugin reports.
    fn deactivate(&mut self) -> AppResult<()>;
    /// Locale resources (`culture -> key -> text`) registered before activation.
    fn resources(&self) -> BTreeMap<String, BTreeMap<String, String>> {
        BTreeMap::new()
    }
    fn as_health_check(&self) -> Option<&dyn PluginHealthCheck> {
        None
    }
    /// Declarative UI tree (form / table / list …) as JSON, for plugins that cannot draw
    /// themselves (WASM). The host renders it.
    fn ui_tree(&mut self) -> Option<serde_json::Value> {
        None
    }
    fn as_hot_reload(&mut self) -> Option<&mut dyn PluginHotReload> {
        None
    }
    fn as_upgradeable(&mut self) -> Option<&mut dyn PluginUpgradeable> {
        None
    }
}

/// C# `ExternalAuthContext`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalAuthContext {
    pub parameters: BTreeMap<String, String>,
    pub redirect_uri: Option<String>,
    pub state: Option<String>,
    pub scope: Option<String>,
}

/// C# `ExternalUserInfo`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalUserInfo {
    pub external_id: String,
    pub provider: String,
    pub username: Option<String>,
    pub email: Option<String>,
    pub display_name: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub picture_url: Option<String>,
    pub claims: BTreeMap<String, String>,
    pub suggested_roles: Vec<String>,
}

/// C# `ExternalAuthResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalAuthResult {
    pub is_success: bool,
    pub error_message: Option<String>,
    pub user: Option<ExternalUserInfo>,
    pub external_access_token: Option<String>,
    pub external_refresh_token: Option<String>,
    pub external_token_expires_at: Option<DateTime<Utc>>,
}

impl ExternalAuthResult {
    #[must_use]
    pub fn success(
        user: ExternalUserInfo,
        access_token: Option<String>,
        refresh_token: Option<String>,
    ) -> Self {
        Self {
            is_success: true,
            user: Some(user),
            external_access_token: access_token,
            external_refresh_token: refresh_token,
            ..Self::default()
        }
    }
    #[must_use]
    pub fn failure(message: &str) -> Self {
        Self {
            error_message: Some(message.to_string()),
            ..Self::default()
        }
    }
}

/// C# `ExternalTokenValidationResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExternalTokenValidationResult {
    pub is_valid: bool,
    pub external_user_id: Option<String>,
    pub failure_reason: Option<String>,
    pub is_expired: bool,
}

impl ExternalTokenValidationResult {
    #[must_use]
    pub fn valid(user_id: &str) -> Self {
        Self {
            is_valid: true,
            external_user_id: Some(user_id.to_string()),
            ..Self::default()
        }
    }
    #[must_use]
    pub fn invalid(reason: &str, is_expired: bool) -> Self {
        Self {
            failure_reason: Some(reason.to_string()),
            is_expired,
            ..Self::default()
        }
    }
}

/// C# `IAuthenticationPlugin`.
pub trait AuthenticationPlugin: Plugin {
    fn provider_name(&self) -> &str;
    fn display_name(&self) -> &str;
    fn icon_resource(&self) -> Option<&str> {
        None
    }
    fn is_available(&self) -> bool;
    fn authenticate(&mut self, context: &ExternalAuthContext) -> ExternalAuthResult;
    fn validate_token(&self, token: &str) -> ExternalTokenValidationResult;
    fn logout(&mut self, external_user_id: &str) -> bool;
    fn login_view(&self) -> Option<ViewDefinition> {
        None
    }
}

/// C# `MfaEnrollmentResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MfaEnrollmentResult {
    pub is_success: bool,
    pub error_message: Option<String>,
    pub setup_data: Option<String>,
    pub backup_codes: Vec<String>,
}

/// C# `MfaChallengeResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MfaChallengeResult {
    pub is_success: bool,
    pub error_message: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// C# `MfaVerificationResult`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MfaVerificationResult {
    pub is_valid: bool,
    pub error_message: Option<String>,
    pub is_backup_code: bool,
}

/// C# `IMfaPlugin`.
pub trait MfaPlugin: Plugin {
    fn method_name(&self) -> &str;
    fn display_name(&self) -> &str;
    fn is_available(&self) -> bool;
    fn enroll(&mut self, user_id: i64) -> MfaEnrollmentResult;
    fn send_challenge(&mut self, user_id: i64) -> MfaChallengeResult;
    fn verify(&mut self, user_id: i64, code: &str) -> MfaVerificationResult;
    fn unenroll(&mut self, user_id: i64) -> bool;
}
