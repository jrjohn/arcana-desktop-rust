//! Plugin contracts (C# `Arcana.Plugins.Contracts`).
//!
//! Everything a plugin may see: the [`Plugin`] trait and its specialised variants, the
//! [`PluginContext`] service bundle, permission flags, the `plugin.json` manifest, activation
//! events and contribution validators. Implementations live in `arcana-plugin-host`.
//!
//! C# is async throughout; the host here is driven from the egui frame loop, so contracts are
//! synchronous and return [`AppResult`](arcana_core::AppResult) where C# throws.

pub mod activation;
pub mod context;
pub mod health;
pub mod manager;
pub mod manifest;
pub mod mvvm;
pub mod permission;
pub mod plugin;
pub mod schema;
pub mod services;
pub mod validation;

pub use activation::{ActivationEventType, activation_events};
pub use context::{HostServices, PluginContext};
pub use health::{
    HealthCheckResult, HealthState, PluginHealthCheck, PluginHealthCheckResult, PluginHotReload,
    PluginUpgradeable,
};
pub use manager::{
    InstallPhase, PluginHealthStatus, PluginInfo, PluginOperationResult, PluginStateChanged,
    PluginVersionInfo,
};
pub use manifest::{
    ManifestCommandDefinition, ManifestConfigurationDefinition, ManifestConfigurationProperty,
    ManifestContributions, ManifestMenuDefinition, ManifestViewDefinition, PluginManifest,
};
pub use permission::{
    PermissionCheckResult, PermissionRequest, PluginPermission, PluginPermissionManifest,
};
pub use plugin::{
    AuthenticationPlugin, ExternalAuthContext, ExternalAuthResult, ExternalTokenValidationResult,
    ExternalUserInfo, MfaChallengeResult, MfaEnrollmentResult, MfaPlugin, MfaVerificationResult,
    Plugin, PluginMetadata, PluginState, PluginType,
};
pub use services::*;
pub use validation::{
    ContributionValidationResult, validate_command_id, validate_menu_item, validate_view,
};
