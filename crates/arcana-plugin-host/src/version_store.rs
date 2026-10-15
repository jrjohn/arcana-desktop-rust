//! C# `PluginVersionRepository` as an async trait, plus the persisted plugin metadata record
//! and an in-memory implementation (tests, and hosts without a database).
//!
//! The `SQLite` implementation lives in `arcana-infra::plugin_store` (it needs `arcana-data`,
//! which this crate may not depend on).

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use arcana_core::{AppError, AppResult, ErrorCode};
use arcana_plugin_api::{PluginHealthStatus, PluginState, PluginType, PluginVersionInfo};
use async_trait::async_trait;
use chrono::{DateTime, Utc};

/// Snapshots kept per plugin (C# "Keep only last 1000 snapshots per plugin").
pub const MAX_HEALTH_SNAPSHOTS: usize = 1000;

/// C# `PluginMetadataEntity` in contract types.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PluginMetadataRecord {
    pub plugin_id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub plugin_type: PluginType,
    pub state: PluginState,
    pub icon_path: Option<String>,
    pub install_path: String,
    pub installed_at: DateTime<Utc>,
    pub last_activated_at: Option<DateTime<Utc>>,
    pub is_built_in: bool,
    pub dependencies: Vec<String>,
    pub metadata: BTreeMap<String, String>,
    pub error_count: i32,
    pub last_error_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl PluginMetadataRecord {
    #[must_use]
    pub fn new(plugin_id: &str, name: &str, version: &str, install_path: &str) -> Self {
        Self {
            plugin_id: plugin_id.to_string(),
            name: name.to_string(),
            version: version.to_string(),
            description: None,
            author: None,
            plugin_type: PluginType::default(),
            state: PluginState::default(),
            icon_path: None,
            install_path: install_path.to_string(),
            installed_at: Utc::now(),
            last_activated_at: None,
            is_built_in: false,
            dependencies: Vec::new(),
            metadata: BTreeMap::new(),
            error_count: 0,
            last_error_at: None,
            last_error: None,
        }
    }
}

/// C# `PluginVersionRepository` + metadata persistence (C# declared `PluginMetadata` but
/// never wrote it).
#[async_trait]
pub trait PluginVersionStore: Send + Sync {
    /// C# `GetVersionsAsync` — newest `installed_at` first.
    async fn versions(&self, plugin_id: &str) -> AppResult<Vec<PluginVersionInfo>>;
    /// C# `GetVersionAsync` (`version` as stored, e.g. `"1.0.0"`).
    async fn version(&self, plugin_id: &str, version: &str)
    -> AppResult<Option<PluginVersionInfo>>;
    /// C# `GetCurrentVersionAsync`.
    async fn current_version(&self, plugin_id: &str) -> AppResult<Option<PluginVersionInfo>>;
    /// C# `AddVersionAsync`; a current version clears the previous current flag.
    /// Errors with `DuplicateEntry` on an existing `(plugin, version)` (C# unique index).
    async fn add_version(&self, info: &PluginVersionInfo) -> AppResult<()>;
    /// C# `UpdateVersionAsync`; no-op when the version does not exist.
    async fn update_version(&self, info: &PluginVersionInfo) -> AppResult<()>;
    /// C# `DeleteVersionAsync`.
    async fn delete_version(&self, plugin_id: &str, version: &str) -> AppResult<()>;
    /// C# `DeleteAllVersionsAsync`.
    async fn delete_all_versions(&self, plugin_id: &str) -> AppResult<()>;
    /// C# `SetCurrentVersionAsync`: only `version` is current afterwards (none if unknown).
    async fn set_current_version(&self, plugin_id: &str, version: &str) -> AppResult<()>;
    /// C# `RecordHealthSnapshotAsync`, keeping the newest [`MAX_HEALTH_SNAPSHOTS`].
    async fn record_health_snapshot(
        &self,
        plugin_id: &str,
        status: &PluginHealthStatus,
    ) -> AppResult<()>;
    /// C# `GetHealthHistoryAsync` — `from <= checked_at <= to`, newest first.
    async fn health_history(
        &self,
        plugin_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<PluginHealthStatus>>;
    /// Stored snapshot count (C# tests read the `DbSet` directly).
    async fn health_snapshot_count(&self, plugin_id: &str) -> AppResult<usize>;
    async fn upsert_metadata(&self, record: &PluginMetadataRecord) -> AppResult<()>;
    async fn metadata(&self, plugin_id: &str) -> AppResult<Option<PluginMetadataRecord>>;
    async fn all_metadata(&self) -> AppResult<Vec<PluginMetadataRecord>>;
    async fn delete_metadata(&self, plugin_id: &str) -> AppResult<()>;

    /// Version a rollback goes back to: the newest version installed before the current one
    /// (C# `PluginManagerService.RollbackPluginAsync` picks it from `GetVersionsAsync`).
    async fn rollback_target(&self, plugin_id: &str) -> AppResult<Option<PluginVersionInfo>> {
        let versions = self.versions(plugin_id).await?;
        let Some(pos) = versions.iter().position(|v| v.is_current) else {
            return Ok(None);
        };
        Ok(versions.into_iter().nth(pos + 1))
    }
}

#[derive(Default)]
struct Mem {
    /// Insertion order kept so equal timestamps sort newest-insert first, like the SQL `Id`.
    versions: Vec<PluginVersionInfo>,
    snapshots: Vec<(u64, String, PluginHealthStatus)>,
    next_snapshot: u64,
    metadata: BTreeMap<String, PluginMetadataRecord>,
}

/// In-memory [`PluginVersionStore`] (C# tests used EF `UseInMemoryDatabase`).
#[derive(Default)]
pub struct InMemoryPluginVersionStore {
    mem: Mutex<Mem>,
}

impl std::fmt::Debug for InMemoryPluginVersionStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InMemoryPluginVersionStore")
    }
}

