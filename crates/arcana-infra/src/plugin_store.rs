//! `SQLite` [`PluginVersionStore`]: adapts `arcana_data::PluginStoreDao` rows to the plugin
//! contract types. Lives here because infra is the only layer that sees both `arcana-data`
//! and `arcana-plugin-host`.

use std::collections::BTreeMap;
use std::time::Duration;

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_data::{
    Db, PluginHealthSnapshotRow, PluginMetadataRow, PluginStoreDao, PluginVersionRow,
};
use arcana_plugin_api::{
    HealthState, PluginHealthStatus, PluginState, PluginType, PluginVersionInfo,
};
use arcana_plugin_host::version_store::{PluginMetadataRecord, PluginVersionStore};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

const STATES: [PluginState; 9] = [
    PluginState::Unknown,
    PluginState::NotLoaded,
    PluginState::Loaded,
    PluginState::Activating,
    PluginState::Active,
    PluginState::Deactivating,
    PluginState::Deactivated,
    PluginState::Uninstalled,
    PluginState::Error,
];
const HEALTH: [HealthState; 4] = [
    HealthState::Healthy,
    HealthState::Degraded,
    HealthState::Unhealthy,
    HealthState::Unknown,
];

fn index_of<T: PartialEq>(all: &[T], v: &T) -> i64 {
    all.iter()
        .position(|x| x == v)
        .and_then(|i| i64::try_from(i).ok())
        .unwrap_or_default()
}

fn from_index<T: Copy + Default>(all: &[T], i: i64) -> T {
    usize::try_from(i)
        .ok()
        .and_then(|i| all.get(i).copied())
        .unwrap_or_default()
}

fn corrupt(what: &str, e: impl std::fmt::Display) -> AppError {
    AppError::data(ErrorCode::DataCorruption, format!("{what}: {e}"))
}

fn utc(t: arcana_core::Timestamp) -> DateTime<Utc> {
    t.and_utc()
}

fn version_info(r: PluginVersionRow) -> AppResult<PluginVersionInfo> {
    Ok(PluginVersionInfo {
        version: semver::Version::parse(&r.version)
            .map_err(|e| corrupt("PluginVersions.Version", e))?,
        plugin_id: r.plugin_id,
        installed_at: utc(r.installed_at),
        backup_path: r.backup_path,
        is_current: r.is_current,
        size_bytes: r.size_bytes,
        release_notes: r.release_notes,
    })
}

fn version_row(i: &PluginVersionInfo) -> PluginVersionRow {
    PluginVersionRow {
        plugin_id: i.plugin_id.clone(),
        version: i.version.to_string(),
        installed_at: i.installed_at.naive_utc(),
        backup_path: i.backup_path.clone(),
        is_current: i.is_current,
        size_bytes: i.size_bytes,
        release_notes: i.release_notes.clone(),
        checksum: None,
    }
}

fn details_json(s: &PluginHealthStatus) -> Option<String> {
    if s.details.is_empty() {
        return None;
    }
    let list: Vec<serde_json::Value> = s
        .details
        .iter()
        .map(|d| {
            serde_json::json!({
                "CheckName": d.check_name,
                "State": d.state.name(),
                "Message": d.message,
                "DurationMs": u64::try_from(d.duration.as_millis()).unwrap_or(u64::MAX),
            })
        })
        .collect();
    serde_json::to_string(&list).ok()
}

/// [`PluginVersionStore`] over the application database.
#[derive(Clone)]
pub struct SqlitePluginVersionStore {
    dao: PluginStoreDao,
}

impl SqlitePluginVersionStore {
    #[must_use]
    pub fn new(db: Db) -> Self {
        Self {
            dao: PluginStoreDao::new(db),
        }
    }
}

#[async_trait]
impl PluginVersionStore for SqlitePluginVersionStore {
    async fn versions(&self, plugin_id: &str) -> AppResult<Vec<PluginVersionInfo>> {
        self.dao
            .versions(plugin_id)
            .await?
            .into_iter()
            .map(version_info)
            .collect()
    }

    async fn version(
        &self,
        plugin_id: &str,
        version: &str,
    ) -> AppResult<Option<PluginVersionInfo>> {
        self.dao
            .version(plugin_id, version)
            .await?
            .map(version_info)
            .transpose()
    }

    async fn current_version(&self, plugin_id: &str) -> AppResult<Option<PluginVersionInfo>> {
        self.dao
            .current_version(plugin_id)
            .await?
            .map(version_info)
            .transpose()
    }

    async fn add_version(&self, info: &PluginVersionInfo) -> AppResult<()> {
        self.dao.add_version(&version_row(info)).await
    }

    async fn update_version(&self, info: &PluginVersionInfo) -> AppResult<()> {
        self.dao.update_version(&version_row(info)).await
    }

    async fn delete_version(&self, plugin_id: &str, version: &str) -> AppResult<()> {
        self.dao.delete_version(plugin_id, version).await
    }

    async fn delete_all_versions(&self, plugin_id: &str) -> AppResult<()> {
        self.dao.delete_all_versions(plugin_id).await
    }

    async fn set_current_version(&self, plugin_id: &str, version: &str) -> AppResult<()> {
        self.dao.set_current_version(plugin_id, version).await
    }

