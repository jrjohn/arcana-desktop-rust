//! `IPluginHealthCheck`, `IPluginHotReload`, `IPluginUpgradeable` and health result types.

use std::collections::BTreeMap;
use std::time::Duration;

use arcana_core::AppResult;
use serde::{Deserialize, Serialize};

/// C# `HealthState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HealthState {
    Healthy,
    Degraded,
    Unhealthy,
    #[default]
    Unknown,
}

impl HealthState {
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Healthy => "Healthy",
            Self::Degraded => "Degraded",
            Self::Unhealthy => "Unhealthy",
            Self::Unknown => "Unknown",
        }
    }
    #[must_use]
    pub fn parse(s: &str) -> Self {
        match s {
            "Healthy" => Self::Healthy,
            "Degraded" => Self::Degraded,
            "Unhealthy" => Self::Unhealthy,
            _ => Self::Unknown,
        }
    }
}

/// C# `HealthCheckResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthCheckResult {
    pub check_name: String,
    pub state: HealthState,
    pub message: Option<String>,
    pub duration: Duration,
}

/// C# `PluginHealthCheckResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginHealthCheckResult {
    pub state: HealthState,
    pub message: Option<String>,
    pub data: BTreeMap<String, String>,
    pub checks: Vec<HealthCheckResult>,
}

impl PluginHealthCheckResult {
    fn of(state: HealthState, message: Option<&str>) -> Self {
        Self {
            state,
            message: message.map(str::to_string),
            data: BTreeMap::new(),
            checks: Vec::new(),
        }
    }
    #[must_use]
    pub fn healthy(message: Option<&str>) -> Self {
        Self::of(HealthState::Healthy, message)
    }
    #[must_use]
    pub fn degraded(message: &str) -> Self {
        Self::of(HealthState::Degraded, Some(message))
    }
    #[must_use]
    pub fn unhealthy(message: &str) -> Self {
        Self::of(HealthState::Unhealthy, Some(message))
    }
}

/// C# `IPluginHealthCheck`.
pub trait PluginHealthCheck {
    /// # Errors
    /// A failing check; the monitor records it as unhealthy.
    fn check_health(&self) -> AppResult<PluginHealthCheckResult>;
}

/// C# `IPluginHotReload`. State crosses the reload as JSON values.
pub trait PluginHotReload {
    /// # Errors
    /// Aborts the reload.
    fn on_before_reload(&mut self) -> AppResult<()>;
    /// # Errors
    /// Reported after the reload.
    fn on_after_reload(&mut self) -> AppResult<()>;
    fn state(&self) -> BTreeMap<String, serde_json::Value>;
    fn restore_state(&mut self, state: BTreeMap<String, serde_json::Value>);
}

/// C# `IPluginUpgradeable`.
pub trait PluginUpgradeable {
    fn on_before_upgrade(&mut self, from: &semver::Version, to: &semver::Version) -> bool;
    fn on_after_upgrade(&mut self, from: &semver::Version, to: &semver::Version);
    fn on_before_rollback(&mut self, from: &semver::Version, to: &semver::Version) -> bool;
    fn on_after_rollback(&mut self, from: &semver::Version, to: &semver::Version);
    fn can_upgrade_to(&self, target: &semver::Version) -> bool;
    fn can_rollback_to(&self, target: &semver::Version) -> bool;
}