impl InMemoryPluginVersionStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    fn lock(&self) -> MutexGuard<'_, Mem> {
        self.mem.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn same(v: &PluginVersionInfo, plugin_id: &str, version: &str) -> bool {
    v.plugin_id == plugin_id && v.version.to_string() == version
}

#[async_trait]
impl PluginVersionStore for InMemoryPluginVersionStore {
    async fn versions(&self, plugin_id: &str) -> AppResult<Vec<PluginVersionInfo>> {
        let mem = self.lock();
        let mut out: Vec<(usize, PluginVersionInfo)> = mem
            .versions
            .iter()
            .enumerate()
            .filter(|(_, v)| v.plugin_id == plugin_id)
            .map(|(i, v)| (i, v.clone()))
            .collect();
        out.sort_by(|(ia, a), (ib, b)| b.installed_at.cmp(&a.installed_at).then(ib.cmp(ia)));
        Ok(out.into_iter().map(|(_, v)| v).collect())
    }

    async fn version(
        &self,
        plugin_id: &str,
        version: &str,
    ) -> AppResult<Option<PluginVersionInfo>> {
        Ok(self
            .lock()
            .versions
            .iter()
            .find(|v| same(v, plugin_id, version))
            .cloned())
    }

    async fn current_version(&self, plugin_id: &str) -> AppResult<Option<PluginVersionInfo>> {
        Ok(self
            .lock()
            .versions
            .iter()
            .find(|v| v.plugin_id == plugin_id && v.is_current)
            .cloned())
    }

    async fn add_version(&self, info: &PluginVersionInfo) -> AppResult<()> {
        let mut mem = self.lock();
        let key = info.version.to_string();
        if mem.versions.iter().any(|v| same(v, &info.plugin_id, &key)) {
            return Err(AppError::data(
                ErrorCode::DuplicateEntry,
                format!("{} {key} already recorded", info.plugin_id),
            ));
        }
        if info.is_current {
            for v in mem
                .versions
                .iter_mut()
                .filter(|v| v.plugin_id == info.plugin_id)
            {
                v.is_current = false;
            }
        }
        mem.versions.push(info.clone());
        Ok(())
    }