    async fn record_health_snapshot(
        &self,
        plugin_id: &str,
        status: &PluginHealthStatus,
    ) -> AppResult<()> {
        let ms = status
            .response_time
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        self.dao
            .record_health_snapshot(&PluginHealthSnapshotRow {
                plugin_id: plugin_id.to_string(),
                checked_at: status.checked_at.naive_utc(),
                health_state: index_of(&HEALTH, &status.state),
                message: status.message.clone(),
                response_time_ms: ms,
                memory_usage_bytes: status.memory_usage_bytes,
                error_count: i64::from(status.error_count),
                details_json: details_json(status),
            })
            .await
    }

    async fn health_history(
        &self,
        plugin_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<PluginHealthStatus>> {
        Ok(self
            .dao
            .health_history(plugin_id, from.naive_utc(), to.naive_utc())
            .await?
            .into_iter()
            .map(|r| {
                // C# does not join the plugin name nor restore details.
                let mut s =
                    PluginHealthStatus::new(&r.plugin_id, "", from_index(&HEALTH, r.health_state));
                s.message = r.message;
                s.checked_at = utc(r.checked_at);
                s.response_time = Some(Duration::from_millis(
                    u64::try_from(r.response_time_ms).unwrap_or(0),
                ));
                s.memory_usage_bytes = r.memory_usage_bytes;
                s.error_count = i32::try_from(r.error_count).unwrap_or(i32::MAX);
                s
            })
            .collect())
    }

    async fn health_snapshot_count(&self, plugin_id: &str) -> AppResult<usize> {
        Ok(usize::try_from(self.dao.health_snapshot_count(plugin_id).await?).unwrap_or(0))
    }

    async fn upsert_metadata(&self, m: &PluginMetadataRecord) -> AppResult<()> {
        self.dao
            .upsert_metadata(&PluginMetadataRow {
                plugin_id: m.plugin_id.clone(),
                name: m.name.clone(),
                version: m.version.clone(),
                description: m.description.clone(),
                author: m.author.clone(),
                plugin_type: index_of(&PluginType::ALL, &m.plugin_type),
                state: index_of(&STATES, &m.state),
                icon_path: m.icon_path.clone(),
                install_path: m.install_path.clone(),
                installed_at: m.installed_at.naive_utc(),
                last_activated_at: m.last_activated_at.map(|t| t.naive_utc()),
                is_built_in: m.is_built_in,
                dependencies_json: erased::Json::to_json(&m.dependencies)?,
                metadata_json: erased::Json::to_json(&m.metadata)?,
                error_count: i64::from(m.error_count),
                last_error_at: m.last_error_at.map(|t| t.naive_utc()),
                last_error: m.last_error.clone(),
            })
            .await
    }

    async fn metadata(&self, plugin_id: &str) -> AppResult<Option<PluginMetadataRecord>> {
        self.dao
            .metadata(plugin_id)
            .await?
            .map(metadata_record)
            .transpose()
    }

    async fn all_metadata(&self) -> AppResult<Vec<PluginMetadataRecord>> {
        self.dao
            .all_metadata()
            .await?
            .into_iter()
            .map(metadata_record)
            .collect()
    }

    async fn delete_metadata(&self, plugin_id: &str) -> AppResult<()> {
        self.dao.delete_metadata(plugin_id).await
    }
}

/// Empty collections are stored as NULL, like C#'s nullable JSON columns.
mod erased {
    use arcana_core::AppResult;

    pub trait Json {
        fn to_json(&self) -> AppResult<Option<String>>;
    }

    impl<T: serde::Serialize> Json for Vec<T> {
        fn to_json(&self) -> AppResult<Option<String>> {
            if self.is_empty() {
                return Ok(None);
            }
            serde_json::to_string(self)
                .map(Some)
                .map_err(|e| super::corrupt("json", e))
        }
    }

    impl<K: serde::Serialize, V: serde::Serialize> Json for std::collections::BTreeMap<K, V> {
        fn to_json(&self) -> AppResult<Option<String>> {
            if self.is_empty() {
                return Ok(None);
            }
            serde_json::to_string(self)
                .map(Some)
                .map_err(|e| super::corrupt("json", e))
        }
    }
}

fn metadata_record(r: PluginMetadataRow) -> AppResult<PluginMetadataRecord> {
    let dependencies: Vec<String> = r
        .dependencies_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| corrupt("PluginMetadata.DependenciesJson", e))?
        .unwrap_or_default();
    let metadata: BTreeMap<String, String> = r
        .metadata_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|e| corrupt("PluginMetadata.MetadataJson", e))?
        .unwrap_or_default();
    Ok(PluginMetadataRecord {
        plugin_id: r.plugin_id,
        name: r.name,
        version: r.version,
        description: r.description,
        author: r.author,
        plugin_type: from_index(&PluginType::ALL, r.plugin_type),
        state: from_index(&STATES, r.state),
        icon_path: r.icon_path,
        install_path: r.install_path,
        installed_at: utc(r.installed_at),
        last_activated_at: r.last_activated_at.map(utc),
        is_built_in: r.is_built_in,
        dependencies,
        metadata,
        error_count: i32::try_from(r.error_count).unwrap_or(i32::MAX),
        last_error_at: r.last_error_at.map(utc),
        last_error: r.last_error,
    })
}
