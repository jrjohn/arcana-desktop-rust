//! `IPluginManager` result and info types (`IPluginInfo`, `PluginVersionInfo`,
//! `PluginHealthStatus`, `PluginOperationResult`, event args).

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::health::{HealthCheckResult, HealthState};
use crate::plugin::{PluginState, PluginType};

/// C# `IPluginInfo` as a value snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginInfo {
    pub id: String,
    pub name: String,
    pub version: semver::Version,
    pub description: Option<String>,
    pub author: Option<String>,
    pub plugin_type: PluginType,
    pub state: PluginState,
    pub icon_path: Option<String>,
    pub install_path: String,
    pub installed_at: DateTime<Utc>,
    pub last_activated_at: Option<DateTime<Utc>>,
    pub is_built_in: bool,
    pub can_uninstall: bool,
    pub can_upgrade: bool,
    pub dependencies: Vec<String>,
    pub metadata: BTreeMap<String, String>,
}

/// C# `PluginVersionInfo`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginVersionInfo {
    pub plugin_id: String,
    pub version: semver::Version,
    pub installed_at: DateTime<Utc>,
    pub backup_path: String,
    pub is_current: bool,
    pub size_bytes: i64,
    pub release_notes: Option<String>,
}

/// C# `PluginHealthStatus`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealthStatus {
    pub plugin_id: String,
    pub plugin_name: String,
    pub state: HealthState,
    pub message: Option<String>,
    pub checked_at: DateTime<Utc>,
    pub response_time: Option<Duration>,
    pub memory_usage_bytes: i64,
    pub error_count: i32,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
    pub details: Vec<HealthCheckResult>,
}

impl PluginHealthStatus {
    #[must_use]
    pub fn new(plugin_id: &str, plugin_name: &str, state: HealthState) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            plugin_name: plugin_name.to_string(),
            state,
            message: None,
            checked_at: Utc::now(),
            response_time: None,
            memory_usage_bytes: 0,
            error_count: 0,
            last_error_at: None,
            last_error: None,
            details: Vec::new(),
        }
    }
}

/// C# `PluginOperationResult` (the `Exception` becomes its message).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PluginOperationResult {
    pub success: bool,
    pub plugin_id: Option<String>,
    pub message: Option<String>,
    pub error_code: Option<String>,
    pub warnings: Vec<String>,
}

impl PluginOperationResult {
    #[must_use]
    pub fn succeeded(plugin_id: &str, message: Option<&str>) -> Self {
        Self {
            success: true,
            plugin_id: Some(plugin_id.to_string()),
            message: message.map(str::to_string),
            ..Self::default()
        }
    }
    #[must_use]
    pub fn failed(plugin_id: Option<&str>, message: &str, error_code: Option<&str>) -> Self {
        Self {
            success: false,
            plugin_id: plugin_id.map(str::to_string),
            message: Some(message.to_string()),
            error_code: error_code.map(str::to_string),
            warnings: Vec::new(),
        }
    }
}

/// C# `PluginStateChangedEventArgs`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginStateChanged {
    pub plugin_id: String,
    pub old_state: PluginState,
    pub new_state: PluginState,
    pub message: Option<String>,
}

/// C# `InstallPhase`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallPhase {
    Downloading,
    Extracting,
    Validating,
    BackingUp,
    Installing,
    Activating,
    Completed,
    Failed,
    RollingBack,
}