    async fn update_version(&self, info: &PluginVersionInfo) -> AppResult<()> {
        let mut mem = self.lock();
        let key = info.version.to_string();
        if !mem.versions.iter().any(|v| same(v, &info.plugin_id, &key)) {
            return Ok(());
        }
        for v in mem
            .versions
            .iter_mut()
            .filter(|v| v.plugin_id == info.plugin_id)
        {
            if v.version.to_string() == key {
                v.is_current = info.is_current;
                v.size_bytes = info.size_bytes;
                v.release_notes.clone_from(&info.release_notes);
            } else if info.is_current {
                v.is_current = false;
            }
        }
        Ok(())
    }

    async fn delete_version(&self, plugin_id: &str, version: &str) -> AppResult<()> {
        self.lock()
            .versions
            .retain(|v| !same(v, plugin_id, version));
        Ok(())
    }

    async fn delete_all_versions(&self, plugin_id: &str) -> AppResult<()> {
        self.lock().versions.retain(|v| v.plugin_id != plugin_id);
        Ok(())
    }

    async fn set_current_version(&self, plugin_id: &str, version: &str) -> AppResult<()> {
        for v in self
            .lock()
            .versions
            .iter_mut()
            .filter(|v| v.plugin_id == plugin_id)
        {
            v.is_current = v.version.to_string() == version;
        }
        Ok(())
    }

    async fn record_health_snapshot(
        &self,
        plugin_id: &str,
        status: &PluginHealthStatus,
    ) -> AppResult<()> {
        let mut mem = self.lock();
        let id = mem.next_snapshot;
        mem.next_snapshot += 1;
        mem.snapshots
            .push((id, plugin_id.to_string(), status.clone()));
        let mut mine: Vec<(DateTime<Utc>, u64)> = mem
            .snapshots
            .iter()
            .filter(|(_, p, _)| p == plugin_id)
            .map(|(i, _, s)| (s.checked_at, *i))
            .collect();
        if mine.len() > MAX_HEALTH_SNAPSHOTS {
            mine.sort_by(|a, b| b.cmp(a));
            let drop: Vec<u64> = mine[MAX_HEALTH_SNAPSHOTS..]
                .iter()
                .map(|(_, i)| *i)
                .collect();
            mem.snapshots.retain(|(i, _, _)| !drop.contains(i));
        }
        Ok(())
    }

    async fn health_history(
        &self,
        plugin_id: &str,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> AppResult<Vec<PluginHealthStatus>> {
        let mem = self.lock();
        let mut out: Vec<(u64, PluginHealthStatus)> = mem
            .snapshots
            .iter()
            .filter(|(_, p, s)| p == plugin_id && s.checked_at >= from && s.checked_at <= to)
            .map(|(i, _, s)| (*i, s.clone()))
            .collect();
        out.sort_by(|(ia, a), (ib, b)| b.checked_at.cmp(&a.checked_at).then(ib.cmp(ia)));
        // C# `GetHealthHistoryAsync` does not join the plugin name nor restore details.
        Ok(out
            .into_iter()
            .map(|(_, mut s)| {
                s.plugin_name = String::new();
                s.details.clear();
                s.last_error = None;
                s.last_error_at = None;
                s
            })
            .collect())
    }

    async fn health_snapshot_count(&self, plugin_id: &str) -> AppResult<usize> {
        Ok(self
            .lock()
            .snapshots
            .iter()
            .filter(|(_, p, _)| p == plugin_id)
            .count())
    }

    async fn upsert_metadata(&self, record: &PluginMetadataRecord) -> AppResult<()> {
        self.lock()
            .metadata
            .insert(record.plugin_id.clone(), record.clone());
        Ok(())
    }

    async fn metadata(&self, plugin_id: &str) -> AppResult<Option<PluginMetadataRecord>> {
        Ok(self.lock().metadata.get(plugin_id).cloned())
    }

    async fn all_metadata(&self) -> AppResult<Vec<PluginMetadataRecord>> {
        Ok(self.lock().metadata.values().cloned().collect())
    }

    async fn delete_metadata(&self, plugin_id: &str) -> AppResult<()> {
        self.lock().metadata.remove(plugin_id);
        Ok(())
    }
}
